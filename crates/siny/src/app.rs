// SPDX-License-Identifier: AGPL-3.0-only
// Intent: a Windows reader/dictator moves between voice and editable text without leaving one page.
// Hierarchy: the transcript is the focal plane; primary record/read controls sit directly above it.
// Domain: microphone, transcript, playhead, monitor, pronunciation, take, silence.
// Color world: paper, graphite, silver microphone mesh, blue monitor lamp, red record lamp.
// Signature: Listen -> Text -> Speak strip, paired record/read shortcuts, reverse transcript beside source.
// Avoid decorative cards, dashboard metrics and chat bubbles; use the existing accessible egui controls.
// Palette/depth: neutral paper/graphite surfaces, blue action accent, red only for live capture.
// Surfaces: egui light neutral base, inset editor, subtle outlines only. Type: system fonts for CJK;
// 28px title, 18px group headings, 14px body, 12px status. Spacing: 4px unit, 12px controls, 16px sections.
use crate::{
    backend::SherpaBackend,
    services::{self, Devices, Shortcuts},
};
use eframe::egui;
use siny_core::{Audio, Job, Operation, Scheduler, Settings};
use std::{path::PathBuf, time::Duration};

pub struct SinyApp {
    base: PathBuf,
    settings: Settings,
    scheduler: Option<Scheduler>,
    job: Option<Job>,
    devices: Devices,
    shortcuts: Option<Shortcuts>,
    text: String,
    status: String,
    reverse: String,
    read_after: bool,
    last_audio: Option<Audio>,
    show_settings: bool,
}
impl SinyApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(12.0, 12.0);
        style.spacing.interact_size.y = 40.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        cc.egui_ctx.set_style(style);
        let base = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."));
        let loaded = Settings::load(&base.join("config/siny.toml"));
        let status = loaded
            .as_ref()
            .err()
            .map(|e| format!("Configuration error: {e}. Correct or save settings before use."))
            .unwrap_or_else(|| "Ready · models load when needed".into());
        let settings = loaded.clone().unwrap_or_default();
        let scheduler = if loaded.is_ok() {
            make_scheduler(&base, &settings).ok()
        } else {
            None
        };
        let shortcuts = Shortcuts::register();
        let status = match &shortcuts {
            Ok(_) => status,
            Err(e) => format!("{status} · shortcuts: {e}"),
        };
        Self {
            base,
            settings,
            scheduler,
            job: None,
            devices: Devices::default(),
            shortcuts: shortcuts.ok(),
            text: String::new(),
            status,
            reverse: String::new(),
            read_after: false,
            last_audio: None,
            show_settings: false,
        }
    }
    fn submit(&mut self, operation: Operation) {
        if self.job.is_some() {
            self.status = "Finish or cancel the current task".into();
            return;
        }
        self.devices.stop_playback();
        match self
            .scheduler
            .as_ref()
            .ok_or("Save valid settings first".into())
            .and_then(|s| s.submit(operation))
        {
            Ok(job) => {
                self.job = Some(job);
                self.status = "Working locally…".into();
            }
            Err(e) => self.status = e,
        }
    }
    fn record(&mut self) {
        if self.job.is_some() || self.devices.recording.is_some() {
            return;
        }
        self.last_audio = None;
        match self.devices.record(&self.settings.microphone) {
            Ok(()) => self.status = "Recording · release shortcut or Finish".into(),
            Err(e) => self.status = e,
        }
    }
    fn finish(&mut self) {
        if let Some(audio) = self.devices.finish() {
            if audio.seconds() < 0.25 {
                self.status = "Recording shorter than 250 ms was discarded".into();
                return;
            }
            self.submit(if self.read_after {
                Operation::DictateThenRead(audio)
            } else {
                Operation::Dictate(audio)
            });
        }
    }
    fn stop(&mut self) {
        self.devices.cancel();
        if let Some(job) = &self.job {
            job.cancel();
            self.status = "Cancelling · waiting for native work to return".into();
        } else {
            self.status = "Stopped".into();
        }
    }
    fn clipboard_read(&mut self) {
        if self.devices.recording.is_some() || self.job.is_some() {
            return;
        }
        match services::read_clipboard() {
            Ok(Some(text)) => {
                self.text = text.clone();
                self.submit(Operation::Read(text));
            }
            Ok(None) => self.status = "Clipboard contains no text".into(),
            Err(e) => self.status = e,
        }
    }
    fn poll(&mut self) {
        if let Some(job) = &self.job {
            match job.result.try_recv() {
                Ok(result) => {
                    let cancelled = job.cancel.load(std::sync::atomic::Ordering::Acquire);
                    self.job = None;
                    if cancelled {
                        self.status = "Cancelled; ready for another task".into();
                        return;
                    }
                    match result {
                        Ok(outcome) => {
                            self.text = outcome.text;
                            self.status =
                                format!("Completed locally in {:.2} s", outcome.elapsed_seconds);
                            self.reverse = outcome
                                .recognized
                                .map(|s| {
                                    format!(
                                        "{} · similarity {:.0}% (diagnostic)",
                                        s,
                                        outcome.similarity.unwrap_or(0.0) * 100.0
                                    )
                                })
                                .unwrap_or_default();
                            self.last_audio = outcome.audio;
                            if let Some(audio) = &self.last_audio {
                                if let Err(e) = self.devices.play(audio.clone()) {
                                    self.status =
                                        format!("Speech generated; playback unavailable: {e}");
                                }
                            }
                        }
                        Err(e) => self.status = e,
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => (),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.job = None;
                    self.status = "Worker stopped unexpectedly; restart Siny".into();
                }
            }
        }
        let dictation = self.shortcuts.as_ref().and_then(|s| s.dictation.try_recv());
        match dictation {
            Some(tinystt::hotkey::HotkeyEvent::Pressed) => self.record(),
            Some(tinystt::hotkey::HotkeyEvent::Released) => self.finish(),
            _ => (),
        }
        if self
            .shortcuts
            .as_ref()
            .and_then(|s| s.reading.try_recv())
            .is_some()
        {
            self.clipboard_read();
        }
        if let Some(capture) = &self.devices.recording {
            if let Some(event) = capture.poll_event() {
                match event {
                    tinystt::audio::CaptureEvent::MaxReached => self.finish(),
                    tinystt::audio::CaptureEvent::Error(e) => {
                        self.devices.cancel();
                        self.status = format!("Microphone: {e}");
                    }
                }
            }
        }
        if self.devices.player.as_ref().is_some_and(|p| p.is_empty()) {
            self.devices.player = None;
        }
    }
}
fn make_scheduler(base: &std::path::Path, settings: &Settings) -> siny_core::Result<Scheduler> {
    Scheduler::new(
        SherpaBackend::new(base, settings.clone())?,
        settings.clone(),
    )
}
impl eframe::App for SinyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        // Native hotkeys are polled at 10 Hz; no per-frame inference or idle model loading.
        ctx.request_repaint_after(Duration::from_millis(100));
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| { ui.heading(egui::RichText::new("Siny").size(28.0)); ui.label("LISTEN  →  TEXT  →  SPEAK"); });
            ui.label("Offline speech, one place for both directions.");
            ui.add_space(4.0);
            let idle = self.job.is_none() && self.devices.recording.is_none();
            ui.horizontal_wrapped(|ui| {
                if self.devices.recording.is_some() {
                    if ui.button("Finish recording").clicked() { self.finish(); }
                } else if ui.add_enabled(idle, egui::Button::new("Record · Ctrl+Alt+D")).clicked() { self.record(); }
                if ui.add_enabled(idle && !self.text.trim().is_empty(), egui::Button::new("Read aloud")).clicked() { self.submit(Operation::Read(self.text.clone())); }
                if ui.button("Stop / Cancel").clicked() { self.stop(); }
                ui.checkbox(&mut self.read_after, "Read after dictation");
            });
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(idle, egui::Button::new("Read clipboard · Ctrl+Alt+S")).clicked() { self.clipboard_read(); }
                if ui.add_enabled(idle && !self.text.trim().is_empty(), egui::Button::new("Verify speech → text")).clicked() { self.submit(Operation::Verify(self.text.clone())); }
                if ui.button("Copy text").clicked() { if let Err(e) = services::write_clipboard(&self.text) { self.status = e; } }
                if let Some(player) = &self.devices.player {
                    if ui.button(if player.is_paused() { "Resume" } else { "Pause" }).clicked() { if player.is_paused() { player.resume(); } else { player.pause(); } }
                }
            });
            ui.add(egui::TextEdit::multiline(&mut self.text).desired_width(f32::INFINITY).desired_rows(10).hint_text("Type or dictate. Nothing is uploaded."));
            if !self.reverse.is_empty() { ui.label(&self.reverse); }
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(self.last_audio.is_some(), egui::Button::new("Save WAV…")).clicked() {
                    // Native dialog and export are explicit; no automatic recording export.
                    if let Some(path) = rfd::FileDialog::new().add_filter("Wave audio", &["wav"]).set_file_name("siny.wav").save_file() {
                        if let Some(audio) = &self.last_audio { self.status = match tinytts::audio::wav::write_wav(&path, &audio.samples, audio.sample_rate) { Ok(()) => "WAV saved".into(), Err(e) => e.to_string() }; }
                    }
                }
                if ui.add_enabled(idle, egui::Button::new("Unload models")).clicked() { self.last_audio = None; self.submit(Operation::Unload); }
                ui.toggle_value(&mut self.show_settings, "Settings & models");
            });
            ui.label(egui::RichText::new(&self.status).strong());
            if self.show_settings {
                ui.separator();
                ui.label("Model paths are relative to Siny.exe unless absolute. No runtime downloads.");
                ui.horizontal(|ui| { ui.label("SenseVoice"); ui.text_edit_singleline(&mut self.settings.sensevoice_dir); });
                ui.horizontal(|ui| { ui.label("Kokoro"); ui.text_edit_singleline(&mut self.settings.kokoro_dir); });
                ui.horizontal(|ui| { ui.label("Microphone (blank = default)"); ui.text_edit_singleline(&mut self.settings.microphone); });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Voice ID"); ui.add(egui::DragValue::new(&mut self.settings.voice).range(0..=53));
                    ui.label("Speed"); ui.add(egui::Slider::new(&mut self.settings.speed, 0.5..=2.0));
                    ui.label("Threads"); egui::ComboBox::from_id_salt("threads").selected_text(self.settings.threads.to_string()).show_ui(ui, |ui| { for n in [1,2,4] { ui.selectable_value(&mut self.settings.threads,n,n.to_string()); } });
                });
                if ui.add_enabled(idle, egui::Button::new("Save & apply")).clicked() {
                    match self.settings.save(&self.base.join("config/siny.toml")).and_then(|()| make_scheduler(&self.base, &self.settings)) {
                        Ok(scheduler) => { self.scheduler = Some(scheduler); self.status = "Settings saved · models load on demand".into(); }
                        Err(e) => self.status = e,
                    }
                }
                ui.label("Shared terminology, language and idle timeout: config/siny.toml. Voice names: model voices.txt / notices.");
            }
            ui.add_space(4.0);
            ui.small("Siny contributors · no warranty · AGPL-3.0-only; TinyTTS portions GPL-3.0-or-later.");
            ui.hyperlink_to("Source & licences", "https://github.com/styayur/siny");
        });
    }
}
impl Drop for SinyApp {
    fn drop(&mut self) {
        self.stop();
    }
}
