// SPDX-License-Identifier: AGPL-3.0-only
//! Microphone-free WAV recognition; does not construct desktop/device/clipboard services.
use siny_core::{Result, Settings};
use std::path::PathBuf;

pub const HELP: &str = "Siny — offline dictation and speech\n\nUsage:\n  siny.exe\n  siny.exe --file <input.wav> [--model-dir <dir>] [--num-threads <1|2|4>] [--language <auto|zh|en|yue|ja|ko>] [--config <siny.toml>]\n  siny.exe --help | --version\n\nFile mode prints recognition to stdout; exit 1 is an execution error, exit 2 invalid arguments. No microphone, playback, clipboard or GUI is opened. WAV input is limited to 256 MiB / one hour.\n";

#[derive(Debug, PartialEq)]
pub enum Command {
    Gui,
    Help,
    Version,
    File(FileOptions),
}

#[derive(Debug, Default, PartialEq)]
pub struct FileOptions {
    pub path: PathBuf,
    pub model_dir: Option<String>,
    pub threads: Option<u8>,
    pub language: Option<String>,
    pub config: Option<PathBuf>,
}

pub fn parse(args: &[String]) -> Result<Command> {
    if args.is_empty() {
        return Ok(Command::Gui);
    }
    if args.len() == 1 {
        match args[0].as_str() {
            "--help" | "-h" => return Ok(Command::Help),
            "--version" | "-V" => return Ok(Command::Version),
            _ => (),
        }
    }
    let mut options = FileOptions::default();
    let mut seen = std::collections::HashSet::new();
    let mut args = args.iter();
    while let Some(flag) = args.next() {
        let flag = if flag == "-f" {
            "--file"
        } else {
            flag.as_str()
        };
        if ![
            "--file",
            "--model-dir",
            "--num-threads",
            "--language",
            "--config",
        ]
        .contains(&flag)
        {
            return Err(format!("Unknown argument: {flag}"));
        }
        if !seen.insert(flag) {
            return Err(format!("Duplicate argument: {flag}"));
        }
        let value = args
            .next()
            .filter(|v| !v.is_empty() && !v.starts_with("--"))
            .ok_or_else(|| format!("Missing value for {flag}"))?;
        match flag {
            "--file" => options.path = value.into(),
            "--model-dir" => options.model_dir = Some(value.clone()),
            "--num-threads" => {
                options.threads = Some(value.parse().map_err(|_| "Invalid thread count")?)
            }
            "--language" => options.language = Some(value.clone()),
            "--config" => options.config = Some(value.into()),
            _ => unreachable!(),
        }
    }
    if options.path.as_os_str().is_empty() {
        return Err("File mode requires --file <input.wav>".into());
    }
    let mut check = Settings::default();
    apply_overrides(&mut check, &options);
    check.validate()?;
    Ok(Command::File(options))
}

fn apply_overrides(settings: &mut Settings, options: &FileOptions) {
    if let Some(value) = &options.model_dir {
        settings.sensevoice_dir = value.clone();
    }
    if let Some(value) = options.threads {
        settings.threads = value;
    }
    if let Some(value) = &options.language {
        settings.language = value.clone();
    }
}

#[cfg(feature = "sherpa")]
pub fn recognize(options: &FileOptions) -> Result<String> {
    use siny_core::{Audio, Backend};
    let base = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Executable has no parent")?
        .to_path_buf();
    let config = options
        .config
        .clone()
        .unwrap_or_else(|| base.join("config/siny.toml"));
    if options.config.is_some() && !config.is_file() {
        return Err("The explicitly selected configuration file does not exist".into());
    }
    let mut settings = Settings::load(&config)?;
    apply_overrides(&mut settings, options);
    settings.validate()?;
    if options.path.metadata().map_err(|e| e.to_string())?.len() > 256 * 1024 * 1024 {
        return Err("WAV input exceeds 256 MiB".into());
    }
    let audio = Audio {
        samples: tinystt::audio::read_wav_as_16k_mono(&options.path).map_err(|e| e.to_string())?,
        sample_rate: 16000,
    };
    audio.validate(3600)?;
    let mut backend = crate::backend::SherpaBackend::new(base, settings.clone())?;
    let text = backend.recognize(
        &audio,
        &std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    )?;
    Ok(siny_core::terms::apply(&text, &settings.terms))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).into()).collect()
    }
    #[test]
    fn cli_requires_explicit_file_and_rejects_ambiguous_or_invalid_options() {
        assert_eq!(parse(&[]).unwrap(), Command::Gui);
        assert_eq!(parse(&args(&["--help"])).unwrap(), Command::Help);
        for invalid in [
            vec!["--file"],
            vec!["--language", "en"],
            vec!["--file", "a", "--file", "b"],
            vec!["--file", "a", "--num-threads", "3"],
            vec!["--file", "a", "--language", "invalid"],
            vec!["--file", "a", "--help"],
        ] {
            assert!(parse(&args(&invalid)).is_err(), "{invalid:?}");
        }
        assert!(matches!(
            parse(&args(&[
                "--file",
                "a.wav",
                "--language",
                "en",
                "--num-threads",
                "4"
            ])),
            Ok(Command::File(_))
        ));
    }
}
