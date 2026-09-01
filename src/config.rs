use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub autostart: bool,
    #[serde(default = "default_true")]
    pub restore_on_wake: bool,
    #[serde(default = "default_true")]
    pub hotkeys_enabled: bool,
    #[serde(default = "default_step")]
    pub step: u8,
    #[serde(default)]
    pub last_brightness: HashMap<String, i16>,
}

fn default_true() -> bool {
    true
}

fn default_step() -> u8 {
    5
}

impl Default for Config {
    fn default() -> Self {
        Self {
            autostart: false,
            restore_on_wake: true,
            hotkeys_enabled: true,
            step: 5,
            last_brightness: HashMap::new(),
        }
    }
}

impl Config {
    fn path() -> Result<PathBuf> {
        let base = std::env::var("APPDATA").context("APPDATA not set")?;
        let dir = PathBuf::from(base).join("LuxTray");
        fs::create_dir_all(&dir)?;
        Ok(dir.join("config.json"))
    }

    fn legacy_path() -> Option<PathBuf> {
        let base = std::env::var("APPDATA").ok()?;
        Some(PathBuf::from(base).join("GlowTray").join("config.json"))
    }

    pub fn load() -> Self {
        let Ok(path) = Self::path() else {
            return Self::default();
        };
        if let Ok(text) = fs::read_to_string(&path) {
            return serde_json::from_str(&text).unwrap_or_default();
        }
        if let Some(legacy) = Self::legacy_path() {
            if let Ok(text) = fs::read_to_string(legacy) {
                let cfg: Self = serde_json::from_str(&text).unwrap_or_default();
                let _ = cfg.save();
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let text = serde_json::to_string_pretty(self)?;
        fs::write(path, text)?;
        Ok(())
    }
}
