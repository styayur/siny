// SPDX-License-Identifier: AGPL-3.0-only
//! Adapters keep native types outside the shared service API.
use siny_core::{tasks::check_cancel, Audio, Backend, Cancel, Result, Settings};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tinystt::asr::{sensevoice::SenseVoiceRecognizer, SpeechRecognizer};
use tinytts::tts::{SherpaKokoroEngine, TtsEngine};

pub struct SherpaBackend {
    base: PathBuf,
    settings: Settings,
    asr: Option<SenseVoiceRecognizer>,
    tts: Option<Arc<SherpaKokoroEngine>>,
}
impl SherpaBackend {
    pub fn new(base: impl Into<PathBuf>, settings: Settings) -> Result<Self> {
        settings.validate()?;
        Ok(Self {
            base: base.into(),
            settings,
            asr: None,
            tts: None,
        })
    }
    fn path(&self, value: &str) -> PathBuf {
        let path = Path::new(value);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base.join(path)
        }
    }
    pub fn loaded(&self) -> (bool, bool) {
        (self.asr.is_some(), self.tts.is_some())
    }
}
impl Backend for SherpaBackend {
    fn recognize(&mut self, audio: &Audio, cancel: &Cancel) -> Result<String> {
        check_cancel(cancel)?;
        // File CLI can recognize a bounded one-hour file; scheduler still bounds microphone/reverse jobs.
        audio.validate(3600)?;
        if self.asr.is_none() {
            let paths = tinystt::paths::Paths::new(&self.base);
            let settings = tinystt::settings::Settings {
                model_dir: self
                    .path(&self.settings.sensevoice_dir)
                    .display()
                    .to_string(),
                ..Default::default()
            };
            let model = paths.resolve_model(&settings).map_err(|e| e.to_string())?;
            self.asr = Some(
                SenseVoiceRecognizer::new(&model, self.settings.threads, &self.settings.language)
                    .map_err(|e| e.to_string())?,
            );
        }
        check_cancel(cancel)?;
        // Let sherpa's bandlimited native resampler accept the actual source rate.
        let result = self
            .asr
            .as_mut()
            .expect("loaded ASR")
            .recognize(&audio.samples, audio.sample_rate)
            .map_err(|e| e.to_string())?;
        check_cancel(cancel)?;
        tinystt::asr::clean_transcript(&result).map_err(|e| e.to_string())
    }
    fn synthesize(&mut self, text: &str, cancel: &Cancel) -> Result<Audio> {
        check_cancel(cancel)?;
        if self.tts.is_none() {
            let paths = tinytts::paths::Paths::new(&self.base);
            let model = paths
                .model_files(Some(&self.path(&self.settings.kokoro_dir)))
                .map_err(|e| e.to_string())?;
            self.tts = Some(Arc::new(
                SherpaKokoroEngine::new(&model, self.settings.threads)
                    .map_err(|e| e.to_string())?,
            ));
        }
        let engine = self.tts.as_ref().expect("loaded TTS");
        if !engine.voices().iter().any(|v| v.id == self.settings.voice) {
            return Err("Voice ID is absent from this model".into());
        }
        let mut output = Audio {
            samples: Vec::new(),
            sample_rate: engine.sample_rate(),
        };
        for segment in tinytts::text_segmenter::segment(text) {
            check_cancel(cancel)?;
            let chunk = engine
                .synthesize(&segment, self.settings.voice, self.settings.speed, cancel)
                .map_err(|e| e.to_string())?;
            if chunk.sample_rate != output.sample_rate
                || output.samples.len().saturating_add(chunk.samples.len())
                    > output.sample_rate as usize * 120
            {
                return Err("Speech exceeds two-minute output limit; split the text".into());
            }
            output.samples.extend(chunk.samples);
        }
        check_cancel(cancel)?;
        output.validate(120)?;
        Ok(output)
    }
    fn unload(&mut self) {
        self.asr = None;
        self.tts = None;
    }
}
