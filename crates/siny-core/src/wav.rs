// SPDX-License-Identifier: AGPL-3.0-only
//! Incremental WAV export with cancellation and atomic publication; existing destination survives errors.
use crate::{tasks::check_cancel, Audio, Backend, Cancel, Result};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_EXPORT: AtomicU64 = AtomicU64::new(0);
pub fn export(backend: &mut impl Backend, text: &str, path: &Path, cancel: &Cancel) -> Result<()> {
    let temp = path.with_extension(format!(
        "wav.{}.{}.tmp",
        std::process::id(),
        NEXT_EXPORT.fetch_add(1, Ordering::Relaxed)
    ));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        let mut file = Some(file);
        let mut writer = None;
        let mut rate = None;
        let mut frames = 0_u64;
        backend.synthesize_stream(text, cancel, &mut |audio: Audio| {
            check_cancel(cancel)?;
            audio.validate(120)?;
            if let Some(expected) = rate {
                if audio.sample_rate != expected {
                    return Err("Synthesis changed sample rate during WAV export".into());
                }
            } else {
                rate = Some(audio.sample_rate);
                writer = Some(
                    hound::WavWriter::new(
                        std::io::BufWriter::with_capacity(
                            64 * 1024,
                            file.take().ok_or("WAV file already initialized")?,
                        ),
                        hound::WavSpec {
                            channels: 1,
                            sample_rate: audio.sample_rate,
                            bits_per_sample: 16,
                            sample_format: hound::SampleFormat::Int,
                        },
                    )
                    .map_err(|e| e.to_string())?,
                );
            }
            frames += audio.samples.len() as u64;
            if frames > (u32::MAX as u64 - 44) / 2 {
                return Err("WAV exceeds RIFF file size limit".into());
            }
            let writer = writer.as_mut().ok_or("No WAV writer")?;
            for sample in audio.samples {
                writer
                    .write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16)
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        })?;
        check_cancel(cancel)?;
        writer
            .ok_or("No speech generated for WAV export")?
            .finalize()
            .map_err(|e| e.to_string())?;
        std::fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
