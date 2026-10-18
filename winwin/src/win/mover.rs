//! Moving and resizing the foreground window.

use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL, MONITORINFO, MonitorFromRect,
    MonitorFromWindow,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
use windows::Win32::UI::WindowsAndMessaging::{
    CURSORINFO, EnumWindows, GA_ROOT, GWL_EXSTYLE, GetAncestor, GetClassNameW, GetCursorInfo,
    GetForegroundWindow, GetShellWindow, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect,
    HTCAPTION, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, IsHungAppWindow, IsIconic,
    IsWindowVisible, IsZoomed, LoadCursorW, PostMessageW, SC_MOVE, SM_SWAPBUTTON, SW_MAXIMIZE,
    SW_RESTORE, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER, SetWindowPos,
    ShowWindow, WM_CANCELMODE, WM_SYSCOMMAND, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};
use windows::core::{BOOL, s, w};

use crate::layout::{Placement, Rect};
use crate::restore::Reset;

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
    set_window(
        hwnd,
        Rect {
            left: target.left - left,
            top: target.top - top,
            right: target.right + right,
            bottom: target.bottom + bottom,
        },
    )
}

/// Places the window's outer rectangle on `target`.
fn set_window(hwnd: HWND, target: Rect) -> Result<(), MoveError> {
    let result = unsafe {
        SetWindowPos(
            hwnd,
            None,
            target.left,
            target.top,
            target.width(),
            target.height(),
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

/// A window a shortcut has moved: where it was, whether it was maximized,
/// where it went, and the DPI of its monitor.
pub struct Placed {
    pub hwnd: HWND,
    pub before: Rect,
    pub maximized: bool,
    pub dpi: u32,
    pub after: Rect,
}

/// The window a shortcut acts on: the foreground one, unless it is one to
/// leave alone.
pub fn foreground() -> Option<HWND> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return None;
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
        return None;
    }
    Some(hwnd)
}

/// Places the foreground window, and says where it was and where it went
/// when it moved.
pub fn place_foreground(placement: &Placement) -> Result<Option<Placed>, MoveError> {
    let Some(hwnd) = foreground() else {
        return Ok(None);
    };

    let zoomed = unsafe { IsZoomed(hwnd) }.as_bool();
    if zoomed {
        let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
    }
    // After the restore, so that a maximized window keeps the size it has
    // when it is not.
    let Some(before) = window_rect(hwnd) else {
        return Ok(None);
    };
    let dpi = monitor_dpi(hwnd);
    if !place(hwnd, placement)? {
        // A placement that fits found no room: the window stays as it was.
        if zoomed {
            let _ = unsafe { ShowWindow(hwnd, SW_MAXIMIZE) };
        }
        return Ok(None);
    }
    Ok(window_rect(hwnd).map(|after| Placed {
        hwnd,
        before,
        maximized: zoomed,
        dpi,
        after,
    }))
}

/// Puts `hwnd` back where a reset says, and maximizes it again if it was.
pub fn reset(hwnd: HWND, to: Reset) -> Result<(), MoveError> {
    if unsafe { IsZoomed(hwnd) }.as_bool() {
        let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
    }
    set_window(hwnd, to.rect)?;
    // A window that crossed onto a monitor of another DPI may have scaled
    // itself on the way. One more pass settles it.
    if window_rect(hwnd).is_some_and(|r| r != to.rect) {
        set_window(hwnd, to.rect)?;
    }
    if to.maximized {
        let _ = unsafe { ShowWindow(hwnd, SW_MAXIMIZE) };
    }
    Ok(())
}

/// Whether some monitor shows part of `rect`.
pub fn is_shown(rect: Rect) -> bool {
    let r = RECT {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    };
    !unsafe { MonitorFromRect(&r, MONITOR_DEFAULTTONULL) }.is_invalid()
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

/// Ends the move loop `hwnd` is in, and has the system start another one
/// after putting the window at `rect`: the drag carries on at the new size.
/// The first is a posted WM_CANCELMODE, which a window that is not in a move
/// loop takes as nothing more than that.
pub fn stop_drag(hwnd: HWND) {
    let _ = unsafe { PostMessageW(Some(hwnd), WM_CANCELMODE, WPARAM(0), LPARAM(0)) };
}

/// Puts `hwnd` at `rect` and starts a new move loop on it, as a press on
/// its title bar would (SC_MOVE with HTCAPTION in the low bits, the
/// combination Windows itself uses for a drag from the caption). The window
/// is put synchronously, so that the loop starts from the new rectangle; a
/// window that does not answer is left alone.
pub fn resume_drag(hwnd: HWND, rect: Rect, cursor: (i32, i32)) {
    if unsafe { IsHungAppWindow(hwnd) }.as_bool() {
        return;
    }
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.width(),
            rect.height(),
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        )
    };
    let point = (cursor.0 as u16 as u32) | ((cursor.1 as u16 as u32) << 16);
    let _ = unsafe {
        PostMessageW(
            Some(hwnd),
            WM_SYSCOMMAND,
            WPARAM((SC_MOVE | HTCAPTION) as usize),
            LPARAM(point as isize),
        )
    };
}

/// Whether the primary mouse button is held, whichever physical one it is.
pub fn primary_button_down() -> bool {
    let swapped = unsafe { GetSystemMetrics(SM_SWAPBUTTON) } != 0;
    let button = if swapped { VK_RBUTTON } else { VK_LBUTTON };
    (unsafe { GetAsyncKeyState(i32::from(button.0)) }) < 0
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

/// The work area of the monitor `hwnd` is on.
fn work_area(hwnd: HWND) -> Option<Rect> {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetMonitorInfoW(monitor, &mut info) }
        .as_bool()
        .then(|| to_rect(info.rcWork))
}

/// Whether `hwnd` is a window the user sees on screen and works with: shown,
/// not minimized, not hidden by Windows (as one on another virtual desktop
/// is), and neither a tool window, such as a tooltip or a floating toolbar,
/// nor the desktop or the taskbar.
fn is_on_screen(hwnd: HWND) -> bool {
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() || unsafe { IsIconic(hwnd) }.as_bool() {
        return false;
    }
    let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    if ex_style & WS_EX_TOOLWINDOW.0 != 0 || is_shell_window(hwnd) {
        return false;
    }
    let mut cloaked = 0u32;
    let got = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            size_of::<u32>() as u32,
        )
    };
    !(got.is_ok() && cloaked != 0)
}

/// Whether `hwnd` stays above every window that is not, as a window set to
/// always on top does.
fn is_topmost(hwnd: HWND) -> bool {
    let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    ex_style & WS_EX_TOPMOST.0 != 0
}

/// The visible frames of the windows on screen other than `hwnd` and those
/// always on top, from the top of the z-order down, as `layout::fit` takes
/// them. A maximized window is given as the whole work area of its monitor,
/// which it covers.
fn other_frames(hwnd: HWND) -> Vec<Rect> {
    unsafe extern "system" fn collect(other: HWND, found: LPARAM) -> BOOL {
        let (skip, frames) = unsafe { &mut *(found.0 as *mut (HWND, Vec<Rect>)) };
        if other != *skip && is_on_screen(other) && !is_topmost(other) {
            let frame = if unsafe { IsZoomed(other) }.as_bool() {
                work_area(other)
            } else {
                measure(other).map(|(_, frame)| frame)
            };
            frames.extend(frame);
        }
        true.into()
    }
    let mut found = (hwnd, Vec::new());
    let _ = unsafe {
        EnumWindows(
            Some(collect),
            LPARAM(&mut found as *mut (HWND, Vec<Rect>) as isize),
        )
    };
    found.1
}

/// Puts `hwnd` where `placement` says on the monitor it is on. False when a
/// placement that fits finds no room, and the window is left where it is.
pub fn place(hwnd: HWND, placement: &Placement) -> Result<bool, MoveError> {
    let (Some(work), Some((_, frame))) = (work_area(hwnd), measure(hwnd)) else {
        return Ok(true);
    };
    let Some(target) =
        placement.resolve(work, (frame.width(), frame.height()), || other_frames(hwnd))
    else {
        return Ok(false);
    };

    set_frame(hwnd, target)?;
    // The invisible borders can change with the size (a window that reached
    // its minimum, or one that redraws its frame), in which case the first
    // pass measured the wrong ones. One more pass settles it.
    if let Some((_, frame)) = measure(hwnd)
        && frame != target
    {
        set_frame(hwnd, target)?;
    }
    Ok(true)
}
