use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject, GetDC, ReleaseDC};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, ICONINFO, HICON};

pub fn create_sun_icon(size: i32) -> anyhow::Result<HICON> {
    let s = size.max(16) as usize;
    let mut bgra = vec![0u8; s * s * 4];

    let cx = (s as f32 - 1.0) * 0.5;
    let cy = cx;
    let r_body = s as f32 * 0.22;
    let r_inner = s as f32 * 0.14;
    let r_ray = s as f32 * 0.42;
    let r_ray_in = s as f32 * 0.28;

    for y in 0..s {
        for x in 0..s {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let mut a = 0.0f32;

            if dist <= r_body {
                a = 1.0;
                if dist < r_inner {
                    a = 0.92;
                }
            }

            let angle = dy.atan2(dx);
            let sector = std::f32::consts::PI / 8.0;
            let mut wrapped = angle;
            while wrapped < 0.0 {
                wrapped += std::f32::consts::PI * 2.0;
            }
            let local = (wrapped + sector) % (sector * 2.0) - sector;
            let ray_width = 0.18 + 0.08 * (dist / r_ray).clamp(0.0, 1.0);
            if dist > r_ray_in && dist < r_ray && local.abs() < ray_width {
                let fade = 1.0 - ((dist - r_ray_in) / (r_ray - r_ray_in)).clamp(0.0, 1.0);
                a = a.max(fade * 0.95);
            }

            if a > 0.02 {
                let i = (y * s + x) * 4;
                bgra[i] = 40;
                bgra[i + 1] = 186;
                bgra[i + 2] = 255;
                bgra[i + 3] = (a * 255.0) as u8;
            }
        }
    }

    unsafe { hicon_from_bgra(&bgra, s as i32, s as i32) }
}

unsafe fn hicon_from_bgra(bgra: &[u8], w: i32, h: i32) -> anyhow::Result<HICON> {
    let hdc = GetDC(None);
    if hdc.is_invalid() {
        anyhow::bail!("GetDC failed");
    }
    let color = CreateBitmap(w, h, 1, 32, Some(bgra.as_ptr() as *const _));
    let mask = CreateBitmap(w, h, 1, 1, None);
    let _ = ReleaseDC(None, hdc);

    if color.is_invalid() || mask.is_invalid() {
        if !color.is_invalid() {
            let _ = DeleteObject(color.into());
        }
        if !mask.is_invalid() {
            let _ = DeleteObject(mask.into());
        }
        anyhow::bail!("CreateBitmap failed");
    }

    let info = ICONINFO {
        fIcon: true.into(),
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask,
        hbmColor: color,
    };
    let icon = CreateIconIndirect(&info)?;
    let _ = DeleteObject(color.into());
    let _ = DeleteObject(mask.into());
    Ok(icon)
}

pub fn destroy(icon: HICON) {
    unsafe {
        let _ = DestroyIcon(icon);
    }
}
