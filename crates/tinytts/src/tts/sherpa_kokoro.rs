//! sherpa-onnx backed Kokoro TTS engine.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig,
    OfflineTtsModelConfig,
};

use super::engine::{voice_catalog, AudioChunk, TtsEngine, Voice};
use crate::error::{Result, TinyTtsError};
use crate::paths::ModelPaths;

/// Kokoro TTS via the official sherpa-onnx Rust API.
pub struct SherpaKokoroEngine {
    tts: Arc<OfflineTts>,
    voices: Vec<Voice>,
    sample_rate: u32,
}

impl SherpaKokoroEngine {
    pub fn new(model: &ModelPaths, num_threads: u8) -> Result<Self> {
        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                kokoro: OfflineTtsKokoroModelConfig {
                    model: Some(model.model.display().to_string()),
                    voices: Some(model.voices.display().to_string()),
                    tokens: Some(model.tokens.display().to_string()),
                    data_dir: Some(model.data_dir.display().to_string()),
                    dict_dir: model.dict_dir.as_ref().map(|p| p.display().to_string()),
                    lexicon: model.lexicon.clone(),
                    length_scale: 1.0,
                    ..Default::default()
                },
                num_threads: num_threads as i32,
                debug: false,
                ..Default::default()
            },
            rule_fsts: model.rule_fsts.clone(),
            ..Default::default()
        };

        let tts = OfflineTts::create(&config).ok_or_else(|| {
            TinyTtsError::ModelLoadFailed(format!(
                "could not initialize Kokoro from {}",
                model.model.display()
            ))
        })?;

        let sample_rate = tts.sample_rate() as u32;
        let voices = voice_catalog(&model.voices_txt, tts.num_speakers().max(0) as usize);

        Ok(Self {
            tts: Arc::new(tts),
            voices,
            sample_rate,
        })
    }
}

impl TtsEngine for SherpaKokoroEngine {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn voices(&self) -> Vec<Voice> {
        self.voices.clone()
    }

    fn synthesize(
        &self,
        text: &str,
        voice_id: i32,
        speed: f32,
        cancel: &Arc<AtomicBool>,
    ) -> Result<AudioChunk> {
        if cancel.load(Ordering::Relaxed) {
            return Err(TinyTtsError::Cancelled);
        }
        let generation = GenerationConfig {
            sid: voice_id,
            speed,
            ..Default::default()
        };

        let audio = self.tts.generate_with_config(
            text,
            &generation,
            Some({
                let cancel = Arc::clone(cancel);
                move |_samples: &[f32], _progress: f32| !cancel.load(Ordering::Relaxed)
            }),
        );

        // Native cancellation may return a partial buffer. Never expose it as
        // a successfully completed chunk after Stop has cancelled the request.
        if cancel.load(Ordering::Relaxed) {
            return Err(TinyTtsError::Cancelled);
        }
        match audio {
            Some(a) => Ok(AudioChunk {
                samples: a.samples().to_vec(),
                sample_rate: a.sample_rate() as u32,
            }),
            None => {
                if cancel.load(Ordering::Relaxed) {
                    Err(TinyTtsError::Cancelled)
                } else {
                    let preview: String = text.chars().take(40).collect();
                    Err(TinyTtsError::GenerationFailed(preview))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn original_voice_ids_remain_stable_for_53_speaker_models() {
        let voices = voice_catalog(Path::new("missing-native-voice-fixture.txt"), 53);
        assert_eq!(voices.len(), 53);
        assert_eq!(voices[3].name, "af_heart");
        assert_eq!(voices[45].name, "zf_xiaobei");
        assert_eq!(voices[52].name, "zm_yunyang");
        assert_eq!(voices[52].id, 52);
    }

    #[test]
    fn reviewed_54th_speaker_is_exposed_without_shifting_existing_ids() {
        let voices = voice_catalog(Path::new("missing-native-voice-fixture.txt"), 54);
        assert_eq!(voices.len(), 54);
        assert_eq!(voices[3].name, "af_heart");
        assert_eq!(voices[45].id, 45);
        assert_eq!(voices[53].name, "em_santa");
        assert_eq!(voices[53].id, 53);
    }
}
