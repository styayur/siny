// SPDX-License-Identifier: AGPL-3.0-only
use crate::{tasks::check_cancel, Audio, Backend, Cancel, Operation, Result, Scheduler, Settings};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::Duration;

struct Chunks {
    count: usize,
    blocked: Option<mpsc::Sender<()>>,
    cancel_after_first: bool,
}
impl Backend for Chunks {
    fn recognize(&mut self, _: &Audio, _: &Cancel) -> Result<String> {
        Ok("recognized".into())
    }
    fn synthesize(&mut self, _: &str, _: &Cancel) -> Result<Audio> {
        Ok(chunk(0))
    }
    fn synthesize_stream(
        &mut self,
        _: &str,
        cancel: &Cancel,
        emit: &mut dyn FnMut(Audio) -> Result<()>,
    ) -> Result<()> {
        for index in 0..self.count {
            if index == 2 {
                if let Some(sender) = self.blocked.take() {
                    sender.send(()).unwrap();
                }
            }
            check_cancel(cancel)?;
            emit(chunk(index))?;
            if self.cancel_after_first {
                cancel.store(true, Ordering::Release);
            }
            check_cancel(cancel)?;
        }
        Ok(())
    }
    fn unload(&mut self) {}
}
fn chunk(index: usize) -> Audio {
    Audio {
        samples: vec![(index % 3 + 1) as f32 / 10.0; 160],
        sample_rate: 16000,
    }
}

#[test]
fn full_stream_queue_cancels_without_a_consumer_and_worker_recovers() {
    let (sender, receiver) = mpsc::channel();
    let scheduler = Scheduler::new(
        Chunks {
            count: 8,
            blocked: Some(sender),
            cancel_after_first: false,
        },
        Settings::default(),
    )
    .unwrap();
    let job = scheduler
        .submit(Operation::ReadStream("long text".into()))
        .unwrap();
    receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        job.result.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    job.cancel();
    assert_eq!(
        job.result
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap_err(),
        "Cancelled"
    );
    let recovery = scheduler
        .submit(Operation::Read("recovered".into()))
        .unwrap();
    assert!(recovery
        .result
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .is_ok());
}

#[test]
fn stream_preserves_order_and_does_not_return_accumulated_audio() {
    let scheduler = Scheduler::new(
        Chunks {
            count: 3,
            blocked: None,
            cancel_after_first: false,
        },
        Settings::default(),
    )
    .unwrap();
    let mut job = scheduler
        .submit(Operation::ReadStream("a document".repeat(2000)))
        .unwrap();
    let audio = job.audio.take().unwrap();
    for index in 0..3 {
        let received = audio.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(received.samples, chunk(index).samples);
    }
    let outcome = job
        .result
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert!(outcome.audio.is_none());
    assert!(outcome.text.chars().count() > 10_000);
}

#[test]
fn dropping_scheduler_cancels_backpressure_even_if_the_job_is_kept_alive() {
    let (sender, receiver) = mpsc::channel();
    let scheduler = Scheduler::new(
        Chunks {
            count: 8,
            blocked: Some(sender),
            cancel_after_first: false,
        },
        Settings::default(),
    )
    .unwrap();
    let job = scheduler
        .submit(Operation::ReadStream("long text".into()))
        .unwrap();
    receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    drop(scheduler);
    assert_eq!(
        job.result
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap_err(),
        "Cancelled"
    );
}

#[test]
fn cancelled_streamed_wav_preserves_existing_file_then_success_is_valid_pcm() {
    let dir = std::env::temp_dir().join(format!("siny-wav-stream-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("speech.wav");
    std::fs::write(&path, "existing destination").unwrap();
    let mut backend = Chunks {
        count: 3,
        blocked: None,
        cancel_after_first: true,
    };
    let cancel = Arc::new(AtomicBool::new(false));
    assert!(crate::wav::export(&mut backend, "text", &path, &cancel).is_err());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "existing destination"
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    cancel.store(false, Ordering::Release);
    backend.cancel_after_first = false;
    crate::wav::export(&mut backend, "text", &path, &cancel).unwrap();
    let reader = hound::WavReader::open(&path).unwrap();
    assert_eq!(reader.spec().sample_rate, 16000);
    assert_eq!(reader.spec().bits_per_sample, 16);
    assert_eq!(reader.len(), 480);
    drop(reader);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn terminology_expansion_is_bounded_before_allocating_the_full_result() {
    let terms = vec![crate::terms::Term {
        from: "a".into(),
        to: "expanded".into(),
    }];
    assert!(crate::terms::apply_bounded("a".repeat(100_000).as_str(), &terms, 32).is_err());
    assert_eq!(
        crate::terms::apply_bounded("a", &terms, 8).unwrap(),
        "expanded"
    );
}
