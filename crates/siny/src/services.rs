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
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Shortcuts {
    pub fn register(ctx: eframe::egui::Context, hotkey: &str) -> Result<Self> {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc,
        };
        use std::time::Duration;
        let (sender, receiver) = mpsc::sync_channel(16);
        let (ready, ready_rx) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let hotkey = HotkeySpec::parse(hotkey).map_err(|e| e.to_string())?;
        let worker = std::thread::Builder::new()
            .name("siny-shortcuts".into())
            .spawn(move || {
                let registered = (|| -> Result<_> {
                    let dictation = GlobalHotkey::register(hotkey).map_err(|e| e.to_string())?;
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
        match ready_rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => (),
            Ok(Err(error)) => {
                stop.store(true, Ordering::Release);
                return Err(error);
            }
            Err(error) => {
                stop.store(true, Ordering::Release);
                return Err(error.to_string());
            }
        }
        Ok(Self {
            receiver,
            stop,
            worker: Some(worker),
        })
    }
    pub fn poll(&self) -> Option<ShortcutEvent> {
        self.receiver.try_recv().ok()
    }
}
impl Drop for Shortcuts {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn read_clipboard() -> Result<Option<String>> {
    tinytts::clipboard::read_text().map_err(|e| e.to_string())
}
pub fn write_clipboard(text: &str) -> Result<()> {
    tinystt::clipboard::write_text(text).map_err(|e| e.to_string())
}

/// A foreground destination observed when dictation starts. Never activates another application.
#[derive(Clone, Copy)]
pub struct PasteTarget {
    window: isize,
    process: u32,
}
impl PasteTarget {
    pub fn capture() -> Option<Self> {
        #[cfg(windows)]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                GetForegroundWindow, GetWindowThreadProcessId,
            };
            let mut process = 0;
            // These HWND values are opaque OS handles. The PID output points to a live local u32.
            let window = unsafe { GetForegroundWindow() };
            if window.is_null() {
                return None;
            }
            unsafe {
                GetWindowThreadProcessId(window, &mut process);
            }
            if process != 0 && process != std::process::id() {
                return Some(Self {
                    window: window as isize,
                    process,
                });
            }
        }
        None
    }
    fn still_foreground(self) -> bool {
        #[cfg(windows)]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                GetForegroundWindow, GetWindowThreadProcessId, IsWindow,
            };
            let window = self.window as *mut std::ffi::c_void;
            let mut process = 0;
            // Validate the captured opaque HWND before comparing it with the current foreground window.
            unsafe {
                GetWindowThreadProcessId(window, &mut process);
                IsWindow(window) != 0 && process == self.process && GetForegroundWindow() == window
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (self.window, self.process);
            false
        }
    }
}

pub fn deliver_dictation(
    text: &str,
    auto_paste: bool,
    target: Option<PasteTarget>,
) -> Result<&'static str> {
    write_clipboard(text)?;
    if auto_paste {
        if target.is_some_and(PasteTarget::still_foreground) {
            tinystt::paste::send_ctrl_v().map_err(|e| e.to_string())?;
            Ok("Copied and pasted to the dictation destination")
        } else {
            Ok("Copied; paste skipped because the destination changed or is Siny")
        }
    } else {
        Ok("Dictation copied to clipboard")
    }
}
