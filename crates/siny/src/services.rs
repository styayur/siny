// SPDX-License-Identifier: AGPL-3.0-only
//! One owner for device capture/playback, clipboard and shortcuts.
use siny_core::{Audio, Result};
use tinystt::{
    audio::AudioCapture,
    hotkey::{GlobalHotkey, HotkeySpec},
};
use tinytts::audio::player::Player;

#[derive(Default)]
pub struct Devices {
    pub recording: Option<AudioCapture>,
    pub player: Option<Player>,
}
impl Devices {
    pub fn record(&mut self, microphone: &str) -> Result<()> {
        self.stop_playback(); // Half-duplex avoids speaker feedback without pretending to implement AEC.
        self.recording = Some(AudioCapture::start(Some(microphone)).map_err(|e| e.to_string())?);
        Ok(())
    }
    pub fn finish(&mut self) -> Option<Audio> {
        self.recording.take().map(|capture| {
            let result = capture.stop();
            Audio {
                samples: result.samples,
                sample_rate: result.sample_rate,
            }
        })
    }
    pub fn cancel(&mut self) {
        if let Some(capture) = self.recording.take() {
            capture.cancel();
        }
        self.stop_playback();
    }
    pub fn stop_playback(&mut self) {
        if let Some(player) = self.player.take() {
            player.stop();
        }
    }
    pub fn play(&mut self, audio: Audio) -> Result<()> {
        if self.recording.is_some() {
            return Err("Finish recording before playback".into());
        }
        self.stop_playback();
        audio.validate(120)?;
        let player = Player::new().map_err(|e| e.to_string())?;
        let (sender, receiver) = tinytts::audio::queue::bounded(1);
        player.start_feeding(receiver);
        sender
            .send(tinytts::tts::AudioChunk {
                samples: audio.samples,
                sample_rate: audio.sample_rate,
            })
            .map_err(|e| e.to_string())?;
        self.player = Some(player);
        Ok(())
    }
}
#[derive(Debug, Clone, Copy)]
pub enum ShortcutEvent {
    DictationPressed,
    DictationReleased,
    ReadClipboard,
}
pub struct Shortcuts {
    receiver: std::sync::mpsc::Receiver<ShortcutEvent>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl Shortcuts {
    pub fn register(ctx: eframe::egui::Context) -> Result<Self> {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc,
        };
        use std::time::Duration;
        let (sender, receiver) = mpsc::sync_channel(16);
        let (ready, ready_rx) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        std::thread::Builder::new()
            .name("siny-shortcuts".into())
            .spawn(move || {
                let registered = (|| -> Result<_> {
                    let dictation = GlobalHotkey::register(
                        HotkeySpec::parse("Ctrl+Alt+D").map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
                    let reading = tinytts::hotkey::GlobalHotkey::register()
                        .ok_or("Cannot register Ctrl+Alt+S")?;
                    Ok((dictation, reading))
                })();
                let (dictation, reading) = match registered {
                    Ok(keys) => {
                        let _ = ready.send(Ok(()));
                        keys
                    }
                    Err(e) => {
                        let _ = ready.send(Err(e));
                        return;
                    }
                };
                while !stopping.load(Ordering::Acquire) {
                    let event = dictation.try_recv().map(|e| match e {
                        tinystt::hotkey::HotkeyEvent::Pressed => ShortcutEvent::DictationPressed,
                        tinystt::hotkey::HotkeyEvent::Released => ShortcutEvent::DictationReleased,
                    });
                    if let Some(event) = event {
                        let _ = sender.try_send(event);
                        ctx.request_repaint();
                    }
                    if reading.try_recv().is_some() {
                        let _ = sender.try_send(ShortcutEvent::ReadClipboard);
                        ctx.request_repaint();
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            })
            .map_err(|e| e.to_string())?;
        ready_rx
            .recv_timeout(Duration::from_secs(3))
            .map_err(|e| e.to_string())??;
        Ok(Self { receiver, stop })
    }
    pub fn poll(&self) -> Option<ShortcutEvent> {
        self.receiver.try_recv().ok()
    }
}
impl Drop for Shortcuts {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
    }
}
pub fn read_clipboard() -> Result<Option<String>> {
    tinytts::clipboard::read_text().map_err(|e| e.to_string())
}
pub fn write_clipboard(text: &str) -> Result<()> {
    tinystt::clipboard::write_text(text).map_err(|e| e.to_string())
}
