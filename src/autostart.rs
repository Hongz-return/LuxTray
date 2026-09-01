use anyhow::{Context, Result};
use windows::core::w;
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY_CURRENT_USER, KEY_READ, REG_SZ,
};

const RUN_KEY: windows::core::PCWSTR =
    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("LuxTray");
const LEGACY_VALUE_NAME: windows::core::PCWSTR = w!("GlowTray");

fn exe_path() -> Result<String> {
    let path = std::env::current_exe().context("current_exe")?;
    Ok(path.to_string_lossy().into_owned())
}

#[allow(dead_code)]
pub fn is_enabled() -> bool {
    query_run_value().is_some()
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

pub fn set_enabled(enable: bool) -> Result<()> {
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
