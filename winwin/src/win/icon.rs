//! winwin's icon as Windows icon handles, drawn at run time at the size
//! Windows asks for: in the notification area, and on the settings window's
//! title bar. The picture itself is art.rs, which build.rs also draws into
//! the executable's icon.

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject,
};
use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, HICON, ICONINFO, IDI_APPLICATION, LoadIconW, SM_CXICON, SM_CXSMICON,
    SYSTEM_METRICS_INDEX,
};

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
    let src = crate::art::pixels(size as usize);
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
