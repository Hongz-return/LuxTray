use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        if let Some(cfg) = Self::load_from(&path) {
            return cfg;
        }
        if let Some(legacy) = Self::legacy_path() {
            if let Some(cfg) = Self::load_from(&legacy) {
                let _ = cfg.save();
                return cfg;
            }
        }
        Self::default()
    }

    fn load_from(path: &Path) -> Option<Self> {
        let text = fs::read_to_string(path).ok()?;
        match serde_json::from_str(&text) {
            Ok(cfg) => Some(cfg),
            Err(err) => {
                let bak = backup_path(path);
                match fs::rename(path, &bak) {
                    Ok(()) => log_config(&format!(
                        "invalid JSON in {}, moved to {}: {err}",
                        path.display(),
                        bak.display()
                    )),
                    Err(rename_err) => log_config(&format!(
                        "invalid JSON in {} ({err}); backup failed: {rename_err}",
                        path.display()
                    )),
                }
                None
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        self.save_to(&path)
    }

    fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        atomic_write(path, text.as_bytes())
    }
}

fn backup_path(path: &Path) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let stem = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("config.json");
    path.with_file_name(format!("{stem}.bak.{ts}"))
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
    }
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn log_config(msg: &str) {
    let Ok(base) = std::env::var("APPDATA") else {
        return;
    };
    let dir = PathBuf::from(base).join("LuxTray");
    let _ = fs::create_dir_all(&dir);
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("launch.log"))
    {
        let _ = writeln!(f, "config: {msg}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("luxtray-config-tests");
        fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{name}-{}.json", std::process::id()))
    }

    #[test]
    fn default_json_roundtrip() {
        let cfg = Config::default();
        let text = serde_json::to_string_pretty(&cfg).unwrap();
        let parsed: Config = serde_json::from_str(&text).unwrap();
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let parsed: Config = serde_json::from_str("{}").unwrap();
        assert!(!parsed.autostart);
        assert!(parsed.restore_on_wake);
        assert!(parsed.hotkeys_enabled);
        assert_eq!(parsed.step, 5);
        assert!(parsed.last_brightness.is_empty());
    }

    #[test]
    fn invalid_json_is_backed_up_not_overwritten() {
        let path = temp_file("bad");
        let _ = fs::remove_file(&path);
        fs::write(&path, "{not json").unwrap();
        assert!(Config::load_from(&path).is_none());
        assert!(!path.exists(), "broken file must be moved aside");
        let parent = path.parent().unwrap();
        let backups: Vec<_> = fs::read_dir(parent)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("bad") && n.contains(".bak."))
            .collect();
        assert!(!backups.is_empty(), "expected a .bak file, got {backups:?}");
        for n in backups {
            let _ = fs::remove_file(parent.join(n));
        }
    }

    #[test]
    fn save_to_is_readable() {
        let path = temp_file("ok");
        let _ = fs::remove_file(&path);
        let mut cfg = Config {
            autostart: true,
            step: 7,
            ..Config::default()
        };
        cfg.last_brightness.insert("m1".into(), -10);
        cfg.save_to(&path).unwrap();
        let loaded = Config::load_from(&path).expect("saved config should parse");
        assert_eq!(loaded, cfg);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("json.tmp"));
    }
}
