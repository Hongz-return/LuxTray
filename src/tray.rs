use anyhow::Result;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconGetRect, Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP,
    NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NOTIFYICONDATAW,
    NOTIFYICONIDENTIFIER, NOTIFYICON_VERSION_4,
};
use windows::Win32::UI::WindowsAndMessaging::{HICON, WM_APP};

pub const WM_TRAY: u32 = WM_APP + 1;
pub const TRAY_ID: u32 = 1;

pub struct TrayIcon {
    data: NOTIFYICONDATAW,
}

impl TrayIcon {
    pub fn add(hwnd: HWND, icon: HICON, tip: &str) -> Result<Self> {
        let mut data = NOTIFYICONDATAW::default();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = TRAY_ID;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = icon;
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        copy_tip(&mut data.szTip, tip);

        unsafe {
            Shell_NotifyIconW(NIM_ADD, &data).ok()?;
            Shell_NotifyIconW(NIM_SETVERSION, &data).ok()?;
        }
        Ok(Self { data })
    }

    pub fn balloon(&mut self, title: &str, body: &str) {
        copy_tip(&mut self.data.szTip, "LuxTray");
        copy_wide(&mut self.data.szInfoTitle, title);
        copy_wide(&mut self.data.szInfo, body);
        self.data.dwInfoFlags = NIIF_INFO;
        self.data.uFlags = NIF_INFO | NIF_TIP | NIF_SHOWTIP;
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &self.data);
        }
        self.data.uFlags = NIF_TIP | NIF_SHOWTIP;
    }

    pub fn set_tooltip(&mut self, tip: &str) {
        copy_tip(&mut self.data.szTip, tip);
        self.data.uFlags = NIF_TIP | NIF_SHOWTIP;
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &self.data);
        }
    }

    pub fn rect(&self) -> Option<RECT> {
        let id = NOTIFYICONIDENTIFIER {
            cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
            hWnd: self.data.hWnd,
            uID: self.data.uID,
            guidItem: windows::core::GUID::zeroed(),
        };
        let rc = unsafe { Shell_NotifyIconGetRect(&id).ok()? };
        Some(rc)
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.data);
        }
    }
}

fn copy_tip(buf: &mut [u16; 128], tip: &str) {
    copy_wide(buf, tip);
}

fn copy_wide(buf: &mut [u16], text: &str) {
    buf.fill(0);
    for (i, c) in text.encode_utf16().take(buf.len().saturating_sub(1)).enumerate() {
        buf[i] = c;
    }
}

pub fn cursor_pos() -> POINT {
    let mut pt = POINT::default();
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt);
    }
    pt
}

pub fn show_context_menu(hwnd: HWND, items: &[(&str, u32)]) {
    use windows::Win32::UI::WindowsAndMessaging::{
        CreatePopupMenu, DestroyMenu, InsertMenuW, TrackPopupMenu, HMENU, MF_BYPOSITION,
        MF_CHECKED, MF_SEPARATOR, MF_STRING, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON,
        SetForegroundWindow,
    };
    unsafe {
        let menu = CreatePopupMenu().unwrap_or(HMENU::default());
        if menu.is_invalid() {
            return;
        }
        for (i, (label, id)) in items.iter().enumerate() {
            if *id == 0 {
                let _ = InsertMenuW(menu, i as u32, MF_BYPOSITION | MF_SEPARATOR, 0, PCWSTR::null());
                continue;
            }
            let mut flags = MF_BYPOSITION | MF_STRING;
            let text = if label.starts_with('*') {
                flags |= MF_CHECKED;
                &label[1..]
            } else {
                *label
            };
            let mut wide: Vec<u16> = text.encode_utf16().collect();
            wide.push(0);
            let _ = InsertMenuW(
                menu,
                i as u32,
                flags,
                *id as usize,
                PCWSTR(wide.as_ptr()),
            );
        }
        let pt = cursor_pos();
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_BOTTOMALIGN | TPM_LEFTALIGN | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
            Some(hwnd),
            windows::Win32::UI::WindowsAndMessaging::WM_NULL,
            WPARAM(0),
            LPARAM(0),
        );
        let _ = DestroyMenu(menu);
        let _ = w!("LuxTray");
    }
}
