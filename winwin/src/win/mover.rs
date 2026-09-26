//! Moving and resizing the foreground window.

use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, HWND, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, GetClassNameW, GetForegroundWindow, GetShellWindow, GetWindowRect,
    IsHungAppWindow, IsIconic, IsWindowVisible, IsZoomed, SW_RESTORE, SWP_NOACTIVATE,
    SWP_NOOWNERZORDER, SWP_NOZORDER, SetWindowPos, ShowWindow,
};

use crate::layout::{Placement, Rect};

pub enum MoveError {
    /// The window belongs to a process running with higher privileges, which
    /// Windows does not let an ordinary process move.
    AccessDenied,
}

fn to_rect(r: RECT) -> Rect {
    Rect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    }
}

/// Windows that belong to the desktop itself, which a shortcut should leave
/// alone even when they have the focus.
const SHELL_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
];

fn is_shell_window(hwnd: HWND) -> bool {
    if hwnd == unsafe { GetShellWindow() } {
        return true;
    }
    let mut buf = [0u16; 64];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) } as usize;
    let class = String::from_utf16_lossy(&buf[..len]);
    SHELL_CLASSES.contains(&class.as_str())
}

/// The window rectangle and the visible frame inside it. They differ by the
/// invisible resize borders Windows 10 and later draw around most windows.
fn measure(hwnd: HWND) -> Option<(Rect, Rect)> {
    let mut window = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut window) }.ok()?;
    let mut frame = RECT::default();
    let got_frame = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut frame as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        )
    };
    let frame = if got_frame.is_ok() { frame } else { window };
    Some((to_rect(window), to_rect(frame)))
}

/// Places the window so that its visible frame, not its outer rectangle,
/// lands on `target`.
fn set_frame(hwnd: HWND, target: Rect) -> Result<(), MoveError> {
    let Some((window, frame)) = measure(hwnd) else {
        return Ok(());
    };
    let left = frame.left - window.left;
    let top = frame.top - window.top;
    let right = window.right - frame.right;
    let bottom = window.bottom - frame.bottom;
    let result = unsafe {
        SetWindowPos(
            hwnd,
            None,
            target.left - left,
            target.top - top,
            target.width() + left + right,
            target.height() + top + bottom,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        )
    };
    match result {
        Err(e) if e.code() == ERROR_ACCESS_DENIED.to_hresult() => Err(MoveError::AccessDenied),
        // Anything else is a window that went away or refused; nothing to
        // tell the user about.
        _ => Ok(()),
    }
}

pub fn place_foreground(placement: &Placement) -> Result<(), MoveError> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return Ok(());
    }
    let hwnd = unsafe { GetAncestor(hwnd, GA_ROOT) };
    if hwnd.is_invalid()
        // winwin's own hidden window is the foreground one right after its
        // menu in the notification area closes.
        || !unsafe { IsWindowVisible(hwnd) }.as_bool()
        || is_shell_window(hwnd)
        || unsafe { IsIconic(hwnd) }.as_bool()
        // Moving a window that does not answer would block winwin until it
        // does.
        || unsafe { IsHungAppWindow(hwnd) }.as_bool()
    {
        return Ok(());
    }

    if unsafe { IsZoomed(hwnd) }.as_bool() {
        let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
    }

    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return Ok(());
    }
    let (mut dpi_x, mut dpi_y) = (96, 96);
    let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    let target = placement.resolve(to_rect(info.rcWork), f64::from(dpi_x) / 96.0);

    set_frame(hwnd, target)?;
    // The invisible borders can change with the size (a window that reached
    // its minimum, or one that redraws its frame), in which case the first
    // pass measured the wrong ones. One more pass settles it.
    if let Some((_, frame)) = measure(hwnd)
        && frame != target
    {
        set_frame(hwnd, target)?;
    }
    Ok(())
}
