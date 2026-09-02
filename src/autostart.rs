use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use windows::core::w;
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY_CURRENT_USER, KEY_READ, REG_SZ,
};

const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("LuxTray");
const LEGACY_VALUE_NAME: windows::core::PCWSTR = w!("GlowTray");

fn exe_path() -> Result<String> {
    let path = std::env::current_exe().context("current_exe")?;
    Ok(path.to_string_lossy().into_owned())
}

fn startup_dir() -> Option<PathBuf> {
    let appdata = std::env::var("APPDATA").ok()?;
    Some(
        PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Startup"),
    )
}

fn shortcut_paths() -> Vec<PathBuf> {
    let Some(dir) = startup_dir() else {
        return Vec::new();
    };
    vec![dir.join("LuxTray.lnk"), dir.join("GlowTray.lnk")]
}

fn shortcut_exists() -> bool {
    shortcut_paths().iter().any(|p| p.exists())
}

fn remove_startup_shortcuts() {
    for path in shortcut_paths() {
        let _ = fs::remove_file(path);
    }
}

/// True if the HKCU Run value or a leftover Startup-folder shortcut exists.
pub fn is_enabled() -> bool {
    query_run_value().is_some() || shortcut_exists()
}

fn query_run_value() -> Option<String> {
    unsafe {
        let mut hkey = Default::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, Some(0), KEY_READ, &mut hkey).is_err() {
            return None;
        }
        let mut buf = [0u16; 520];
        let mut size = (buf.len() * 2) as u32;
        let mut ty = REG_SZ;
        let ok = RegQueryValueExW(
            hkey,
            VALUE_NAME,
            None,
            Some(&mut ty),
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut size),
        )
        .is_ok();
        let _ = RegCloseKey(hkey);
        if !ok {
            return None;
        }
        let n = (size as usize / 2).saturating_sub(1).min(buf.len());
        Some(String::from_utf16_lossy(&buf[..n]))
    }
}

fn set_run_key(enable: bool) -> Result<()> {
    unsafe {
        let mut hkey = Default::default();
        RegCreateKeyW(HKEY_CURRENT_USER, RUN_KEY, &mut hkey)
            .ok()
            .context("RegCreateKeyW")?;

        let result = if enable {
            let path = format!("\"{}\"", exe_path()?);
            let mut wide: Vec<u16> = path.encode_utf16().collect();
            wide.push(0);
            let set = RegSetValueExW(
                hkey,
                VALUE_NAME,
                None,
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    wide.as_ptr() as *const u8,
                    wide.len() * 2,
                )),
            )
            .ok()
            .context("RegSetValueExW");
            let _ = RegDeleteValueW(hkey, LEGACY_VALUE_NAME);
            set
        } else {
            let _ = RegDeleteValueW(hkey, LEGACY_VALUE_NAME);
            let err = RegDeleteValueW(hkey, VALUE_NAME);
            if err.is_ok() || err == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                Err(anyhow::anyhow!("RegDeleteValueW failed: {err:?}"))
            }
        };
        let _ = RegCloseKey(hkey);
        result
    }
}

/// Enable or disable autostart via HKCU Run only. Always removes Startup-folder
/// shortcuts so the installer shortcut and the in-app toggle cannot both fire.
pub fn set_enabled(enable: bool) -> Result<()> {
    remove_startup_shortcuts();
    set_run_key(enable)
}

/// Collapse leftover Startup-folder shortcuts and the Run key into a single
/// mechanism. Returns the effective autostart state.
pub fn sync_on_launch(prefer: bool) -> Result<bool> {
    let enable = prefer || shortcut_exists() || query_run_value().is_some();
    set_enabled(enable)?;
    Ok(enable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_paths_are_under_startup() {
        let paths = shortcut_paths();
        if std::env::var_os("APPDATA").is_none() {
            assert!(paths.is_empty());
            return;
        }
        assert_eq!(paths.len(), 2);
        for p in paths {
            let s = p.to_string_lossy();
            assert!(s.contains("Startup"), "{s}");
            assert!(s.ends_with(".lnk"), "{s}");
        }
    }
}
