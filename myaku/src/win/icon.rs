//! The notification-area icon, drawn at run time at the size and in the
//! colour the taskbar needs. The picture is `myaku::art`.

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject,
};
use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, HICON, ICONINFO, IDI_APPLICATION, LoadIconW, SM_CXSMICON,
};

/// The glyph at the small-icon size for `dpi`, white on a dark taskbar and
/// black on a light one.
pub fn tray(dpi: u32, dark_taskbar: bool) -> HICON {
    let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, dpi) }.max(16);
    draw(size, &myaku::art::tray_pixels(size as usize, dark_taskbar))
        .unwrap_or_else(|| unsafe { LoadIconW(None, IDI_APPLICATION).unwrap_or_default() })
}

fn draw(size: i32, src: &[u32]) -> Option<HICON> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            // Negative: rows run top to bottom, as the pixels are written.
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
    // SAFETY: the section is size × size 32-bit pixels, as requested above.
    unsafe { std::ptr::copy_nonoverlapping(src.as_ptr(), bits.cast::<u32>(), src.len()) };
    // With a 32-bit colour bitmap the alpha channel decides transparency;
    // the mask is required but unused.
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
