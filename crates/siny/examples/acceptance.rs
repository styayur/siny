// SPDX-License-Identifier: AGPL-3.0-only
//! Real opt-in acceptance: official sample ASR + self-authored speech round trips.
use serde::Serialize;
use siny::backend::SherpaBackend;
use siny_core::{Audio, Backend, Operation, Scheduler, Settings};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
#[derive(Serialize)]
struct Case {
    name: String,
    expected: String,
    recognized: String,
    similarity: f32,
    seconds: f32,
    elapsed_seconds: f64,
    rtf: f64,
}
#[derive(Serialize)]
struct Report {
    lazy_start: bool,
    unloaded: bool,
    cancelled: bool,
    recovered: bool,
    cases: Vec<Case>,
}
#[repr(C)]
struct OrtApiBase {
    get_api: unsafe extern "system" fn(u32) -> *const std::ffi::c_void,
    version: unsafe extern "system" fn() -> *const std::ffi::c_char,
}
extern "system" {
    fn OrtGetApiBase() -> *const OrtApiBase;
}
fn runtime_version() -> String {
    // Stable ORT C API base layout; no borrowed pointer escapes the call.
    unsafe {
        std::ffi::CStr::from_ptr(((*OrtGetApiBase()).version)())
            .to_string_lossy()
            .into_owned()
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = runtime_version();
    println!("LINKED_ORT_VERSION={runtime}");
    if let Ok(expected) = std::env::var("SINY_EXPECT_ORT_VERSION") {
        assert_eq!(runtime, expected);
    }
    // Known 12 kHz input aliases at 4 kHz after 48 -> 16 kHz linear decimation.
    // Compare that deterministic fixture with the production native low-pass path.
    let signal: Vec<f32> = (0..48000)
        .map(|i| (std::f32::consts::TAU * 12000.0 * i as f32 / 48000.0).sin() * 0.5)
        .collect();
    let rms = |samples: &[f32]| {
        (samples.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
    };
    let linear = tinystt::audio::resample_offline(&signal, 48000, 16000)?;
    let filter =
        sherpa_onnx::LinearResampler::create(48000, 16000).ok_or("Cannot initialize filter")?;
    let started = Instant::now();
    let filtered = filter.resample(&signal, true);
    assert!(rms(&filtered[100..filtered.len() - 100]) < rms(&linear) * 0.01);
    println!(
        "RESAMPLING linear_rms={} filtered_rms={} cpu_wall_seconds={}",
        rms(&linear),
        rms(&filtered[100..filtered.len() - 100]),
        started.elapsed().as_secs_f64()
    );
    let settings = Settings {
        sensevoice_dir: std::env::var("SINY_TEST_ASR")?,
        kokoro_dir: std::env::var("TINY_TTS_TEST_MODEL")?,
        ..Default::default()
    };
    let mut backend = SherpaBackend::new(".", settings.clone())?;
    assert_eq!(backend.loaded(), (false, false));
    let cancel = Arc::new(AtomicBool::new(false));
    let mut report = Report {
        lazy_start: true,
        unloaded: false,
        cancelled: false,
        recovered: false,
        cases: Vec::new(),
    };
    let samples = tinystt::audio::read_wav_as_16k_mono(
        &PathBuf::from(&settings.sensevoice_dir).join("test_wavs/en.wav"),
    )?;
    let input = Audio {
        samples,
        sample_rate: 16000,
    };
    let started = Instant::now();
    let text = backend.recognize(&input, &cancel)?;
    let elapsed = started.elapsed().as_secs_f64();
    assert!(
        text.to_lowercase().contains("tribal chieftain"),
        "Unexpected official sample: {text}"
    );
    report.cases.push(Case {
        name: "official-en".into(),
        expected: "tribal chieftain (partial reference only)".into(),
        recognized: text,
        similarity: 0.0,
        seconds: input.seconds(),
        elapsed_seconds: elapsed,
        rtf: elapsed / input.seconds() as f64,
    });
    assert_eq!(backend.loaded(), (true, false));
    for (name, text, minimum) in [
        (
            "english",
            "This is an offline speech test. One, two, three.",
            0.85,
        ),
        (
            "unknown-names",
            "The product names are Quorvex and Zorblax.",
            0.0,
        ),
    ] {
        let started = Instant::now();
        let audio = backend.synthesize(text, &cancel)?;
        let recognized = backend.recognize(&audio, &cancel)?;
        let similarity = siny_core::terms::similarity(text, &recognized);
        assert!(
            similarity >= minimum,
            "Round trip failed: {text} -> {recognized} ({similarity})"
        );
        let elapsed = started.elapsed().as_secs_f64();
        report.cases.push(Case {
            name: name.into(),
            expected: text.into(),
            recognized,
            similarity,
            seconds: audio.seconds(),
            elapsed_seconds: elapsed,
            rtf: elapsed / audio.seconds() as f64,
        });
    }
    cancel.store(true, Ordering::Release);
    assert!(backend.recognize(&input, &cancel).is_err());
    assert!(backend
        .synthesize("This request was cancelled.", &cancel)
        .is_err());
    report.cancelled = true;
    cancel.store(false, Ordering::Release);
    assert!(!backend.recognize(&input, &cancel)?.is_empty());
    report.recovered = true;
    backend.unload();
    assert_eq!(backend.loaded(), (false, false));
    report.unloaded = true;
    // Shared workflows go through the same production scheduler, including reverse verification.
    let scheduler = Scheduler::new(backend, settings)?;
    let dictated = scheduler.submit(Operation::DictateThenRead(input.clone()))?;
    let dictated = dictated.result.recv_timeout(Duration::from_secs(180))??;
    assert!(dictated.text.to_lowercase().contains("tribal chieftain"));
    assert!(dictated
        .audio
        .as_ref()
        .is_some_and(|audio| audio.seconds() > 1.0));
    let cancelled_asr = scheduler.submit(Operation::Dictate(input.clone()))?;
    std::thread::sleep(Duration::from_millis(20));
    cancelled_asr.cancel();
    assert_eq!(
        cancelled_asr
            .result
            .recv_timeout(Duration::from_secs(60))?
            .unwrap_err(),
        "Cancelled"
    );
    let job = scheduler.submit(Operation::Verify("This is a local voice test.".into()))?;
    let outcome = job.result.recv_timeout(Duration::from_secs(180))??;
    assert!(
        outcome.similarity.unwrap_or_default() >= 0.85,
        "{:?}",
        outcome.recognized
    );
    let output = PathBuf::from("work/acceptance");
    std::fs::create_dir_all(&output)?;
    std::fs::write(
        output.join("siny-report.toml"),
        toml::to_string_pretty(&report)?,
    )?;
    println!("Actual SenseVoice/Kokoro inference and production verification passed");
    Ok(())
}
