use anyhow::Result;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::rc::Rc;
use windows::core::BOOL;
use windows::Win32::Devices::Display::{
    DestroyPhysicalMonitors, DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes,
    GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR, GetPhysicalMonitorsFromHMONITOR,
    QueryDisplayConfig, SetMonitorBrightness, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
    DISPLAYCONFIG_TARGET_DEVICE_NAME, PHYSICAL_MONITOR, QDC_ONLY_ACTIVE_PATHS,
};
use windows::Win32::Foundation::{HANDLE, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFOEXW,
    MONITOR_DEFAULTTOPRIMARY,
};

use crate::wmi_brightness::{self, WmiPanel};

/// User-facing level: 0–100 is hardware brightness; below 0 is extra software dim.
pub const LEVEL_MIN: i16 = -50;
pub const LEVEL_MAX: i16 = 100;

pub fn clamp_level(level: i16) -> i16 {
    level.clamp(LEVEL_MIN, LEVEL_MAX)
}

pub fn hardware_percent(level: i16) -> u8 {
    if level <= 0 {
        0
    } else {
        (level as u8).min(100)
    }
}

pub fn extra_alpha(level: i16) -> u8 {
    if level >= 0 {
        return 0;
    }
    let extra = (-level) as u32;
    (extra * 170 / (-LEVEL_MIN) as u32) as u8
}

/// Owns a DDC/CI physical-monitor handle and releases it with
/// `DestroyPhysicalMonitors` (shared via `Rc` so `Monitor` stays `Clone`).
pub struct DdcHandle {
    inner: PHYSICAL_MONITOR,
}

impl std::fmt::Debug for DdcHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let handle = self.inner.hPhysicalMonitor;
        f.debug_struct("DdcHandle")
            .field("handle", &handle.0)
            .finish()
    }
}

impl Drop for DdcHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyPhysicalMonitors(&[self.inner]);
        }
    }
}

impl DdcHandle {
    fn handle(&self) -> HANDLE {
        self.inner.hPhysicalMonitor
    }
}

#[derive(Debug, Clone)]
pub enum Backend {
    Ddc { handle: Rc<DdcHandle> },
    Wmi { instance_name: String },
}

#[derive(Debug, Clone)]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub backend: Backend,
    pub min: u32,
    pub max: u32,
    pub current: i16,
    pub rect: RECT,
}

impl Monitor {
    pub fn set_percent(&self, pct: u8) -> Result<()> {
        let pct = pct.min(100);
        match &self.backend {
            Backend::Ddc { handle } => {
                let handle = handle.handle();
                let span = self.max.saturating_sub(self.min);
                let raw = self.min + span * pct as u32 / 100;
                unsafe {
                    if SetMonitorBrightness(handle, raw) == 0 {
                        anyhow::bail!("SetMonitorBrightness failed");
                    }
                }
                Ok(())
            }
            Backend::Wmi { instance_name } => wmi_brightness::set(instance_name, pct),
        }
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.current = read_percent(self)? as i16;
        Ok(())
    }
}

fn read_percent(mon: &Monitor) -> Result<u8> {
    match &mon.backend {
        Backend::Ddc { handle } => {
            let (min, cur, max) = ddc_get(handle.handle())?;
            Ok(normalize(cur, min, max))
        }
        Backend::Wmi { instance_name } => wmi_brightness::get(instance_name),
    }
}

fn normalize(cur: u32, min: u32, max: u32) -> u8 {
    if max <= min {
        return 0;
    }
    let pct = (cur.saturating_sub(min) as u64 * 100 / (max - min) as u64) as u32;
    pct.min(100) as u8
}

fn ddc_get(handle: HANDLE) -> Result<(u32, u32, u32)> {
    let mut min = 0u32;
    let mut cur = 0u32;
    let mut max = 0u32;
    unsafe {
        if GetMonitorBrightness(handle, &mut min, &mut cur, &mut max) == 0 {
            anyhow::bail!("GetMonitorBrightness failed");
        }
    }
    Ok((min, cur, max))
}

struct DisplayPath {
    gdi_name: String,
    friendly: String,
    device_path: String,
}

pub fn enumerate() -> Result<Vec<Monitor>> {
    let names = display_names();
    let hmons = enum_hmonitors();
    let wmi_panels = wmi_brightness::list().unwrap_or_default();
    let mut used_wmi = vec![false; wmi_panels.len()];
    let mut out = Vec::new();

    for (idx, hmon) in hmons.into_iter().enumerate() {
        let gdi = monitor_gdi_name(hmon).unwrap_or_else(|| format!("DISPLAY{}", idx + 1));
        let path = names.iter().find(|p| p.gdi_name.eq_ignore_ascii_case(&gdi));
        let friendly = path
            .map(|p| p.friendly.clone())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("显示器 {}", idx + 1));
        let device_path = path.map(|p| p.device_path.clone()).unwrap_or_default();

        let rect = monitor_rect(hmon);
        let physicals = physical_monitors(hmon);
        if physicals.is_empty() {
            if let Some(i) = match_wmi(&wmi_panels, &device_path, &used_wmi) {
                used_wmi[i] = true;
                out.push(wmi_monitor(&wmi_panels[i], &friendly, idx, rect));
            }
            continue;
        }

        let mut claimed_ddc = false;
        for (pidx, phys) in physicals.into_iter().enumerate() {
            let handle = phys.hPhysicalMonitor;
            match ddc_get(handle) {
                Ok((min, cur, max)) => {
                    claimed_ddc = true;
                    let id = if device_path.is_empty() {
                        format!("{gdi}#{pidx}")
                    } else {
                        format!("{device_path}#{pidx}")
                    };
                    let desc = {
                        let raw = phys.szPhysicalMonitorDescription;
                        utf16_z(&raw)
                    };
                    let name =
                        if !desc.is_empty() && !desc.eq_ignore_ascii_case("Generic PnP Monitor") {
                            desc
                        } else {
                            friendly.clone()
                        };
                    out.push(Monitor {
                        id,
                        name,
                        backend: Backend::Ddc {
                            handle: Rc::new(DdcHandle { inner: phys }),
                        },
                        min,
                        max,
                        current: normalize(cur, min, max) as i16,
                        rect,
                    });
                }
                Err(_) => unsafe {
                    let _ = DestroyPhysicalMonitors(&[phys]);
                },
            }
        }

        if !claimed_ddc {
            if let Some(i) = match_wmi(&wmi_panels, &device_path, &used_wmi) {
                used_wmi[i] = true;
                out.push(wmi_monitor(&wmi_panels[i], &friendly, idx, rect));
            }
        }
    }

    // WMI panels that didn't match a GDI monitor (some laptops).
    for (i, panel) in wmi_panels.iter().enumerate() {
        if used_wmi[i] {
            continue;
        }
        if out.iter().any(|m| match &m.backend {
            Backend::Wmi { instance_name } => instance_name == &panel.instance_name,
            _ => false,
        }) {
            continue;
        }
        out.push(wmi_monitor(panel, "内置屏幕", out.len(), primary_rect()));
    }

    Ok(out)
}

fn wmi_monitor(panel: &WmiPanel, fallback_name: &str, idx: usize, rect: RECT) -> Monitor {
    Monitor {
        id: format!("wmi:{}", panel.instance_name),
        name: if fallback_name.is_empty() {
            format!("显示器 {}", idx + 1)
        } else {
            fallback_name.to_string()
        },
        backend: Backend::Wmi {
            instance_name: panel.instance_name.clone(),
        },
        min: 0,
        max: 100,
        current: panel.current() as i16,
        rect,
    }
}

fn monitor_rect(hmon: HMONITOR) -> RECT {
    unsafe {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmon, &mut info as *mut _ as *mut _).as_bool() {
            info.monitorInfo.rcMonitor
        } else {
            RECT::default()
        }
    }
}

fn primary_rect() -> RECT {
    unsafe {
        let origin = POINT { x: 0, y: 0 };
        monitor_rect(MonitorFromPoint(origin, MONITOR_DEFAULTTOPRIMARY))
    }
}

fn match_wmi(panels: &[WmiPanel], device_path: &str, used: &[bool]) -> Option<usize> {
    let needle = normalize_pnp(device_path);
    if needle.is_empty() {
        return panels.iter().position(|_p| false).or_else(|| {
            panels
                .iter()
                .enumerate()
                .find(|(i, _)| !used[*i])
                .map(|(i, _)| i)
                .filter(|_| panels.len() == 1)
        });
    }
    panels.iter().enumerate().find_map(|(i, p)| {
        if used[i] {
            return None;
        }
        let hay = normalize_pnp(&p.instance_name);
        if !hay.is_empty() && (hay.contains(&needle) || needle.contains(&hay)) {
            Some(i)
        } else {
            None
        }
    })
}

fn normalize_pnp(s: &str) -> String {
    let s = s.replace('/', "\\").to_uppercase();
    let s = s.replace("\\\\?\\", "").replace("\\\\.\\", "");
    s.replace("#", "\\")
        .split('\\')
        .filter(|p| {
            !p.is_empty()
                && *p != "DISPLAY"
                && !p.starts_with('{')
                && !p.starts_with("UID")
                && *p != "MONITOR"
        })
        .take(2)
        .collect::<Vec<_>>()
        .join("\\")
}

fn enum_hmonitors() -> Vec<HMONITOR> {
    let mut list = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut list as *mut _ as isize),
        );
    }
    list
}

unsafe extern "system" fn enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _lprc: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let list = &mut *(lparam.0 as *mut Vec<HMONITOR>);
    list.push(hmonitor);
    true.into()
}

fn monitor_gdi_name(hmon: HMONITOR) -> Option<String> {
    unsafe {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        GetMonitorInfoW(hmon, &mut info as *mut _ as *mut _)
            .as_bool()
            .then_some(())?;
        Some(utf16_z(&info.szDevice))
    }
}

fn physical_monitors(hmon: HMONITOR) -> Vec<PHYSICAL_MONITOR> {
    unsafe {
        let mut count = 0u32;
        if GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count).is_err() || count == 0 {
            return Vec::new();
        }
        let mut buf = vec![PHYSICAL_MONITOR::default(); count as usize];
        if GetPhysicalMonitorsFromHMONITOR(hmon, &mut buf).is_err() {
            return Vec::new();
        }
        buf
    }
}

fn display_names() -> Vec<DisplayPath> {
    unsafe {
        let mut npath = 0u32;
        let mut nmode = 0u32;
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut npath, &mut nmode).is_err() {
            return Vec::new();
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); npath as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nmode as usize];
        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut npath,
            paths.as_mut_ptr(),
            &mut nmode,
            modes.as_mut_ptr(),
            None,
        )
        .is_err()
        {
            return Vec::new();
        }
        paths.truncate(npath as usize);

        let mut out = Vec::new();
        for path in &paths {
            let mut src = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: path.sourceInfo.adapterId,
                    id: path.sourceInfo.id,
                },
                ..Default::default()
            };
            let mut tgt = DISPLAYCONFIG_TARGET_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                    size: std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                    adapterId: path.targetInfo.adapterId,
                    id: path.targetInfo.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut src.header) != 0 {
                continue;
            }
            let _ = DisplayConfigGetDeviceInfo(&mut tgt.header);
            out.push(DisplayPath {
                gdi_name: utf16_z(&src.viewGdiDeviceName),
                friendly: utf16_z(&tgt.monitorFriendlyDeviceName),
                device_path: utf16_z(&tgt.monitorDevicePath),
            });
        }
        out
    }
}

fn utf16_z(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    OsString::from_wide(&buf[..len])
        .to_string_lossy()
        .trim()
        .to_string()
}

pub fn apply_percent(monitors: &mut [Monitor], id: Option<&str>, pct: u8) -> Result<()> {
    let pct = pct.min(100);
    for m in monitors.iter_mut() {
        if id.map(|id| m.id == id).unwrap_or(true) {
            m.set_percent(pct)?;
            m.current = pct as i16;
        }
    }
    Ok(())
}

pub fn offset_all(monitors: &mut [Monitor], delta: i16) -> Result<()> {
    for m in monitors.iter_mut() {
        let next = clamp_level(m.current + delta);
        m.set_percent(hardware_percent(next))?;
        m.current = next;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_level_bounds() {
        assert_eq!(clamp_level(-100), LEVEL_MIN);
        assert_eq!(clamp_level(LEVEL_MIN), LEVEL_MIN);
        assert_eq!(clamp_level(0), 0);
        assert_eq!(clamp_level(50), 50);
        assert_eq!(clamp_level(LEVEL_MAX), LEVEL_MAX);
        assert_eq!(clamp_level(200), LEVEL_MAX);
    }

    #[test]
    fn hardware_percent_maps_extra_dim_to_zero() {
        assert_eq!(hardware_percent(-50), 0);
        assert_eq!(hardware_percent(-1), 0);
        assert_eq!(hardware_percent(0), 0);
        assert_eq!(hardware_percent(1), 1);
        assert_eq!(hardware_percent(100), 100);
        assert_eq!(hardware_percent(127), 100);
    }

    #[test]
    fn extra_alpha_only_below_zero() {
        assert_eq!(extra_alpha(100), 0);
        assert_eq!(extra_alpha(0), 0);
        assert_eq!(extra_alpha(-1), 3);
        assert_eq!(extra_alpha(LEVEL_MIN), 170);
    }

    #[test]
    fn normalize_brightness_span() {
        assert_eq!(normalize(0, 0, 0), 0);
        assert_eq!(normalize(50, 100, 50), 0);
        assert_eq!(normalize(0, 0, 100), 0);
        assert_eq!(normalize(50, 0, 100), 50);
        assert_eq!(normalize(100, 0, 100), 100);
        assert_eq!(normalize(75, 50, 150), 25);
    }
}
