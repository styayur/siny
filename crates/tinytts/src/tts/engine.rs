//! UI-facing TTS abstraction.
//!
//! The GUI only talks to [`TtsEngine`]. It never depends on sherpa-onnx types
//! directly, which keeps the door open for other backends in the future.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::error::Result;

/// Canonical speaker names for `kokoro-multi-lang-v1_0` (index order matches
/// `voices.bin`). Used only as a fallback when `voices.txt` is absent.
pub const KOKORO_VOICES: &[&str] = &[
    "af_alloy",
    "af_aoede",
    "af_bella",
    "af_heart",
    "af_jessica",
    "af_kore",
    "af_nicole",
    "af_nova",
    "af_river",
    "af_sarah",
    "af_sky",
    "am_adam",
    "am_echo",
    "am_eric",
    "am_fenrir",
    "am_liam",
    "am_michael",
    "am_onyx",
    "am_puck",
    "am_santa",
    "bf_alice",
    "bf_emma",
    "bf_isabella",
    "bf_lily",
    "bm_daniel",
    "bm_fable",
    "bm_george",
    "bm_lewis",
    "ef_dora",
    "em_alex",
    "ff_siwis",
    "hf_alpha",
    "hf_beta",
    "hm_omega",
    "hm_psi",
    "if_sara",
    "im_nicola",
    "jf_alpha",
    "jf_gongitsune",
    "jf_nezumi",
    "jf_tebukuro",
    "jm_kumo",
    "pf_dora",
    "pm_alex",
    "pm_santa",
    "zf_xiaobei",
    "zf_xiaoni",
    "zf_xiaoxiao",
    "zf_xiaoyi",
    "zm_yunjian",
    "zm_yunxi",
    "zm_yunxia",
    "zm_yunyang",
    "em_santa",
];

/// A selectable speaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voice {
    pub id: i32,
    pub name: String,
}

/// Read optional model voice names without loading weights. Keep original speaker order and bound metadata allocation.
pub fn voice_catalog(path: &std::path::Path, count: usize) -> Vec<Voice> {
    use std::io::Read;
    let count = count.min(1024);
    if let Ok(file) = std::fs::File::open(path) {
        let mut text = String::new();
        if file.take(1_048_577).read_to_string(&mut text).is_ok() && text.len() <= 1_048_576 {
            let names: Vec<Voice> = text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .take(count)
                .enumerate()
                .map(|(id, name)| Voice {
                    id: id as i32,
                    name: name.into(),
                })
                .collect();
            if !names.is_empty() {
                return names;
            }
        }
    }
    KOKORO_VOICES
        .iter()
        .take(count)
        .enumerate()
        .map(|(id, name)| Voice {
            id: id as i32,
            name: (*name).into(),
        })
        .collect()
}

/// One chunk of synthesized audio.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl AudioChunk {
    pub fn duration_secs(&self) -> f32 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.samples.len() as f32 / self.sample_rate as f32
        }
    }
}

/// A minimal, synchronous TTS engine interface.
pub trait TtsEngine: Send + Sync {
    fn sample_rate(&self) -> u32;
    fn voices(&self) -> Vec<Voice>;
    fn synthesize(
        &self,
        text: &str,
        voice_id: i32,
        speed: f32,
        cancel: &Arc<AtomicBool>,
    ) -> Result<AudioChunk>;
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    #[test]
    fn model_names_keep_speaker_order_and_oversized_metadata_is_bounded() {
        let path =
            std::env::temp_dir().join(format!("tinytts-voice-catalog-{}.txt", std::process::id()));
        std::fs::write(&path, "custom_zero\n\ncustom_one\n").unwrap();
        let voices = voice_catalog(&path, 54);
        assert_eq!(voices[0].name, "custom_zero");
        assert_eq!(voices[1].name, "custom_one");
        assert_eq!(voices[1].id, 1);
        std::fs::write(&path, vec![b'x'; 1_048_577]).unwrap();
        let fallback = voice_catalog(&path, 54);
        assert_eq!(fallback.len(), 54);
        assert_eq!(fallback[3].name, "af_heart");
        std::fs::remove_file(path).unwrap();
    }
}
