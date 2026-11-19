// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit, read-only legacy import. A candidate is applied only after Save & apply.
use siny_core::{Result, Settings};
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum LegacyApp {
    TinyStt,
    TinyTts,
}

pub fn import(path: &Path, app: LegacyApp, current: &Settings) -> Result<Settings> {
    if path.metadata().map_err(|e| e.to_string())?.len() > 1024 * 1024 {
        return Err("Legacy configuration exceeds 1 MiB".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let document: toml::Table = toml::from_str(&text).map_err(|e| e.to_string())?;
    let allowed: &[&str] = match app {
        LegacyApp::TinyStt => &[
            "model_dir",
            "num_threads",
            "hotkey",
            "auto_paste",
            "microphone",
            "language",
        ],
        LegacyApp::TinyTts => &["model_dir", "num_threads", "voice", "speed"],
    };
    if document.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("The configuration does not match the selected legacy application".into());
    }
    let path = std::path::absolute(path).map_err(|e| e.to_string())?;
    let parent = path
        .parent()
        .ok_or("Configuration has no parent directory")?;
    let base = if parent.file_name().is_some_and(|n| n == "config") {
        parent
            .parent()
            .ok_or("Legacy application has no base directory")?
    } else {
        parent
    };
    let model = |value: &str, default: &str| {
        let value = if value.trim().is_empty() {
            default
        } else {
            value
        };
        let value = Path::new(value);
        if value.is_absolute() {
            value.to_path_buf()
        } else {
            base.join(value)
        }
        .display()
        .to_string()
    };
    let mut candidate = current.clone();
    match app {
        LegacyApp::TinyStt => {
            let old: tinystt::settings::Settings =
                toml::from_str(&text).map_err(|e| e.to_string())?;
            candidate.sensevoice_dir = model(&old.model_dir, "models/sensevoice");
            candidate.threads = old.num_threads;
            candidate.language = old.language;
            candidate.microphone = old.microphone;
            candidate.auto_paste = old.auto_paste;
            candidate.hotkey = tinystt::hotkey::HotkeySpec::parse(&old.hotkey)
                .map_err(|e| e.to_string())?
                .display;
        }
        LegacyApp::TinyTts => {
            let old: tinytts::settings::Settings =
                toml::from_str(&text).map_err(|e| e.to_string())?;
            candidate.kokoro_dir = model(
                old.model_dir.as_deref().unwrap_or_default(),
                "models/kokoro",
            );
            candidate.threads = old.num_threads;
            candidate.speed = old.speed;
            candidate.voice = tinytts::tts::KOKORO_VOICES
                .iter()
                .position(|name| *name == old.voice)
                .ok_or("Legacy voice is absent from the reviewed Kokoro voice catalog")?
                as i32;
        }
    }
    candidate.validate()?;
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_are_explicit_preserve_other_settings_and_never_rewrite_old_files() {
        let dir = std::env::temp_dir().join(format!("siny-import-{}", std::process::id()));
        let config = dir.join("config");
        std::fs::create_dir_all(&config).unwrap();
        let path = config.join("config.toml");
        let stt = "model_dir = 'relative/asr'\nnum_threads = 4\nhotkey = 'Ctrl+Shift+D'\nauto_paste = true\nlanguage = 'zh'\n";
        std::fs::write(&path, stt).unwrap();
        let imported = import(&path, LegacyApp::TinyStt, &Settings::default()).unwrap();
        assert_eq!(
            imported.sensevoice_dir,
            dir.join("relative/asr").display().to_string()
        );
        assert_eq!(imported.hotkey, "Ctrl+Shift+D");
        assert!(imported.auto_paste);
        assert_eq!(imported.threads, 4);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), stt);
        assert!(!config.join("siny.toml").exists());
        std::fs::write(&path, "voice = 'zf_xiaobei'\nspeed = 1.4\n").unwrap();
        assert!(import(&path, LegacyApp::TinyStt, &imported).is_err());
        let combined = import(&path, LegacyApp::TinyTts, &imported).unwrap();
        assert_eq!(combined.voice, 45);
        assert_eq!(combined.speed, 1.4);
        assert_eq!(combined.sensevoice_dir, imported.sensevoice_dir);
        assert!(combined.auto_paste);
        std::fs::write(&path, "voice = 'missing'\n").unwrap();
        assert!(import(&path, LegacyApp::TinyTts, &combined).is_err());
        std::fs::write(&path, "num_threads = 99\n").unwrap();
        assert!(import(&path, LegacyApp::TinyStt, &combined).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
