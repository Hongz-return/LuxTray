use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, EndPaint, FillRect,
    InvalidateRect, Rectangle, SelectObject, SetBkMode, SetTextColor, TextOutW, CLEARTYPE_QUALITY,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD,
    HBRUSH, HFONT, OUT_TT_PRECIS, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, GetSystemMetrics, LoadCursorW, MoveWindow,
    RegisterClassExW, SetWindowPos, ShowWindow, CS_DROPSHADOW, CS_HREDRAW, CS_VREDRAW,
    CW_USEDEFAULT, HWND_TOPMOST, IDC_ARROW, SM_CXSCREEN, SM_CYCAPTION, SM_CYSCREEN, SWP_SHOWWINDOW,
    SW_HIDE, SW_SHOW, WM_CLOSE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_PAINT, WNDCLASSEXW, WS_CAPTION, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_SYSMENU,
};

use crate::app;

pub const CLASS: windows::core::PCWSTR = w!("LuxTrayFlyout");
const WM_MOUSELEAVE: u32 = 0x02A3;

#[derive(Clone, Copy)]
struct Theme {
    bg: COLORREF,
    text: COLORREF,
    muted: COLORREF,
    track: COLORREF,
    accent: COLORREF,
    thumb: COLORREF,
}

impl Theme {
    fn current() -> Self {
        if system_light() {
            Self {
                bg: rgb(245, 245, 245),
                text: rgb(28, 28, 28),
                muted: rgb(96, 96, 96),
                track: rgb(218, 218, 218),
                accent: rgb(255, 176, 32),
                thumb: rgb(255, 255, 255),
            }
        } else {
            Self {
                bg: rgb(32, 32, 32),
                text: rgb(250, 250, 250),
                muted: rgb(170, 170, 170),
                track: rgb(64, 64, 64),
                accent: rgb(255, 185, 56),
                thumb: rgb(255, 255, 255),
            }
        }
    }
}

fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16))
}

fn system_light() -> bool {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut data = 0u32;
    let mut size = 4u32;
    unsafe {
        if RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut _ as *mut _),
            Some(&mut size),
        )
        .is_ok()
        {
            data != 0
        } else {
            false
        }
    }
}

pub fn register_class() {
    unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW | CS_DROPSHADOW,
            lpfnWndProc: Some(flyout_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);
    }
}

pub fn create() -> windows::core::Result<HWND> {
    unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            CLASS,
            w!("LuxTray"),
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            320,
            120,
            None,
            None,
            Some(hinstance.into()),
            None,
        )?;
        let pref = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const _,
            std::mem::size_of_val(&pref) as u32,
        );
        Ok(hwnd)
    }
}

pub fn hide(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

pub fn is_visible(hwnd: HWND) -> bool {
    unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() }
}

pub fn show_near(hwnd: HWND, tray_rect: RECT, rows: usize) {
    unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96);
        let scale = |v: i32| v * dpi as i32 / 96;
        let width = scale(340);
        let header = scale(52);
        let row_h = scale(52);
        let caption = GetSystemMetrics(SM_CYCAPTION);
        let height = header + row_h * rows.max(1) as i32 + scale(12) + caption;

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let tray_ok = (tray_rect.right - tray_rect.left).abs() > 4
            && (tray_rect.bottom - tray_rect.top).abs() > 4;

        let mut x;
        let mut y;
        if tray_ok {
            x = (tray_rect.left + tray_rect.right) / 2 - width / 2;
            y = tray_rect.top - height - scale(8);
            if y < scale(8) {
                y = tray_rect.bottom + scale(8);
            }
        } else {
            x = (screen_w - width) / 2;
            y = (screen_h - height) / 2;
        }
        x = x.clamp(scale(8), (screen_w - width - scale(8)).max(scale(8)));
        y = y.clamp(scale(8), (screen_h - height - scale(8)).max(scale(8)));

        let _ = MoveWindow(hwnd, x, y, width, height, true);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
        let _ = InvalidateRect(Some(hwnd), None, true);
    }
}

struct RowHit {
    index: usize, // 0 = all, 1.. = monitor
    track: RECT,
}

fn layout_rows(hwnd: HWND, n_monitors: usize) -> (i32, Vec<RowHit>) {
    unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96);
        let scale = |v: i32| v * dpi as i32 / 96;
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let header = scale(52);
        let row_h = scale(52);
        let pad = scale(16);
        let mut hits = Vec::new();
        let rows = n_monitors + 1;
        for i in 0..rows {
            let top = header + i as i32 * row_h;
            let track = RECT {
                left: pad + scale(86),
                top: top + scale(18),
                right: rc.right - pad - scale(50),
                bottom: top + scale(34),
            };
            hits.push(RowHit { index: i, track });
        }
        (row_h, hits)
    }
}

fn level_from_x(track: RECT, x: i32) -> i16 {
    let w = (track.right - track.left).max(1);
    let t = (x - track.left).clamp(0, w) as f32 / w as f32;
    let span = (crate::monitors::LEVEL_MAX - crate::monitors::LEVEL_MIN) as f32;
    let v = crate::monitors::LEVEL_MIN as f32 + t * span;
    v.round().clamp(
        crate::monitors::LEVEL_MIN as f32,
        crate::monitors::LEVEL_MAX as f32,
    ) as i16
}

unsafe extern "system" fn flyout_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    match msg {
        WM_PAINT => {
            paint(hwnd);
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            app::with_app(hwnd, |app| {
                let x = (lparam.0 as i16) as i32;
                let y = ((lparam.0 >> 16) as i16) as i32;
                let (_, hits) = layout_rows(hwnd, app.monitors.len());
                for hit in hits {
                    if y >= hit.track.top - 10
                        && y <= hit.track.bottom + 10
                        && x >= hit.track.left - 8
                        && x <= hit.track.right + 8
                    {
                        app.drag_row = Some(hit.index);
                        let level = level_from_x(hit.track, x);
                        app.apply_slider(hit.index, level);
                        let _ = SetCapture(hwnd);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                        break;
                    }
                }
            });
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_MOUSEMOVE => {
            app::with_app(hwnd, |app| {
                if let Some(idx) = app.drag_row {
                    let x = (lparam.0 as i16) as i32;
                    let (_, hits) = layout_rows(hwnd, app.monitors.len());
                    if let Some(hit) = hits.into_iter().find(|h| h.index == idx) {
                        let level = level_from_x(hit.track, x);
                        app.apply_slider(idx, level);
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                } else {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                }
            });
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_LBUTTONUP | WM_MOUSELEAVE => {
            app::with_app(hwnd, |app| {
                app.drag_row = None;
            });
            let _ = ReleaseCapture();
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_CLOSE => {
            hide(hwnd);
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            app::with_app(hwnd, |app| {
                let delta = (wparam.0 >> 16) as i16;
                let step = app.config.step as i16;
                let d = if delta > 0 { step } else { -step };
                app.offset_all(d);
                let _ = InvalidateRect(Some(hwnd), None, true);
            });
            windows::Win32::Foundation::LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn paint(hwnd: HWND) {
    let _ = app::with_app(hwnd, |app| paint_inner(hwnd, app));
}

fn paint_inner(hwnd: HWND, app: &crate::app::App) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let theme = Theme::current();
        let dpi = GetDpiForWindow(hwnd).max(96);
        let scale = |v: i32| v * dpi as i32 / 96;
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);

        let bg = CreateSolidBrush(theme.bg);
        FillRect(hdc, &rc, bg);

        let title_font = make_font(scale(14), true);
        let body_font = make_font(scale(12), false);
        let old_font = SelectObject(hdc, title_font.into());
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, theme.text);
        text_out(hdc, scale(16), scale(10), "亮度");
        SetTextColor(hdc, theme.muted);
        SelectObject(hdc, body_font.into());
        text_out(hdc, scale(16), scale(28), "低于 0 为软件压暗");

        SelectObject(hdc, body_font.into());
        SetTextColor(hdc, theme.muted);

        let rows: Vec<(String, i16)> = {
            let avg = if app.monitors.is_empty() {
                0
            } else {
                (app.monitors.iter().map(|m| m.current as i32).sum::<i32>()
                    / app.monitors.len() as i32) as i16
            };
            let mut rows = vec![("全部".to_string(), avg)];
            for m in &app.monitors {
                rows.push((m.name.clone(), m.current));
            }
            rows
        };

        if app.monitors.is_empty() {
            SetTextColor(hdc, theme.muted);
            text_out(
                hdc,
                scale(16),
                scale(56),
                "未检测到可调节显示器。请在 OSD 中开启 DDC/CI。",
            );
        }

        let (_, hits) = layout_rows(hwnd, app.monitors.len());
        for (row, hit) in rows.iter().zip(hits.iter()) {
            SetTextColor(hdc, theme.text);
            SelectObject(hdc, body_font.into());
            let label: String = row.0.chars().take(8).collect();
            text_out(hdc, scale(16), hit.track.top - scale(2), &label);

            // track
            let track_brush = CreateSolidBrush(theme.track);
            fill_roundish(hdc, hit.track, track_brush);

            let w = (hit.track.right - hit.track.left).max(1);
            let span = (crate::monitors::LEVEL_MAX - crate::monitors::LEVEL_MIN) as i32;
            let pos = (row.1 - crate::monitors::LEVEL_MIN) as i32;
            let fill_w = w * pos / span.max(1);
            if fill_w > 0 {
                let fill = RECT {
                    left: hit.track.left,
                    top: hit.track.top,
                    right: hit.track.left + fill_w,
                    bottom: hit.track.bottom,
                };
                let ab = CreateSolidBrush(theme.accent);
                fill_roundish(hdc, fill, ab);
                let _ = DeleteObject(ab.into());
            }

            let zero_x = hit.track.left + w * (-crate::monitors::LEVEL_MIN) as i32 / span.max(1);
            let tick = RECT {
                left: zero_x,
                top: hit.track.top,
                right: zero_x + scale(1),
                bottom: hit.track.bottom,
            };
            let tick_brush = CreateSolidBrush(theme.muted);
            FillRect(hdc, &tick, tick_brush);
            let _ = DeleteObject(tick_brush.into());
            let _ = DeleteObject(track_brush.into());

            // thumb
            let tx = hit.track.left + fill_w;
            let cy = (hit.track.top + hit.track.bottom) / 2;
            let r = scale(7);
            let thumb = RECT {
                left: tx - r,
                top: cy - r,
                right: tx + r,
                bottom: cy + r,
            };
            let tb = CreateSolidBrush(theme.thumb);
            let pen = CreatePen(PS_SOLID, 1, theme.accent);
            let old_pen = SelectObject(hdc, pen.into());
            let old_br = SelectObject(hdc, tb.into());
            let _ = Rectangle(hdc, thumb.left, thumb.top, thumb.right, thumb.bottom);
            SelectObject(hdc, old_pen);
            SelectObject(hdc, old_br);
            let _ = DeleteObject(tb.into());
            let _ = DeleteObject(pen.into());

            SetTextColor(hdc, theme.muted);
            let pct = format!("{}", row.1);
            text_out(
                hdc,
                hit.track.right + scale(8),
                hit.track.top - scale(2),
                &pct,
            );
        }

        SelectObject(hdc, old_font);
        let _ = DeleteObject(title_font.into());
        let _ = DeleteObject(body_font.into());
        let _ = DeleteObject(bg.into());
        let _ = EndPaint(hwnd, &ps);
    }
}

fn fill_roundish(hdc: windows::Win32::Graphics::Gdi::HDC, rc: RECT, brush: HBRUSH) {
    unsafe {
        FillRect(hdc, &rc, brush);
    }
}

fn make_font(px: i32, bold: bool) -> HFONT {
    unsafe {
        CreateFontW(
            -px,
            0,
            0,
            0,
            if bold {
                FW_SEMIBOLD.0 as i32
            } else {
                FW_NORMAL.0 as i32
            },
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_TT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32,
            w!("Segoe UI"),
        )
    }
}

fn text_out(hdc: windows::Win32::Graphics::Gdi::HDC, x: i32, y: i32, s: &str) {
    let wide: Vec<u16> = s.encode_utf16().collect();
    unsafe {
        let _ = TextOutW(hdc, x, y, &wide);
    }
}

#[cfg(test)]
mod tests {
    use super::level_from_x;
    use crate::monitors::{LEVEL_MAX, LEVEL_MIN};
    use windows::Win32::Foundation::RECT;

    fn track(left: i32, right: i32) -> RECT {
        RECT {
            left,
            top: 0,
            right,
            bottom: 10,
        }
    }

    #[test]
    fn level_from_x_ends() {
        let t = track(0, 150);
        assert_eq!(level_from_x(t, 0), LEVEL_MIN);
        assert_eq!(level_from_x(t, 150), LEVEL_MAX);
        assert_eq!(level_from_x(t, -10), LEVEL_MIN);
        assert_eq!(level_from_x(t, 999), LEVEL_MAX);
    }

    #[test]
    fn level_from_x_zero_at_one_third() {
        let t = track(0, 150);
        // LEVEL_MIN=-50, LEVEL_MAX=100, span=150; zero is at t=50/150
        assert_eq!(level_from_x(t, 50), 0);
    }
}
