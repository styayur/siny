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
