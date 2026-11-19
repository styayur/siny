// SPDX-License-Identifier: AGPL-3.0-only
use crate::{terms, Result, Settings};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
    Arc,
};
use std::time::Duration;

pub type Cancel = Arc<AtomicBool>;
pub const STREAM_QUEUE_CAPACITY: usize = 2;
#[derive(Debug, Clone)]
pub struct Voice {
    pub id: i32,
    pub name: String,
}
#[derive(Debug, Clone)]
pub struct Audio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}
impl Audio {
    pub fn validate(&self, max_seconds: usize) -> Result<()> {
        if !(8_000..=192_000).contains(&self.sample_rate)
            || self.samples.is_empty()
            || self.samples.len() > self.sample_rate as usize * max_seconds
            || self.samples.iter().any(|s| !s.is_finite())
        {
            return Err("Audio must be finite, nonempty, 8–192 kHz, within duration limit".into());
        }
        Ok(())
    }
    pub fn seconds(&self) -> f32 {
        self.samples.len() as f32 / self.sample_rate as f32
    }
}
#[derive(Debug)]
pub enum Operation {
    Dictate(Audio),
    Read(String),
    ReadStream(String),
    ExportWav {
        text: String,
        path: std::path::PathBuf,
    },
    Voices,
    DictateThenRead(Audio),
    DictateThenReadStream(Audio),
    Verify(String),
    Unload,
}
#[derive(Debug)]
pub struct Outcome {
    pub text: String,
    pub audio: Option<Audio>,
    pub recognized: Option<String>,
    pub similarity: Option<f32>,
    pub elapsed_seconds: f64,
    pub voices: Option<Vec<Voice>>,
    pub saved: Option<std::path::PathBuf>,
}
pub trait Backend: Send + 'static {
    fn recognize(&mut self, audio: &Audio, cancel: &Cancel) -> Result<String>;
    fn synthesize(&mut self, text: &str, cancel: &Cancel) -> Result<Audio>;
    fn synthesize_stream(
        &mut self,
        text: &str,
        cancel: &Cancel,
        emit: &mut dyn FnMut(Audio) -> Result<()>,
    ) -> Result<()> {
        emit(self.synthesize(text, cancel)?)
    }
    fn voices(&mut self) -> Result<Vec<Voice>> {
        Err("Voice metadata is unavailable from this backend".into())
    }
    fn unload(&mut self);
}
struct Request {
    operation: Operation,
    cancel: Cancel,
    reply: SyncSender<Result<Outcome>>,
    audio: SyncSender<Audio>,
}
enum Message {
    Work(Request),
    Shutdown,
}
pub struct Job {
    pub cancel: Cancel,
    pub result: Receiver<Result<Outcome>>,
    pub audio: Option<Receiver<Audio>>,
}
impl Job {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}
pub struct Scheduler {
    sender: SyncSender<Message>,
    shutdown: Cancel,
    active: Arc<std::sync::Mutex<Option<Cancel>>>,
}
impl Scheduler {
    pub fn new(mut backend: impl Backend, settings: Settings) -> Result<Self> {
        settings.validate()?;
        let (sender, receiver) = mpsc::sync_channel::<Message>(2);
        let shutdown = Arc::new(AtomicBool::new(false));
        let stop = shutdown.clone();
        let active = Arc::new(std::sync::Mutex::new(None::<Cancel>));
        let active_job = active.clone();
        std::thread::Builder::new()
            .name("siny-inference".into())
            .spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    match receiver.recv_timeout(Duration::from_secs(settings.idle_unload_seconds)) {
                        Ok(Message::Work(request)) => {
                            if let Ok(mut active) = active_job.lock() {
                                *active = Some(request.cancel.clone());
                            }
                            if stop.load(Ordering::Acquire) {
                                request.cancel.store(true, Ordering::Release);
                            }
                            let result = execute(
                                &mut backend,
                                request.operation,
                                &settings,
                                &request.cancel,
                                &request.audio,
                            );
                            let _ = request.reply.send(result);
                            if let Ok(mut active) = active_job.lock() {
                                *active = None;
                            }
                        }
                        Ok(Message::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => backend.unload(),
                    }
                }
                backend.unload();
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            sender,
            shutdown,
            active,
        })
    }
    pub fn submit(&self, operation: Operation) -> Result<Job> {
        match &operation {
            Operation::Dictate(a)
            | Operation::DictateThenRead(a)
            | Operation::DictateThenReadStream(a) => a.validate(60)?,
            Operation::Read(t) | Operation::Verify(t) => validate_text(t, 10_000)?,
            Operation::ReadStream(t) | Operation::ExportWav { text: t, .. } => {
                validate_text(t, 1_000_000)?
            }
            Operation::Unload | Operation::Voices => (),
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let (reply, result) = mpsc::sync_channel(1);
        let (audio, chunks) = mpsc::sync_channel(STREAM_QUEUE_CAPACITY);
        self.sender
            .try_send(Message::Work(Request {
                operation,
                cancel: cancel.clone(),
                reply,
                audio,
            }))
            .map_err(|e| e.to_string())?;
        Ok(Job {
            cancel,
            result,
            audio: Some(chunks),
        })
    }
}
impl Drop for Scheduler {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Ok(active) = self.active.lock() {
            if let Some(cancel) = active.as_ref() {
                cancel.store(true, Ordering::Release);
            }
        }
        let _ = self.sender.try_send(Message::Shutdown);
    }
}
pub fn check_cancel(cancel: &Cancel) -> Result<()> {
    if cancel.load(Ordering::Acquire) {
        Err("Cancelled".into())
    } else {
        Ok(())
    }
}
fn validate_text(text: &str, limit: usize) -> Result<()> {
    if text.trim().is_empty() || text.chars().count() > limit || text.contains('\0') {
        return Err(format!("Text requires 1–{limit} characters without NUL"));
    }
    Ok(())
}
fn execute(
    backend: &mut impl Backend,
    operation: Operation,
    settings: &Settings,
    cancel: &Cancel,
    audio_sender: &SyncSender<Audio>,
) -> Result<Outcome> {
    check_cancel(cancel)?;
    let started = std::time::Instant::now();
    let mut outcome = Outcome {
        text: String::new(),
        audio: None,
        recognized: None,
        similarity: None,
        elapsed_seconds: 0.0,
        voices: None,
        saved: None,
    };
    let verify = matches!(&operation, Operation::Verify(_));
    match operation {
        Operation::Unload => backend.unload(),
        Operation::Voices => outcome.voices = Some(backend.voices()?),
        Operation::ReadStream(text) => {
            outcome.text = terms::apply_bounded(&text, &settings.terms, 4_000_000)?;
            validate_text(&outcome.text, 1_000_000)?;
            stream(backend, &outcome.text, cancel, audio_sender)?;
        }
        Operation::DictateThenReadStream(audio) => {
            outcome.text = terms::apply_bounded(
                &backend.recognize(&audio, cancel)?,
                &settings.terms,
                4_000_000,
            )?;
            validate_text(&outcome.text, 1_000_000)?;
            stream(backend, &outcome.text, cancel, audio_sender)?;
        }
        Operation::ExportWav { text, path } => {
            outcome.text = terms::apply_bounded(&text, &settings.terms, 4_000_000)?;
            validate_text(&outcome.text, 1_000_000)?;
            crate::wav::export(backend, &outcome.text, &path, cancel)?;
            outcome.saved = Some(path);
        }
        Operation::Dictate(audio) => {
            outcome.text = terms::apply(&backend.recognize(&audio, cancel)?, &settings.terms)
        }
        Operation::Read(text) | Operation::Verify(text) => {
            outcome.text = terms::apply(&text, &settings.terms);
            validate_text(&outcome.text, 10_000)?;
            outcome.audio = Some(backend.synthesize(&outcome.text, cancel)?);
        }
        Operation::DictateThenRead(audio) => {
            outcome.text = terms::apply(&backend.recognize(&audio, cancel)?, &settings.terms);
            check_cancel(cancel)?;
            validate_text(&outcome.text, 10_000)?;
            outcome.audio = Some(backend.synthesize(&outcome.text, cancel)?);
        }
    }
    if let Some(audio) = &outcome.audio {
        audio.validate(120)?;
    }
    check_cancel(cancel)?;
    if verify {
        let recognized = terms::apply(
            &backend.recognize(outcome.audio.as_ref().expect("synthesized audio"), cancel)?,
            &settings.terms,
        );
        outcome.similarity = Some(terms::similarity(&outcome.text, &recognized));
        outcome.recognized = Some(recognized);
    }
    check_cancel(cancel)?;
    outcome.elapsed_seconds = started.elapsed().as_secs_f64();
    Ok(outcome)
}
fn stream(
    backend: &mut impl Backend,
    text: &str,
    cancel: &Cancel,
    audio_sender: &SyncSender<Audio>,
) -> Result<()> {
    backend.synthesize_stream(text, cancel, &mut |mut audio| {
        audio.validate(120)?;
        loop {
            check_cancel(cancel)?;
            match audio_sender.try_send(audio) {
                Ok(()) => return Ok(()),
                Err(mpsc::TrySendError::Full(chunk)) => {
                    audio = chunk;
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err("Playback consumer closed".into())
                }
            }
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Echo;
    impl Backend for Echo {
        fn recognize(&mut self, _: &Audio, _: &Cancel) -> Result<String> {
            Ok("hello".into())
        }
        fn synthesize(&mut self, _: &str, _: &Cancel) -> Result<Audio> {
            Ok(Audio {
                samples: vec![0.1; 160],
                sample_rate: 16000,
            })
        }
        fn unload(&mut self) {}
    }
    #[test]
    fn cancelled_native_result_is_discarded_and_worker_recovers() {
        struct Slow {
            started: std::sync::mpsc::Sender<()>,
            release: std::sync::mpsc::Receiver<()>,
            first: bool,
        }
        impl Backend for Slow {
            fn recognize(&mut self, _: &Audio, _: &Cancel) -> Result<String> {
                Ok("hello".into())
            }
            fn synthesize(&mut self, _: &str, _: &Cancel) -> Result<Audio> {
                if self.first {
                    self.first = false;
                    self.started.send(()).unwrap();
                    self.release.recv_timeout(Duration::from_secs(2)).unwrap();
                }
                Ok(Audio {
                    samples: vec![0.1; 160],
                    sample_rate: 16000,
                })
            }
            fn unload(&mut self) {}
        }
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = Scheduler::new(
            Slow {
                started: started_tx,
                release: release_rx,
                first: true,
            },
            Settings::default(),
        )
        .unwrap();
        let cancelled = worker.submit(Operation::Read("first".into())).unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        cancelled.cancel();
        release_tx.send(()).unwrap();
        assert_eq!(
            cancelled
                .result
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap_err(),
            "Cancelled"
        );
        let recovered = worker.submit(Operation::Read("second".into())).unwrap();
        assert!(recovered
            .result
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .is_ok());
    }

    #[test]
    fn workflows_validate_and_recover() {
        let worker = Scheduler::new(Echo, Settings::default()).unwrap();
        assert!(worker.submit(Operation::Read(String::new())).is_err());
        assert!(worker
            .submit(Operation::Dictate(Audio {
                samples: vec![f32::NAN],
                sample_rate: 16000
            }))
            .is_err());
        let job = worker
            .submit(Operation::DictateThenRead(Audio {
                samples: vec![0.1; 160],
                sample_rate: 16000,
            }))
            .unwrap();
        let result = job
            .result
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        assert_eq!(result.text, "hello");
        assert!(result.audio.is_some());
        let job = worker.submit(Operation::Verify("hello".into())).unwrap();
        assert_eq!(
            job.result
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap()
                .similarity,
            Some(1.0)
        );
    }
}
