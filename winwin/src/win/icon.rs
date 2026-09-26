//! winwin's icon, drawn at run time at the size Windows asks for, so that
//! there is no resource file to keep in step: in the notification area, and
//! on the settings window's title bar and taskbar button.

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject,
};
use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, HICON, ICONINFO, IDI_APPLICATION, LoadIconW, SM_CXICON, SM_CXSMICON,
    SYSTEM_METRICS_INDEX,
};

const BACKGROUND: u32 = 0xFF_2F_6F_EB;
const FOREGROUND: u32 = 0xFF_FF_FF_FF;

/// A window outline on a rounded blue square, its left half filled: a window
/// snapped to one side. `size` is the edge in pixels; the result is
/// top-down ARGB.
fn pixels(size: usize) -> Vec<u32> {
    let s = size as f32;
    let u = s / 16.0;
    let radius = 3.0 * u;
    let stroke = u.round().max(1.0);
    let (fl, fr, ft, fb) = (3.0 * u, 13.0 * u, 4.0 * u, 12.0 * u);
    let mut out = vec![0u32; size * size];
    for y in 0..size {
        for x in 0..size {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Distance outside the rounded square's straight part.
            let dx = (radius - px).max(px - (s - radius)).max(0.0);
            let dy = (radius - py).max(py - (s - radius)).max(0.0);
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
            let inside_frame = px >= fl && px < fr && py >= ft && py < fb;
            let on_stroke = inside_frame
                && (px < fl + stroke || px >= fr - stroke || py < ft + stroke || py >= fb - stroke);
            let in_left_half = inside_frame && px < (fl + fr) / 2.0;
            out[y * size + x] = if on_stroke || in_left_half {
                FOREGROUND
            } else {
                BACKGROUND
            };
        }
    }
    out
}

/// The small icon: the notification area and title bars.
pub fn create(dpi: u32) -> HICON {
    create_sized(SM_CXSMICON, dpi, 16)
}

/// The large icon: taskbar buttons and Alt+Tab.
pub fn create_large(dpi: u32) -> HICON {
    create_sized(SM_CXICON, dpi, 32)
}

fn create_sized(metric: SYSTEM_METRICS_INDEX, dpi: u32, least: i32) -> HICON {
    let size = unsafe { GetSystemMetricsForDpi(metric, dpi) }.max(least);
    draw(size).unwrap_or_else(|| unsafe { LoadIconW(None, IDI_APPLICATION).unwrap_or_default() })
}

fn draw(size: i32) -> Option<HICON> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            // Negative: rows run top to bottom, as `pixels` writes them.
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let color =
        unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) }.ok()?;
    let src = pixels(size as usize);
    // SAFETY: the section is size × size 32-bit pixels, as requested above.
    unsafe { std::ptr::copy_nonoverlapping(src.as_ptr(), bits.cast::<u32>(), src.len()) };
    // With a 32-bit color bitmap the alpha channel decides transparency; the
    // mask is required but unused.
    let mask = unsafe { CreateBitmap(size, size, 1, 1, None) };
    let icon = unsafe {
        CreateIconIndirect(&ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        })
    };
    unsafe {
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
    }
    icon.ok()
}
