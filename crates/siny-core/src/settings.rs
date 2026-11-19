// SPDX-License-Identifier: AGPL-3.0-only
use crate::{terms::Term, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub sensevoice_dir: String,
    pub kokoro_dir: String,
    pub threads: u8,
    pub language: String,
    pub voice: i32,
    pub speed: f32,
    pub microphone: String,
    pub hotkey: String,
    pub auto_paste: bool,
    pub idle_unload_seconds: u64,
    pub terms: Vec<Term>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            sensevoice_dir: "models/sensevoice".into(),
            kokoro_dir: "models/kokoro".into(),
            threads: 2,
            language: "auto".into(),
            voice: 3,
            speed: 1.0,
            microphone: String::new(),
            hotkey: "Ctrl+Alt+D".into(),
            auto_paste: false,
            idle_unload_seconds: 60,
            terms: Vec::new(),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if ![1, 2, 4].contains(&self.threads)
            || !self.speed.is_finite()
            || !(0.5..=2.0).contains(&self.speed)
            || !(1..=3600).contains(&self.idle_unload_seconds)
            || self.voice < 0
        {
            return Err("Invalid threads, speed, voice or idle timeout".into());
        }
        if !["auto", "zh", "en", "ja", "ko", "yue"].contains(&self.language.as_str()) {
            return Err("Unsupported SenseVoice language".into());
        }
        if self.hotkey.is_empty() || self.hotkey.len() > 80 || self.hotkey.contains('\0') {
            return Err("Invalid dictation shortcut".into());
        }
        crate::terms::validate(&self.terms)
    }
    pub fn load(path: &Path) -> Result<Self> {
        let settings = if path.exists() {
            let metadata = path.metadata().map_err(|e| e.to_string())?;
            if metadata.len() > 1024 * 1024 {
                return Err("Configuration exceeds 1 MiB".into());
            }
            toml::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?
        } else {
            Self::default()
        };
        settings.validate()?;
        Ok(settings)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        use std::io::Write;
        self.validate()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let temp = path.with_extension(format!("{}.tmp", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            file.write_all(
                toml::to_string_pretty(self)
                    .map_err(|e| e.to_string())?
                    .as_bytes(),
            )
            .map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            drop(file);
            std::fs::rename(&temp, path).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_preserves_existing_temp_file_and_roundtrips() {
        let dir = std::env::temp_dir().join(format!("siny-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("siny.toml");
        let temp = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&temp, "existing file").unwrap();
        assert!(Settings::default().save(&path).is_err());
        assert_eq!(std::fs::read_to_string(&temp).unwrap(), "existing file");
        std::fs::remove_file(&temp).unwrap();
        Settings::default().save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap().threads, 2);
        let updated = Settings {
            language: "zh".into(),
            auto_paste: true,
            ..Settings::default()
        };
        updated.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap().language, "zh");
        assert!(Settings::load(&path).unwrap().auto_paste);
        let invalid = Settings {
            threads: 99,
            ..Settings::default()
        };
        assert!(invalid.save(&path).is_err());
        assert_eq!(Settings::load(&path).unwrap().threads, 2);
        assert_eq!(Settings::load(&path).unwrap().language, "zh");
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}
