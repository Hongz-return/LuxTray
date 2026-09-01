use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetStockObject, BLACK_BRUSH, HBRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, IsWindowVisible, MoveWindow,
    RegisterClassExW, SetLayeredWindowAttributes, SetWindowPos, ShowWindow, CS_HREDRAW,
    CS_VREDRAW, HWND_TOPMOST, LWA_ALPHA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    SW_HIDE, SW_SHOWNOACTIVATE, WM_NCHITTEST, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::monitors::{self, Monitor};

const CLASS: windows::core::PCWSTR = w!("LuxTrayDim");

pub struct SoftDim {
    windows: Vec<(String, HWND)>,
    class_ready: bool,
}

impl Default for SoftDim {
    fn default() -> Self {
        Self {
            windows: Vec::new(),
            class_ready: false,
        }
    }
}

impl SoftDim {
    pub fn sync(&mut self, monitors: &[Monitor], flyout: HWND) {
        self.ensure_class();
        self.windows.retain(|(id, hwnd)| {
            if monitors.iter().any(|m| &m.id == id) {
                true
            } else {
                unsafe {
                    let _ = DestroyWindow(*hwnd);
                }
                false
            }
        });
        for m in monitors {
            let alpha = monitors::extra_alpha(m.current);
            let existing = self
                .windows
                .iter()
                .find(|(id, _)| id == &m.id)
                .map(|(_, hwnd)| *hwnd);
            if let Some(hwnd) = existing {
                update(hwnd, m.rect, alpha);
            } else if alpha > 0 {
                if let Ok(hwnd) = create(m.rect, alpha) {
                    self.windows.push((m.id.clone(), hwnd));
                }
            }
        }
        raise_flyout(flyout);
    }

    pub fn destroy_all(&mut self) {
        for (_, hwnd) in self.windows.drain(..) {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }

    fn ensure_class(&mut self) {
        if self.class_ready {
            return;
        }
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(overlay_proc),
                hInstance: hinstance.into(),
                hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0 as *mut core::ffi::c_void),
                lpszClassName: CLASS,
                ..Default::default()
            };
            let _ = RegisterClassExW(&wc);
        }
        self.class_ready = true;
    }
}

impl Drop for SoftDim {
    fn drop(&mut self) {
        self.destroy_all();
    }
}

fn create(rect: RECT, alpha: u8) -> windows::core::Result<HWND> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED
                | WS_EX_TRANSPARENT
                | WS_EX_NOACTIVATE
                | WS_EX_TOOLWINDOW
                | WS_EX_TOPMOST,
            CLASS,
            w!(""),
            WS_POPUP,
            rect.left,
            rect.top,
            (rect.right - rect.left).max(1),
            (rect.bottom - rect.top).max(1),
            None,
            None,
            Some(hinstance.into()),
            None,
        )?;
        update(hwnd, rect, alpha);
        Ok(hwnd)
    }
}

fn update(hwnd: HWND, rect: RECT, alpha: u8) {
    unsafe {
        let _ = MoveWindow(
            hwnd,
            rect.left,
            rect.top,
            (rect.right - rect.left).max(1),
            (rect.bottom - rect.top).max(1),
            false,
        );
        if alpha == 0 {
            let _ = ShowWindow(hwnd, SW_HIDE);
            return;
        }
        let _ = SetLayeredWindowAttributes(hwnd, windows::Win32::Foundation::COLORREF(0), alpha, LWA_ALPHA);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            rect.left,
            rect.top,
            (rect.right - rect.left).max(1),
            (rect.bottom - rect.top).max(1),
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
}

pub fn raise_flyout(flyout: HWND) {
    unsafe {
        if flyout.0.is_null() || !IsWindowVisible(flyout).as_bool() {
            return;
        }
        let _ = SetWindowPos(
            flyout,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

unsafe extern "system" fn overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCHITTEST {
        return LRESULT(-1);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
