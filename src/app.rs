use anyhow::Result;
use std::cell::Cell;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{
    RegisterPowerSettingNotification, UnregisterPowerSettingNotification, HPOWERNOTIFY,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, VK_DOWN, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, KillTimer, LoadCursorW, PostQuitMessage, RegisterClassExW, SetTimer,
    SetWindowLongPtrW, TranslateMessage, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, IDC_ARROW, MSG,
    WM_APP, WM_COMMAND, WM_DESTROY, WM_DISPLAYCHANGE, WM_HOTKEY, WM_POWERBROADCAST, WM_TIMER,
    WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};

use crate::config::Config;
use crate::flyout;
use crate::icon;
use crate::monitors::{self, Monitor};
use crate::softdim::{self, SoftDim};
use crate::tray::{self, TrayIcon, WM_TRAY};

const CLASS: windows::core::PCWSTR = w!("LuxTrayHidden");
const HOTKEY_UP: i32 = 1;
const HOTKEY_DOWN: i32 = 2;
const TIMER_FLUSH: usize = 1;
const TIMER_RESCAN: usize = 2;
const TIMER_RESTORE: usize = 3;
const TIMER_CLICKAWAY: usize = 4;
const TIMER_POLL: usize = 5;
pub const WM_SHOW_FLYOUT: u32 = WM_APP + 2;

thread_local! {
    static APP_BORROWED: Cell<bool> = const { Cell::new(false) };
}

const ID_REFRESH: u32 = 1001;
const ID_AUTOSTART: u32 = 1002;
const ID_HOTKEYS: u32 = 1003;
const ID_RESTORE: u32 = 1004;
const ID_EXIT: u32 = 1005;

pub struct App {
    pub hwnd: HWND,
    pub flyout: HWND,
    pub monitors: Vec<Monitor>,
    pub config: Config,
    pub drag_row: Option<usize>,
    tray: Option<TrayIcon>,
    icon: windows::Win32::UI::WindowsAndMessaging::HICON,
    pending: HashMap<String, i16>,
    power: Option<HPOWERNOTIFY>,
    clickaway_arm: Option<Instant>,
    last_tray_open: Option<Instant>,
    softdim: SoftDim,
}

impl App {
    pub fn apply_slider(&mut self, row: usize, level: i16) {
        let level = monitors::clamp_level(level);
        if row == 0 {
            for m in &mut self.monitors {
                m.current = level;
                self.pending.insert(m.id.clone(), level);
            }
        } else if let Some(m) = self.monitors.get_mut(row - 1) {
            m.current = level;
            self.pending.insert(m.id.clone(), level);
        }
        unsafe {
            let _ = SetTimer(Some(self.hwnd), TIMER_FLUSH, 40, None);
        }
        self.sync_dim();
        self.update_tooltip();
    }

    pub fn offset_all(&mut self, delta: i16) {
        for m in &mut self.monitors {
            let next = monitors::clamp_level(m.current + delta);
            m.current = next;
            self.pending.insert(m.id.clone(), next);
        }
        unsafe {
            let _ = SetTimer(Some(self.hwnd), TIMER_FLUSH, 40, None);
        }
        self.sync_dim();
        self.update_tooltip();
        if flyout::is_visible(self.flyout) {
            unsafe {
                let _ =
                    windows::Win32::Graphics::Gdi::InvalidateRect(Some(self.flyout), None, true);
            }
        }
    }

    fn flush_pending(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        for (id, level) in pending {
            if let Some(m) = self.monitors.iter().find(|m| m.id == id) {
                let hw = monitors::hardware_percent(level);
                if m.set_percent(hw).is_ok() || level < 0 {
                    self.config.last_brightness.insert(id, level);
                }
            }
        }
        let _ = self.config.save();
        self.sync_dim();
        self.update_tooltip();
    }

    fn sync_dim(&mut self) {
        self.softdim.sync(&self.monitors, self.flyout);
    }

    fn rescan(&mut self) {
        let prev: HashMap<String, i16> = self
            .monitors
            .iter()
            .map(|m| (m.id.clone(), m.current))
            .collect();
        if let Ok(mut list) = monitors::enumerate() {
            for m in &mut list {
                if let Some(&lvl) = prev.get(&m.id) {
                    m.current = lvl;
                } else if let Some(&lvl) = self.config.last_brightness.get(&m.id) {
                    m.current = lvl;
                }
            }
            self.monitors = list;
        }
        self.sync_dim();
        self.update_tooltip();
        if flyout::is_visible(self.flyout) {
            self.open_flyout();
        }
    }

    fn restore_saved(&mut self) {
        let saved = self.config.last_brightness.clone();
        for m in &mut self.monitors {
            if let Some(&level) = saved.get(&m.id) {
                let hw = monitors::hardware_percent(level);
                if m.set_percent(hw).is_ok() {
                    m.current = level;
                }
            }
        }
        self.sync_dim();
        self.update_tooltip();
    }

    fn restore_extra_dim(&mut self) {
        let saved = self.config.last_brightness.clone();
        for m in &mut self.monitors {
            if let Some(&level) = saved.get(&m.id) {
                if level < 0 {
                    m.current = level;
                    let _ = m.set_percent(0);
                }
            }
        }
        self.sync_dim();
    }

    fn update_tooltip(&mut self) {
        if let Some(tray) = self.tray.as_mut() {
            let tip = if self.monitors.is_empty() {
                "LuxTray — 未检测到显示器".to_string()
            } else {
                let parts: Vec<String> = self
                    .monitors
                    .iter()
                    .map(|m| {
                        if m.current < 0 {
                            format!("{} 压暗{}", truncate(&m.name, 10), -m.current)
                        } else {
                            format!("{} {}%", truncate(&m.name, 10), m.current)
                        }
                    })
                    .collect();
                format!("LuxTray\n{}", parts.join("\n"))
            };
            tray.set_tooltip(&tip);
        }
    }

    fn tray_left_click(&mut self) {
        let now = Instant::now();
        if self
            .last_tray_open
            .map(|t| now.duration_since(t) < Duration::from_millis(400))
            .unwrap_or(false)
        {
            return;
        }
        self.last_tray_open = Some(now);
        if flyout::is_visible(self.flyout) {
            self.hide_flyout();
        } else {
            self.open_flyout();
        }
    }

    fn refresh_from_hardware(&mut self) {
        if self.drag_row.is_some() {
            return;
        }
        let mut changed = false;
        for m in &mut self.monitors {
            if self.pending.contains_key(&m.id) {
                continue;
            }
            let prev = m.current;
            if prev < 0 {
                continue;
            }
            if m.refresh().is_ok() && m.current != prev {
                changed = true;
            }
        }
        if changed {
            self.sync_dim();
            self.update_tooltip();
        }
    }

    fn open_flyout(&mut self) {
        self.refresh_from_hardware();
        let rc = self
            .tray
            .as_ref()
            .and_then(|t| t.rect())
            .unwrap_or_else(|| {
                let pt = tray::cursor_pos();
                RECT {
                    left: pt.x - 8,
                    top: pt.y - 8,
                    right: pt.x + 8,
                    bottom: pt.y + 8,
                }
            });
        flyout::show_near(self.flyout, rc, self.monitors.len().max(1) + 1);
        softdim::raise_flyout(self.flyout);
        self.clickaway_arm = Some(Instant::now() + Duration::from_millis(2000));
        unsafe {
            let _ = SetTimer(Some(self.hwnd), TIMER_CLICKAWAY, 50, None);
            let _ = SetTimer(Some(self.hwnd), TIMER_POLL, 2000, None);
        }
    }

    fn hide_flyout(&mut self) {
        flyout::hide(self.flyout);
        self.clickaway_arm = None;
        unsafe {
            let _ = KillTimer(Some(self.hwnd), TIMER_CLICKAWAY);
            let _ = KillTimer(Some(self.hwnd), TIMER_POLL);
        }
    }

    fn poll_clickaway(&mut self) {
        if !flyout::is_visible(self.flyout) {
            self.hide_flyout();
            return;
        }
        if self
            .clickaway_arm
            .map(|t| Instant::now() < t)
            .unwrap_or(false)
        {
            return;
        }
        unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::{
                GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_RBUTTON,
            };
            let escape = GetAsyncKeyState(VK_ESCAPE.0 as i32) < 0;
            let click = GetAsyncKeyState(VK_LBUTTON.0 as i32) < 0
                || GetAsyncKeyState(VK_RBUTTON.0 as i32) < 0;
            if !escape && !click {
                return;
            }
            let pt = tray::cursor_pos();
            if !escape && (pt_in_hwnd(self.flyout, pt) || self.pt_in_tray(pt)) {
                return;
            }
        }
        self.hide_flyout();
    }

    fn pt_in_tray(&self, pt: POINT) -> bool {
        self.tray
            .as_ref()
            .and_then(|t| t.rect())
            .map(|rc| pt_in_rect(rc, pt))
            .unwrap_or(false)
    }

    fn menu_items(&self) -> Vec<(String, u32)> {
        let auto = if crate::autostart::is_enabled() {
            "*"
        } else {
            ""
        };
        let hk = if self.config.hotkeys_enabled { "*" } else { "" };
        let rs = if self.config.restore_on_wake { "*" } else { "" };
        vec![
            ("刷新显示器".into(), ID_REFRESH),
            (String::new(), 0),
            (format!("{auto}开机启动"), ID_AUTOSTART),
            (format!("{hk}快捷键 Ctrl+Alt+↑/↓"), ID_HOTKEYS),
            (format!("{rs}唤醒后恢复亮度"), ID_RESTORE),
            (String::new(), 0),
            ("退出 LuxTray".into(), ID_EXIT),
        ]
    }

    fn set_hotkeys(&self, enable: bool) {
        unsafe {
            let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_UP);
            let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_DOWN);
            if enable {
                let mods = MOD_CONTROL | MOD_ALT | MOD_NOREPEAT;
                let _ = RegisterHotKey(Some(self.hwnd), HOTKEY_UP, mods, VK_UP.0 as u32);
                let _ = RegisterHotKey(Some(self.hwnd), HOTKEY_DOWN, mods, VK_DOWN.0 as u32);
            }
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    let mut it = s.chars();
    let take: String = it.by_ref().take(n).collect();
    if it.next().is_some() {
        format!("{take}…")
    } else {
        take
    }
}

pub fn run() -> Result<()> {
    flyout::register_class();
    register_hidden_class()?;

    let hinstance = unsafe { GetModuleHandleW(None)? };
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            CLASS,
            w!("LuxTray"),
            WS_POPUP,
            -32000,
            -32000,
            1,
            1,
            None,
            None,
            Some(hinstance.into()),
            None,
        )?
    };

    let flyout_hwnd = flyout::create()?;
    let icon_handle = icon::create_sun_icon(32)?;
    let tray = TrayIcon::add(hwnd, icon_handle, "LuxTray")?;

    let mut config = Config::load();
    match crate::autostart::sync_on_launch(config.autostart) {
        Ok(effective) => {
            if effective != config.autostart {
                config.autostart = effective;
                let _ = config.save();
            }
        }
        Err(_) => {
            if config.autostart {
                let _ = crate::autostart::set_enabled(true);
            }
        }
    }

    let monitors = monitors::enumerate().unwrap_or_default();

    let mut app = Box::new(App {
        hwnd,
        flyout: flyout_hwnd,
        monitors,
        config,
        drag_row: None,
        tray: Some(tray),
        icon: icon_handle,
        pending: HashMap::new(),
        power: None,
        clickaway_arm: None,
        last_tray_open: None,
        softdim: SoftDim::default(),
    });
    app.restore_extra_dim();
    app.update_tooltip();
    app.set_hotkeys(app.config.hotkeys_enabled);
    if let Some(tray) = app.tray.as_mut() {
        tray.balloon(
            "LuxTray",
            "已在托盘运行。点击太阳图标调节亮度；再次运行本程序会打开面板。",
        );
    }
    app.open_flyout();

    unsafe {
        let guid = windows::Win32::System::SystemServices::GUID_CONSOLE_DISPLAY_STATE;
        app.power = RegisterPowerSettingNotification(
            windows::Win32::Foundation::HANDLE(hwnd.0),
            &guid,
            windows::Win32::UI::WindowsAndMessaging::DEVICE_NOTIFY_WINDOW_HANDLE,
        )
        .ok();
    }

    let ptr = app.as_mut() as *mut App;
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);
        SetWindowLongPtrW(flyout_hwnd, GWLP_USERDATA, ptr as isize);
    }

    let _app_keep = app;

    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

fn register_hidden_class() -> Result<()> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        Ok(())
    }
}

/// Borrow `App` from `GWLP_USERDATA` with a reentrancy guard so nested modal
/// loops (e.g. `TrackPopupMenu`) cannot create overlapping `&mut App`.
pub(crate) fn with_app<R>(hwnd: HWND, f: impl FnOnce(&mut App) -> R) -> Option<R> {
    let p = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
    if p.is_null() {
        return None;
    }
    if APP_BORROWED.with(|c| c.replace(true)) {
        return None;
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            APP_BORROWED.with(|c| c.set(false));
        }
    }
    let _guard = Guard;
    // SAFETY: `p` is the Box<App> stored for the window lifetime; the Cell
    // prevents a second &mut while this call is active.
    Some(f(unsafe { &mut *p }))
}

fn pt_in_rect(rc: RECT, pt: POINT) -> bool {
    pt.x >= rc.left && pt.x < rc.right && pt.y >= rc.top && pt.y < rc.bottom
}

fn pt_in_hwnd(hwnd: HWND, pt: POINT) -> bool {
    unsafe {
        let mut rc = RECT::default();
        if windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut rc).is_err() {
            return false;
        }
        pt_in_rect(rc, pt)
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_SHOW_FLYOUT => {
            with_app(hwnd, |app| app.open_flyout());
            LRESULT(0)
        }
        WM_TRAY => {
            let event = (lparam.0 as u32) & 0xFFFF;
            const NIN_SELECT: u32 = 0x0400;
            const NIN_KEYSELECT: u32 = 0x0401;
            match event {
                NIN_SELECT | NIN_KEYSELECT => {
                    with_app(hwnd, |app| app.tray_left_click());
                }
                windows::Win32::UI::WindowsAndMessaging::WM_CONTEXTMENU
                | windows::Win32::UI::WindowsAndMessaging::WM_RBUTTONUP
                | windows::Win32::UI::WindowsAndMessaging::WM_RBUTTONDOWN => {
                    let items = with_app(hwnd, |app| {
                        app.hide_flyout();
                        app.menu_items()
                    });
                    if let Some(items) = items {
                        tray::show_context_menu(hwnd, &items);
                    }
                }
                windows::Win32::UI::WindowsAndMessaging::WM_MOUSEWHEEL => {
                    with_app(hwnd, |app| {
                        let delta = ((wparam.0 >> 16) as i16).signum() * app.config.step as i16;
                        if delta != 0 {
                            app.offset_all(delta);
                        }
                    });
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 as u32) & 0xFFFF;
            let exit = with_app(hwnd, |app| match id {
                ID_REFRESH => {
                    app.rescan();
                    false
                }
                ID_AUTOSTART => {
                    app.config.autostart = !app.config.autostart;
                    let _ = crate::autostart::set_enabled(app.config.autostart);
                    let _ = app.config.save();
                    false
                }
                ID_HOTKEYS => {
                    app.config.hotkeys_enabled = !app.config.hotkeys_enabled;
                    app.set_hotkeys(app.config.hotkeys_enabled);
                    let _ = app.config.save();
                    false
                }
                ID_RESTORE => {
                    app.config.restore_on_wake = !app.config.restore_on_wake;
                    let _ = app.config.save();
                    false
                }
                ID_EXIT => true,
                _ => false,
            })
            .unwrap_or(false);
            if exit {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            with_app(hwnd, |app| {
                let step = app.config.step as i16;
                match wparam.0 as i32 {
                    HOTKEY_UP => app.offset_all(step),
                    HOTKEY_DOWN => app.offset_all(-step),
                    _ => {}
                }
            });
            LRESULT(0)
        }
        WM_TIMER => {
            with_app(hwnd, |app| match wparam.0 {
                TIMER_FLUSH => {
                    let _ = KillTimer(Some(hwnd), TIMER_FLUSH);
                    app.flush_pending();
                }
                TIMER_RESCAN => {
                    let _ = KillTimer(Some(hwnd), TIMER_RESCAN);
                    app.rescan();
                }
                TIMER_RESTORE => {
                    let _ = KillTimer(Some(hwnd), TIMER_RESTORE);
                    if app.config.restore_on_wake {
                        app.rescan();
                        app.restore_saved();
                    }
                }
                TIMER_CLICKAWAY => app.poll_clickaway(),
                TIMER_POLL if flyout::is_visible(app.flyout) => {
                    app.refresh_from_hardware();
                    let _ =
                        windows::Win32::Graphics::Gdi::InvalidateRect(Some(app.flyout), None, true);
                }
                _ => {}
            });
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            let _ = SetTimer(Some(hwnd), TIMER_RESCAN, 1200, None);
            LRESULT(0)
        }
        WM_POWERBROADCAST => {
            const PBT_APMRESUMEAUTOMATIC: usize = 18;
            const PBT_APMRESUMESUSPEND: usize = 7;
            if wparam.0 == PBT_APMRESUMEAUTOMATIC || wparam.0 == PBT_APMRESUMESUSPEND {
                let _ = SetTimer(Some(hwnd), TIMER_RESTORE, 2500, None);
            }
            LRESULT(1)
        }
        WM_DESTROY => {
            with_app(hwnd, |app| {
                app.set_hotkeys(false);
                if let Some(p) = app.power.take() {
                    let _ = UnregisterPowerSettingNotification(p);
                }
                app.tray.take();
                icon::destroy(app.icon);
                app.hide_flyout();
                app.softdim.destroy_all();
            });
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn find_running() -> Option<HWND> {
    unsafe { windows::Win32::UI::WindowsAndMessaging::FindWindowW(CLASS, w!("LuxTray")).ok() }
}

pub fn activate_existing(hidden: HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, BringWindowToTop, FindWindowW, GetWindowThreadProcessId,
        SendMessageTimeoutW, SetForegroundWindow, SetWindowPos, ShowWindow, HWND_TOPMOST,
        SMTO_ABORTIFHUNG, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_RESTORE, SW_SHOW,
    };
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hidden, Some(&mut pid));
        if pid != 0 {
            let _ = AllowSetForegroundWindow(pid);
        }
        let _ = SendMessageTimeoutW(
            hidden,
            WM_SHOW_FLYOUT,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            2000,
            None,
        );
        if let Ok(flyout) = FindWindowW(crate::flyout::CLASS, w!("LuxTray")) {
            let _ = ShowWindow(flyout, SW_SHOW);
            let _ = ShowWindow(flyout, SW_RESTORE);
            let _ = SetWindowPos(
                flyout,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
            let _ = BringWindowToTop(flyout);
            let _ = SetForegroundWindow(flyout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("显示器", 8), "显示器");
    }

    #[test]
    fn truncate_adds_ellipsis() {
        assert_eq!(truncate("abcdefghijk", 10), "abcdefghij…");
        assert_eq!(truncate("内置屏幕亮度", 3), "内置屏…");
    }
}
