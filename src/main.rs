#![windows_subsystem = "windows"]

mod app;
mod autostart;
mod config;
mod flyout;
mod icon;
mod monitors;
mod softdim;
mod tray;
mod wmi_brightness;

use anyhow::Result;
use std::io::Write;
use std::os::windows::io::{FromRawHandle, IntoRawHandle, RawHandle};
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Console::{
    AttachConsole, GetStdHandle, WriteConsoleW, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
    STD_OUTPUT_HANDLE,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

fn main() {
    install_panic_hook();
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        prepare_stdio();
        match cli(&args) {
            Ok(()) => {}
            Err(err) => {
                emit_err(&format!("luxtray: {err:#}"));
                std::process::exit(1);
            }
        }
        return;
    }

    log_launch("start");

    if let Some(hwnd) = app::find_running() {
        log_launch("already running — bringing panel to front");
        app::activate_existing(hwnd);
        return;
    }

    if !claim_single_instance() {
        if let Some(hwnd) = app::find_running() {
            log_launch("mutex held — bringing panel to front");
            app::activate_existing(hwnd);
        }
        return;
    }

    log_launch("starting tray app");

    if let Err(err) = app::run() {
        let text = format!("{err:#}");
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        let mut title: Vec<u16> = "LuxTray".encode_utf16().collect();
        title.push(0);
        unsafe {
            let _ = MessageBoxW(
                None,
                windows::core::PCWSTR(wide.as_ptr()),
                windows::core::PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        let body = format!("{}  {loc}\n{payload}\n", chrono_like_now());
        if let Ok(base) = std::env::var("APPDATA") {
            let dir = std::path::PathBuf::from(base).join("LuxTray");
            let _ = std::fs::create_dir_all(&dir);
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("panic.log"))
            {
                let _ = write!(f, "{body}");
            }
        }
        let text = "LuxTray 已异常退出。详情见 %APPDATA%\\LuxTray\\panic.log";
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        let mut title: Vec<u16> = "LuxTray".encode_utf16().collect();
        title.push(0);
        unsafe {
            let _ = MessageBoxW(
                None,
                windows::core::PCWSTR(wide.as_ptr()),
                windows::core::PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONERROR,
            );
        }
    }));
}

fn log_launch(msg: &str) {
    let Ok(base) = std::env::var("APPDATA") else {
        return;
    };
    let dir = std::path::PathBuf::from(base).join("LuxTray");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("launch.log"))
    {
        let _ = writeln!(f, "{}  {}", chrono_like_now(), msg);
    }
}

fn chrono_like_now() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn stdout_valid() -> bool {
    unsafe {
        GetStdHandle(STD_OUTPUT_HANDLE)
            .ok()
            .map(|h| !h.is_invalid())
            .unwrap_or(false)
    }
}

fn prepare_stdio() {
    if !stdout_valid() {
        unsafe {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

fn emit(msg: &str) {
    let line = if msg.ends_with('\n') {
        msg.to_string()
    } else {
        format!("{msg}\n")
    };
    unsafe {
        let Ok(h) = GetStdHandle(STD_OUTPUT_HANDLE) else {
            return;
        };
        if h.is_invalid() {
            return;
        }
        let wide: Vec<u16> = line.encode_utf16().collect();
        let mut n = 0u32;
        if WriteConsoleW(h, &wide, Some(&mut n), None).is_ok() {
            return;
        }
        let mut file = std::fs::File::from_raw_handle(h.0 as RawHandle);
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
        let _ = file.into_raw_handle();
    }
}

fn emit_err(msg: &str) {
    let line = format!("{msg}\n");
    unsafe {
        if let Ok(h) = GetStdHandle(STD_ERROR_HANDLE) {
            if !h.is_invalid() {
                let wide: Vec<u16> = line.encode_utf16().collect();
                let mut n = 0u32;
                if WriteConsoleW(h, &wide, Some(&mut n), None).is_ok() {
                    return;
                }
                let mut file = std::fs::File::from_raw_handle(h.0 as RawHandle);
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
                let _ = file.into_raw_handle();
            }
        }
    }
    emit(msg);
}

fn claim_single_instance() -> bool {
    unsafe {
        let _mutex = CreateMutexW(None, true, windows::core::w!("Local\\LuxTraySingleton"));
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}

fn cli(args: &[String]) -> Result<()> {
    let cmd = args[0].as_str();
    match cmd {
        "list" | "--list" | "-l" => {
            let mons = monitors::enumerate()?;
            if mons.is_empty() {
                emit("(no controllable monitors)");
                return Ok(());
            }
            for (i, m) in mons.iter().enumerate() {
                let kind = match m.backend {
                    monitors::Backend::Ddc { .. } => "DDC/CI",
                    monitors::Backend::Wmi { .. } => "WMI",
                };
                emit(&format!(
                    "{}. {}  {}%  [{kind}]  {}",
                    i + 1,
                    m.name,
                    m.current,
                    m.id
                ));
            }
            Ok(())
        }
        "set" | "--set" => {
            let pct: u8 = args
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("usage: luxtray set <0-100> [--monitor N]"))?
                .parse()?;
            let mut mons = monitors::enumerate()?;
            let id = monitor_id_from_args(args, &mons)?;
            monitors::apply_percent(&mut mons, id.as_deref(), pct)?;
            emit(&format!("set {}%", pct));
            Ok(())
        }
        "offset" | "--offset" => {
            let delta: i16 = args
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("usage: luxtray offset <+/-N> [--monitor N]"))?
                .parse()?;
            let mut mons = monitors::enumerate()?;
            if let Some(id) = monitor_id_from_args(args, &mons)? {
                for m in mons.iter_mut().filter(|m| m.id == id) {
                    let next = monitors::clamp_level(m.current + delta);
                    m.set_percent(monitors::hardware_percent(next))?;
                    m.current = next;
                    emit(&format!("{} -> {}", m.name, next));
                }
            } else {
                monitors::offset_all(&mut mons, delta)?;
                emit(&format!("offset {delta}"));
            }
            Ok(())
        }
        "help" | "--help" | "-h" => {
            emit(
                "LuxTray — lightweight monitor brightness\n\n\
                 luxtray                 start tray app\n\
                 luxtray list            list monitors\n\
                 luxtray set 50          set all to 50%\n\
                 luxtray set 70 --monitor 1\n\
                 luxtray offset -10\n\
                 luxtray offset 5 --monitor 2",
            );
            Ok(())
        }
        other => anyhow::bail!("unknown command: {other} (try luxtray help)"),
    }
}

fn monitor_id_from_args(args: &[String], mons: &[monitors::Monitor]) -> Result<Option<String>> {
    let Some(pos) = args.iter().position(|a| a == "--monitor" || a == "-m") else {
        return Ok(None);
    };
    let n: usize = args
        .get(pos + 1)
        .ok_or_else(|| anyhow::anyhow!("--monitor needs a number"))?
        .parse()?;
    let m = mons
        .get(n.saturating_sub(1))
        .ok_or_else(|| anyhow::anyhow!("monitor {n} not found"))?;
    Ok(Some(m.id.clone()))
}
