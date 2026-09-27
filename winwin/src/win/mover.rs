//! Moving and resizing the foreground window.

use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, HWND, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CURSORINFO, GA_ROOT, GetAncestor, GetClassNameW, GetCursorInfo, GetForegroundWindow,
    GetShellWindow, GetWindowRect, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE,
    IsHungAppWindow, IsIconic, IsWindowVisible, IsZoomed, LoadCursorW, SW_RESTORE,
    SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOZORDER, SetWindowPos,
    ShowWindow,
};
use windows::core::{s, w};

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
    let window = window_rect(hwnd)?;
    let mut frame = RECT::default();
    let got_frame = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut frame as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        )
    };
    let frame = if got_frame.is_ok() {
        to_rect(frame)
    } else {
        window
    };
    Some((window, frame))
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

/// A window a shortcut has moved: where it was and where it went, and the
/// DPI of its monitor.
pub struct Placed {
    pub hwnd: HWND,
    pub before: Rect,
    pub dpi: u32,
    pub after: Rect,
}

/// Places the foreground window, and says where it was and where it went
/// when it moved.
pub fn place_foreground(placement: &Placement) -> Result<Option<Placed>, MoveError> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return Ok(None);
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
        return Ok(None);
    }

    if unsafe { IsZoomed(hwnd) }.as_bool() {
        let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
    }
    // After the restore, so that a maximized window keeps the size it has
    // when it is not.
    let Some(before) = window_rect(hwnd) else {
        return Ok(None);
    };
    let dpi = monitor_dpi(hwnd);
    place(hwnd, placement)?;
    Ok(window_rect(hwnd).map(|after| Placed {
        hwnd,
        before,
        dpi,
        after,
    }))
}

pub fn window_rect(hwnd: HWND) -> Option<Rect> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }.ok()?;
    Some(to_rect(rect))
}

/// The DPI of the monitor `hwnd` is on, whether or not the window itself
/// knows about DPI.
pub fn monitor_dpi(hwnd: HWND) -> u32 {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let (mut x, mut y) = (0, 0);
    match unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) } {
        Ok(()) if x > 0 => x,
        _ => 96,
    }
}

/// Whether Windows has snapped or maximized `hwnd`. IsWindowArranged
/// arrived in Windows 10 1903 and has no import library, so it is looked up;
/// before that, only maximized counts.
pub fn is_arranged(hwnd: HWND) -> bool {
    if unsafe { IsZoomed(hwnd) }.as_bool() {
        return true;
    }
    let Ok(user32) = (unsafe { GetModuleHandleW(w!("user32.dll")) }) else {
        return false;
    };
    let Some(f) = (unsafe { GetProcAddress(user32, s!("IsWindowArranged")) }) else {
        return false;
    };
    // SAFETY: IsWindowArranged takes an HWND and returns a BOOL.
    let is_window_arranged: unsafe extern "system" fn(HWND) -> windows::core::BOOL =
        unsafe { std::mem::transmute(f) };
    unsafe { is_window_arranged(hwnd) }.as_bool()
}

/// Whether the cursor is one of the resizing arrows, which is what tells a
/// resize from a move when either starts.
pub fn is_resize_cursor() -> bool {
    let mut info = CURSORINFO {
        cbSize: size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetCursorInfo(&mut info) }.is_err() {
        return false;
    }
    [IDC_SIZENS, IDC_SIZEWE, IDC_SIZENESW, IDC_SIZENWSE]
        .into_iter()
        .filter_map(|id| unsafe { LoadCursorW(None, id) }.ok())
        .any(|cursor| cursor == info.hCursor)
}

/// Gives `hwnd` a new outer size where it is, without waiting for the
/// window to answer.
pub fn resize(hwnd: HWND, width: i32, height: i32) {
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            width,
            height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_ASYNCWINDOWPOS,
        )
    };
}

/// Puts `hwnd` exactly at `rect`, without waiting for the window to answer.
pub fn put(hwnd: HWND, rect: Rect) {
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.width(),
            rect.height(),
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_ASYNCWINDOWPOS,
        )
    };
}

/// Puts `hwnd` where `placement` says on the monitor it is on.
pub fn place(hwnd: HWND, placement: &Placement) -> Result<(), MoveError> {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return Ok(());
    }
    let target = placement.resolve(to_rect(info.rcWork));

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
