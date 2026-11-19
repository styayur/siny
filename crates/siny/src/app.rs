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
    services::{self, Devices, PasteTarget, Shortcuts},
};
use eframe::egui;
use siny_core::{Audio, Job, Operation, Scheduler, Settings};
use std::{path::PathBuf, time::Duration};

pub struct SinyApp {
    base: PathBuf,
    settings: Settings,
    applied_settings: Settings,
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
    dictation_delivery: bool,
    paste_target: Option<PasteTarget>,
}
impl SinyApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        let mut style = (*cc.egui_ctx.style_of(egui::Theme::Light)).clone();
        style.spacing.item_spacing = egui::vec2(12.0, 12.0);
        style.spacing.interact_size.y = 40.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        cc.egui_ctx.set_style_of(egui::Theme::Light, style);
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
        let shortcuts = Shortcuts::register(cc.egui_ctx.clone(), &settings.hotkey);
        let status = match &shortcuts {
            Ok(_) => status,
            Err(e) => format!("{status} · shortcuts: {e}"),
        };
        Self {
            base,
            applied_settings: settings.clone(),
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
            dictation_delivery: false,
            paste_target: None,
        }
    }
    fn submit(&mut self, operation: Operation) {
        if self.job.is_some() {
            self.status = "Finish or cancel the current task".into();
            return;
        }
        self.devices.stop_playback();
        self.dictation_delivery = matches!(
            &operation,
            Operation::Dictate(_) | Operation::DictateThenRead(_)
        );
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
            Err(e) => {
                self.dictation_delivery = false;
                self.paste_target = None;
                self.status = e;
            }
        }
    }
    fn record(&mut self) {
        if self.job.is_some() || self.devices.recording.is_some() {
            return;
        }
        self.last_audio = None;
        self.paste_target = PasteTarget::capture();
        match self.devices.record(&self.applied_settings.microphone) {
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
        self.dictation_delivery = false;
        self.paste_target = None;
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
                        self.dictation_delivery = false;
                        self.paste_target = None;
                        self.status = "Cancelled; ready for another task".into();
                        return;
                    }
                    match result {
                        Ok(outcome) => {
                            if !outcome.text.is_empty() {
                                self.text = outcome.text;
                            }
                            self.status =
                                format!("Completed locally in {:.2} s", outcome.elapsed_seconds);
                            if self.dictation_delivery && !self.text.is_empty() {
                                match services::deliver_dictation(
                                    &self.text,
                                    self.applied_settings.auto_paste,
                                    self.paste_target.take(),
                                ) {
                                    Ok(message) => self.status.push_str(&format!(" · {message}")),
                                    Err(error) => {
                                        self.status = format!(
                                            "Dictation completed; clipboard/paste failed: {error}"
                                        )
                                    }
                                }
                            }
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
                    self.dictation_delivery = false;
                    self.paste_target = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => (),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.job = None;
                    self.status = "Worker stopped unexpectedly; restart Siny".into();
                }
            }
        }
        while let Some(event) = self.shortcuts.as_ref().and_then(Shortcuts::poll) {
            match event {
                services::ShortcutEvent::DictationPressed => self.record(),
                services::ShortcutEvent::DictationReleased => self.finish(),
                services::ShortcutEvent::ReadClipboard => self.clipboard_read(),
            }
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

    fn apply_settings(&mut self, ctx: &egui::Context) {
        let result = (|| -> siny_core::Result<_> {
            self.settings.validate()?;
            let spec = tinystt::hotkey::HotkeySpec::parse(&self.settings.hotkey)
                .map_err(|e| e.to_string())?;
            if spec.display == "Ctrl+Alt+S" {
                return Err("Dictation and clipboard shortcuts must differ".into());
            }
            self.settings.hotkey = spec.display;
            let scheduler = make_scheduler(&self.base, &self.settings)?;
            self.shortcuts = None; // Join the registration owner so old handles are released first.
            let shortcuts = Shortcuts::register(ctx.clone(), &self.settings.hotkey)?;
            self.settings.save(&self.base.join("config/siny.toml"))?;
            Ok((scheduler, shortcuts))
        })();
        match result {
            Ok((scheduler, shortcuts)) => {
                self.devices.stop_playback();
                self.scheduler = Some(scheduler);
                self.shortcuts = Some(shortcuts);
                self.applied_settings = self.settings.clone();
                self.status = "Settings saved and applied · models load on demand".into();
            }
            Err(error) => {
                if self.shortcuts.is_none() {
                    self.shortcuts =
                        Shortcuts::register(ctx.clone(), &self.applied_settings.hotkey).ok();
                }
                self.status = format!("Settings not applied: {error}");
            }
        }
    }

    fn import_settings(&mut self, app: crate::migration::LegacyApp) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("TOML configuration", &["toml"])
            .pick_file()
        {
            match crate::migration::import(&path, app, &self.settings) {
                Ok(candidate) => {
                    self.settings = candidate;
                    self.status = "Imported into settings preview · review paths, threads and auto-paste, then Save & apply. Legacy file unchanged.".into();
                }
                Err(error) => self.status = format!("Import rejected: {error}"),
            }
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
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root_ui.ctx().clone();
        self.poll();
        // Shortcuts wake egui directly. Idle windows do not redraw on a timer.
        if self.job.is_some() || self.devices.recording.is_some() || self.devices.player.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        egui::CentralPanel::default().show(root_ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| { ui.heading(egui::RichText::new("Siny").size(28.0)); ui.label("LISTEN  →  TEXT  →  SPEAK"); });
            ui.label("Offline speech, one place for both directions.");
            ui.add_space(4.0);
            let idle = self.job.is_none() && self.devices.recording.is_none();
            ui.horizontal_wrapped(|ui| {
                if self.devices.recording.is_some() {
                    if ui.button("Finish recording").clicked() { self.finish(); }
                } else if ui.add_enabled(idle, egui::Button::new(format!("Record · {}", self.applied_settings.hotkey))).clicked() { self.record(); }
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
                ui.horizontal(|ui| { ui.label("Dictation shortcut"); ui.text_edit_singleline(&mut self.settings.hotkey); });
                ui.checkbox(&mut self.settings.auto_paste, "Paste dictation into its original foreground application (off by default)");
                ui.label("Dictation is copied automatically. Paste is skipped if focus changes or the destination is Siny.");
                ui.horizontal_wrapped(|ui| {
                    ui.label("Recognition language");
                    egui::ComboBox::from_id_salt("language").selected_text(&self.settings.language).show_ui(ui, |ui| {
                        for language in ["auto", "zh", "en", "yue", "ja", "ko"] { ui.selectable_value(&mut self.settings.language, language.into(), language); }
                    });
                    ui.label("Unload after idle seconds"); ui.add(egui::DragValue::new(&mut self.settings.idle_unload_seconds).range(1..=3600));
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Voice ID"); ui.add(egui::DragValue::new(&mut self.settings.voice).range(0..=53));
                    ui.label("Speed"); ui.add(egui::Slider::new(&mut self.settings.speed, 0.5..=2.0));
                    ui.label("Threads"); egui::ComboBox::from_id_salt("threads").selected_text(self.settings.threads.to_string()).show_ui(ui, |ui| { for n in [1,2,4] { ui.selectable_value(&mut self.settings.threads,n,n.to_string()); } });
                });
                ui.collapsing("Shared terminology · exact case-sensitive replacement", |ui| {
                    let mut remove = None;
                    for (index, term) in self.settings.terms.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut term.from).hint_text("Recognized spelling").desired_width(180.0));
                            ui.label("→");
                            ui.add(egui::TextEdit::singleline(&mut term.to).hint_text("Preferred spelling").desired_width(180.0));
                            if ui.button("Remove").clicked() { remove = Some(index); }
                        });
                    }
                    if let Some(index) = remove { self.settings.terms.remove(index); }
                    if ui.add_enabled(self.settings.terms.len() < 256, egui::Button::new("Add term")).clicked() {
                        self.settings.terms.push(siny_core::terms::Term { from: String::new(), to: String::new() });
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(idle, egui::Button::new("Import TinySTT config…")).clicked() { self.import_settings(crate::migration::LegacyApp::TinyStt); }
                    if ui.add_enabled(idle, egui::Button::new("Import TinyTTS config…")).clicked() { self.import_settings(crate::migration::LegacyApp::TinyTts); }
                    if ui.add_enabled(idle, egui::Button::new("Save & apply")).clicked() { self.apply_settings(&ctx); }
                });
                ui.label("Settings changes remain a preview until Save & apply. Models reload on the next task; restart is unnecessary.");
            }
            ui.add_space(4.0);
            ui.small("Siny contributors · no warranty · AGPL-3.0-only; TinyTTS portions GPL-3.0-or-later.");
            ui.hyperlink_to("Source & licences", "https://github.com/styayur/siny");
            });
        });
    }
}
impl Drop for SinyApp {
    fn drop(&mut self) {
        self.stop();
    }
}
