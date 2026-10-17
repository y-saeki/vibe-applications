//! eleven-zip's icon as Windows icon handles, for the settings window's
//! title bar and taskbar button. They come from the icon build.rs drew into
//! the executable from icon.svg, at the size Windows asks for.

use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows::Win32::UI::WindowsAndMessaging::{
    HICON, IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTCOLOR, LoadIconW, LoadImageW, SM_CXICON,
    SM_CXSMICON, SYSTEM_METRICS_INDEX,
};
use windows::core::PCWSTR;

/// The icon group's resource id (`GROUP_ID` in icon_res.rs).
const ICON_ID: u16 = 1;

/// The small icon: the title bar.
pub fn create(dpi: u32) -> HICON {
    load(SM_CXSMICON, dpi, 16)
}

/// The large icon: taskbar buttons and Alt+Tab.
pub fn create_large(dpi: u32) -> HICON {
    load(SM_CXICON, dpi, 32)
}

fn load(metric: SYSTEM_METRICS_INDEX, dpi: u32, least: i32) -> HICON {
    let size = unsafe { GetSystemMetricsForDpi(metric, dpi) }.max(least);
    let loaded = unsafe {
        let module = GetModuleHandleW(PCWSTR::null()).ok();
        LoadImageW(
            module.map(Into::into),
            // MAKEINTRESOURCE: the id in place of a name.
            PCWSTR(usize::from(ICON_ID) as *const u16),
            IMAGE_ICON,
            size,
            size,
            LR_DEFAULTCOLOR,
        )
    };
    match loaded {
        Ok(handle) => HICON(handle.0),
        Err(_) => unsafe { LoadIconW(None, IDI_APPLICATION).unwrap_or_default() },
    }
}
