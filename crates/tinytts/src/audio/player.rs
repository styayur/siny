//! Audio playback via rodio.
//!
//! A single [`Player`] owns the output stream and a [`rodio::Sink`]. The TTS
//! worker pushes chunks through the bounded queue, a feeder thread appends them
//! to the sink, and the sink plays them back in order.

use std::num::NonZero;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rodio::buffer::SamplesBuffer;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player as RodioPlayer};

use crate::error::{Result, TinyTtsError};
use crate::tts::AudioChunk;

pub struct Player {
    _stream: MixerDeviceSink,
    sink: Arc<RodioPlayer>,
    producer_done: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
}

impl Player {
    /// Open the default output device and create a ready-to-play sink.
    pub fn new() -> Result<Self> {
        let mut stream = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| TinyTtsError::AudioDeviceUnavailable(e.to_string()))?;
        stream.log_on_drop(false);
        let sink = RodioPlayer::connect_new(stream.mixer());
        Ok(Self {
            _stream: stream,
            sink: Arc::new(sink),
            producer_done: Arc::new(AtomicBool::new(true)),
            stopped: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Spawn a feeder thread that moves chunks from the bounded queue into the
    /// sink. The thread exits when the queue is disconnected (producer done).
    pub fn start_feeding(
        &self,
        rx: impl IntoIterator<Item = AudioChunk> + Send + 'static,
    ) -> Result<()> {
        let sink = Arc::clone(&self.sink);
        let done = self.producer_done.clone();
        let stopped = self.stopped.clone();
        self.producer_done.store(false, Ordering::Release);
        std::thread::Builder::new()
            .name("tinytts-player".to_string())
            .spawn(move || {
                for chunk in rx {
                    // A bounded channel alone is insufficient: rodio's sink also needs backpressure.
                    while sink.len() >= 2 && !stopped.load(Ordering::Acquire) {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    if stopped.load(Ordering::Acquire) {
                        break;
                    }
                    let Some(sample_rate) = NonZero::new(chunk.sample_rate) else {
                        continue;
                    };
                    let source =
                        SamplesBuffer::new(NonZero::<u16>::MIN, sample_rate, chunk.samples);
                    sink.append(source);
                }
                done.store(true, Ordering::Release);
            })
            .map_err(|error| {
                self.producer_done.store(true, Ordering::Release);
                TinyTtsError::AudioDeviceUnavailable(error.to_string())
            })?;
        Ok(())
    }

    pub fn pause(&self) {
        self.sink.pause();
    }

    pub fn resume(&self) {
        self.sink.play();
    }

    pub fn is_paused(&self) -> bool {
        self.sink.is_paused()
    }

    /// Stop playback and clear any queued sounds.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        self.sink.stop();
    }

    /// Number of chunks still queued on the sink.
    pub fn pending(&self) -> usize {
        self.sink.len()
    }

    /// True once the sink has finished playing everything it received.
    pub fn is_empty(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
            || (self.producer_done.load(Ordering::Acquire) && self.sink.empty())
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::queue;
    use crate::tts::AudioChunk;
    use std::f32::consts::TAU;
    use std::time::{Duration, Instant};

    #[test]
    #[ignore = "requires a real audio output device"]
    fn default_output_plays_pause_resume_and_clears() -> Result<()> {
        let player = Player::new()?;
        player.pause();
        assert!(player.is_paused());
        player.resume();
        assert!(!player.is_paused());

        let (tx, rx) = queue::bounded(1);
        player.start_feeding(rx)?;
        let sample_rate = 24_000;
        let samples = (0..sample_rate / 4)
            .map(|index| (TAU * 440.0 * index as f32 / sample_rate as f32).sin() * 0.05)
            .collect();
        tx.send(AudioChunk {
            samples,
            sample_rate,
        })
        .expect("audio queue receiver must remain active");
        drop(tx);

        let deadline = Instant::now() + Duration::from_secs(5);
        while !player.is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            player.is_empty(),
            "rodio player did not drain the queued audio"
        );
        player.stop();
        Ok(())
    }
}
