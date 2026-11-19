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
    fn tts(&mut self) -> Result<&Arc<SherpaKokoroEngine>> {
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
        self.tts.as_ref().ok_or("TTS did not load".into())
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
        let mut output: Option<Audio> = None;
        self.synthesize_stream(text, cancel, &mut |audio| {
            let accumulated = output.get_or_insert_with(|| Audio { samples: Vec::new(), sample_rate: audio.sample_rate });
            if accumulated.sample_rate != audio.sample_rate || accumulated.samples.len().saturating_add(audio.samples.len()) > audio.sample_rate as usize * 120 {
                return Err("Reverse verification audio exceeds two minutes; use Read aloud or Export WAV for long text".into());
            }
            accumulated.samples.extend(audio.samples);
            Ok(())
        })?;
        let output = output.ok_or("No speech generated")?;
        output.validate(120)?;
        Ok(output)
    }
    fn synthesize_stream(
        &mut self,
        text: &str,
        cancel: &Cancel,
        emit: &mut dyn FnMut(Audio) -> Result<()>,
    ) -> Result<()> {
        check_cancel(cancel)?;
        let voice = self.settings.voice;
        let speed = self.settings.speed;
        let engine = self.tts()?;
        if !engine.voices().iter().any(|v| v.id == voice) {
            return Err("Voice ID is absent from this model".into());
        }
        for segment in tinytts::text_segmenter::segment(text) {
            check_cancel(cancel)?;
            let chunk = engine
                .synthesize(&segment, voice, speed, cancel)
                .map_err(|e| e.to_string())?;
            let audio = Audio {
                samples: chunk.samples,
                sample_rate: chunk.sample_rate,
            };
            audio.validate(120)?;
            emit(audio)?;
        }
        check_cancel(cancel)
    }
    fn voices(&mut self) -> Result<Vec<siny_core::Voice>> {
        Ok(self
            .tts()?
            .voices()
            .into_iter()
            .map(|voice| siny_core::Voice {
                id: voice.id,
                name: voice.name,
            })
            .collect())
    }
    fn unload(&mut self) {
        self.asr = None;
        self.tts = None;
    }
}
