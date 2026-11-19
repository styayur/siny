// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit local device probe. Does not recognize, export or retain microphone samples.
use serde::Serialize;
use std::time::{Duration, Instant};
#[derive(Serialize)]
struct Report {
    output_pause_resume_stop: bool,
    capture_cancel_restart: bool,
    frames: usize,
    output_error: String,
    capture_error: String,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut report = Report {
        output_pause_resume_stop: false,
        capture_cancel_restart: false,
        frames: 0,
        output_error: String::new(),
        capture_error: String::new(),
    };
    report.output_error = probe_output()
        .map(|()| {
            report.output_pause_resume_stop = true;
            String::new()
        })
        .unwrap_or_else(|e| e);
    report.capture_error = (|| -> Result<usize, String> {
        let first = tinystt::audio::AudioCapture::start(None).map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(300));
        first.cancel();
        let second = tinystt::audio::AudioCapture::start(None).map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(300));
        let captured = second.stop();
        if captured.samples.is_empty() {
            return Err("No microphone frames".into());
        }
        Ok(captured.samples.len())
    })()
    .map(|n| {
        report.capture_cancel_restart = true;
        report.frames = n;
        String::new()
    })
    .unwrap_or_else(|e| e);
    std::fs::create_dir_all("work/acceptance")?;
    std::fs::write(
        "work/acceptance/devices.toml",
        toml::to_string_pretty(&report)?,
    )?;
    println!("{}", toml::to_string_pretty(&report)?);
    if !(report.output_pause_resume_stop && report.capture_cancel_restart) {
        return Err("Device acceptance incomplete; inspect devices.toml".into());
    }
    Ok(())
}
fn probe_output() -> Result<(), String> {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let player = tinytts::audio::player::Player::new().map_err(|e| e.to_string())?;
    player.pause();
    if !player.is_paused() {
        return Err("Pause failed".into());
    }
    let (sender, receiver) = tinytts::audio::queue::bounded(1);
    player.start_feeding(receiver).map_err(|e| e.to_string())?;
    let producer_done = Arc::new(AtomicBool::new(false));
    let completed = producer_done.clone();
    std::thread::spawn(move || {
        for _ in 0..8 {
            if sender
                .send(tinytts::tts::AudioChunk {
                    samples: (0..2400)
                        .map(|i| (std::f32::consts::TAU * 440.0 * i as f32 / 24000.0).sin() * 0.02)
                        .collect(),
                    sample_rate: 24000,
                })
                .is_err()
            {
                return;
            }
        }
        completed.store(true, Ordering::Release);
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while player.pending() < 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if player.pending() != 2 || producer_done.load(Ordering::Acquire) {
        return Err(
            "Paused sink must hold exactly two chunks and apply producer backpressure".into(),
        );
    }
    player.resume();
    if player.is_paused() {
        return Err("Resume failed".into());
    }
    while !player.is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !player.is_empty() {
        return Err("Output did not drain".into());
    }
    if !producer_done.load(Ordering::Acquire) {
        return Err("Producer did not finish after resume".into());
    }
    player.stop();
    Ok(())
}
