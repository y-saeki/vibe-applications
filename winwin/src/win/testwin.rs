//! The test window: a plain pane of frosted glass the settings window opens
//! on request, which the shortcuts being edited move about before they are
//! saved. It lives in the settings process, on the WinUI thread, and holds
//! the hotkeys for as long as it is open (the resident part has released
//! them while the settings window is open).
//!
//! It has no title bar or buttons. It can be dragged anywhere on it and
//! resized from its edges, and it takes any size a placement asks for.

use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWMWINDOWATTRIBUTE, DwmExtendFrameIntoClientArea,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, CreateSolidBrush, DeleteObject, FillRect, GetMonitorInfoW, GetStockObject, HBRUSH,
    HDC, InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWL_EXSTYLE, GetClientRect, GetCursorPos,
    GetWindowLongPtrW, GetWindowRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTLEFT,
    HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, HWND_TOP, KillTimer, LWA_ALPHA, MINMAXINFO,
    RegisterClassExW, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, WM_CLOSE,
    WM_ERASEBKGND, WM_GETMINMAXINFO, WM_HOTKEY, WM_NCCALCSIZE, WM_NCHITTEST, WM_TIMER, WNDCLASSEXW,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_POPUP, WS_THICKFRAME,
};
use windows::core::{HSTRING, w};

use super::{modifiers_held, mover};
use crate::config::{Config, Theme};
use crate::cycle::{self, Binding, Cycle};
use crate::layout::{Anchor, Placement, Ratio, Rect};

const CLASS_NAME: windows::core::PCWSTR = w!("winwin.test");
const TITLE: &str = "winwin テスト用ウィンドウ";
/// As in the resident part (app.rs): watches for a cycling shortcut's
/// modifiers to be let go.
const CYCLE_TIMER: usize = 1;
const CYCLE_POLL_MS: u32 = 15;
/// How far in from its edges the window can be resized, in DIPs.
const RESIZE_BORDER: i32 = 8;
/// Where the window first appears on the monitor the pointer is on.
const FIRST_PLACEMENT: Placement = Placement {
    anchor: Anchor::Center,
    width: Ratio::HALF,
    height: Ratio::HALF,
};
/// Where Windows has no glass to offer (Windows 10), the window is a plain
/// color this see-through instead (of 255).
const PLAIN_ALPHA: u8 = 216;

struct TestWindow {
    hwnd: HWND,
    /// The shortcuts it answers to; hotkey id `n` is `bindings[n - 1]`.
    bindings: Vec<Binding>,
    cycle: Cycle,
    registered: i32,
    /// The combinations another application holds, from the last time the
    /// shortcuts changed.
    failed: Vec<String>,
}

thread_local! {
    static WINDOW: RefCell<Option<TestWindow>> = const { RefCell::new(None) };
    /// Called when the window is asked to close (Alt+F4); the settings
    /// window decides.
    static ON_CLOSE: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
    /// How the window paints itself: see-through to the glass behind it, or
    /// a plain color, dark or light.
    static GLASS: Cell<bool> = const { Cell::new(false) };
    static DARK: Cell<bool> = const { Cell::new(false) };
}

/// As `with_app` in app.rs: never call anything that sends a message to the
/// window (SetWindowPos, DestroyWindow) inside `f`.
fn with_window<R>(f: impl FnOnce(&mut TestWindow) -> R) -> Option<R> {
    WINDOW.with(|w| w.try_borrow_mut().ok()?.as_mut().map(f))
}

pub fn is_open() -> bool {
    WINDOW.with(|w| w.borrow().is_some())
}

/// Opens the window in the middle of the monitor the pointer is on, without
/// taking the focus from the settings window. `on_close` hears when it is
/// asked to close; [`close`] closes it.
pub fn open(theme: Theme, on_close: impl Fn() + 'static) -> Result<(), String> {
    if is_open() {
        return Ok(());
    }
    let hwnd = create().map_err(|e| e.to_string())?;
    GLASS.set(false);
    ON_CLOSE.with(|c| *c.borrow_mut() = Some(Box::new(on_close)));
    WINDOW.with(|w| {
        *w.borrow_mut() = Some(TestWindow {
            hwnd,
            bindings: Vec::new(),
            cycle: Cycle::default(),
            registered: 0,
            failed: Vec::new(),
        })
    });
    dress(hwnd, theme);
    if let Some(work) = pointer_work_area() {
        let r = FIRST_PLACEMENT.resolve(work);
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                None,
                r.left,
                r.top,
                r.width(),
                r.height(),
                SWP_NOACTIVATE,
            )
        };
    }
    let _ = unsafe { ShowWindow(hwnd, SW_SHOWNA) };
    Ok(())
}

/// Closes the window and lets go of its shortcuts.
pub fn close() {
    let Some(window) = WINDOW.with(|w| w.borrow_mut().take()) else {
        return;
    };
    for id in 1..=window.registered {
        let _ = unsafe { UnregisterHotKey(Some(window.hwnd), id) };
    }
    ON_CLOSE.with(|c| c.borrow_mut().take());
    let _ = unsafe { DestroyWindow(window.hwnd) };
}

/// Makes the window answer to `config`'s shortcuts, and returns the ones
/// another application or Windows already holds. Called after every change
/// in the settings window; the hotkeys are registered again only when the
/// shortcuts actually changed, so that a cycle in progress carries on.
pub fn set_shortcuts(config: &Config) -> Vec<String> {
    let bindings = cycle::bindings(config);
    let stale = with_window(|w| w.bindings != bindings).unwrap_or(false);
    if stale {
        stop_cycle();
        with_window(|w| {
            for id in 1..=w.registered {
                let _ = unsafe { UnregisterHotKey(Some(w.hwnd), id) };
            }
            w.failed.clear();
            for (i, b) in bindings.iter().enumerate() {
                let modifiers = HOT_KEY_MODIFIERS(b.keys.modifiers) | MOD_NOREPEAT;
                let id = i as i32 + 1;
                if unsafe { RegisterHotKey(Some(w.hwnd), id, modifiers, b.keys.vk) }.is_err() {
                    w.failed.push(b.keys.to_string());
                }
            }
            w.registered = bindings.len() as i32;
            w.bindings = bindings;
        });
    }
    with_window(|w| w.failed.clone()).unwrap_or_default()
}

/// Follows the settings window's theme.
pub fn set_theme(theme: Theme) {
    if let Some(hwnd) = with_window(|w| w.hwnd) {
        dress(hwnd, theme);
    }
}

fn create() -> windows::core::Result<HWND> {
    let instance = unsafe { GetModuleHandleW(None) }?;
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wndproc),
        hInstance: instance.into(),
        lpszClassName: CLASS_NAME,
        ..Default::default()
    };
    // Fails harmlessly when the window is opened a second time.
    unsafe { RegisterClassExW(&class) };
    // A tool window stays off the taskbar and Alt+Tab, and the resident
    // part does not take it for the settings window when bringing that
    // forward (settings.rs). WS_THICKFRAME gives it the frame the glass,
    // the shadow and the rounded corners are drawn on; WM_NCCALCSIZE then
    // hands all of it to the client area.
    unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            CLASS_NAME,
            &HSTRING::from(TITLE),
            WS_POPUP | WS_THICKFRAME,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }
}

fn set_attribute<T>(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, value: &T) -> bool {
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attribute,
            (value as *const T).cast(),
            size_of::<T>() as u32,
        )
    }
    .is_ok()
}

/// Frosted glass over the whole window where Windows offers it (Windows 11
/// 22H2 and later), otherwise a see-through plain color; dark or light as
/// `theme` says.
fn dress(hwnd: HWND, theme: Theme) {
    let dark = match theme {
        Theme::System => system_is_dark(),
        Theme::Light => false,
        Theme::Dark => true,
    };
    DARK.set(dark);
    set_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &i32::from(dark));
    set_attribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &DWMWCP_ROUND);
    if !GLASS.get() && set_attribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &DWMSBT_TRANSIENTWINDOW) {
        // The glass shows through wherever the client area is painted
        // black.
        let whole = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        GLASS.set(unsafe { DwmExtendFrameIntoClientArea(hwnd, &whole) }.is_ok());
    }
    if !GLASS.get() {
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_LAYERED.0 as isize);
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), PLAIN_ALPHA, LWA_ALPHA);
        }
    }
    let _ = unsafe { InvalidateRect(Some(hwnd), None, true) };
}

/// Whether Windows is set to dark for applications.
fn system_is_dark() -> bool {
    let mut light = 1u32;
    let mut size = size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut light as *mut u32).cast()),
            Some(&mut size),
        )
    };
    status.is_ok() && light == 0
}

fn pointer_work_area() -> Option<Rect> {
    let mut cursor = POINT::default();
    let _ = unsafe { GetCursorPos(&mut cursor) };
    let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetMonitorInfoW(monitor, &mut info) }
        .as_bool()
        .then(|| {
            let w = info.rcWork;
            Rect {
                left: w.left,
                top: w.top,
                right: w.right,
                bottom: w.bottom,
            }
        })
}

/// As the resident part's shortcuts do, except that it is always this
/// window that moves, whichever window has the focus. It comes to the front
/// so that the result can be seen, without taking the focus.
fn on_hotkey(id: usize) {
    let picked = with_window(|w| {
        let index = id.wrapping_sub(1);
        let b = w.bindings.get(index)?;
        let entry = w.cycle.press(index, b.placements.len());
        Some((w.hwnd, b.placements[entry], b.cycles()))
    })
    .flatten();
    let Some((hwnd, placement, cycles)) = picked else {
        return;
    };
    if cycles {
        unsafe { SetTimer(Some(hwnd), CYCLE_TIMER, CYCLE_POLL_MS, None) };
    }
    let _ = mover::place(hwnd, &placement);
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    };
}

fn on_cycle_timer() {
    let modifiers = with_window(|w| {
        let b = w.cycle.active()?;
        w.bindings.get(b).map(|b| b.keys.modifiers)
    })
    .flatten();
    if modifiers.is_none_or(|m| !modifiers_held(m)) {
        stop_cycle();
    }
}

fn stop_cycle() {
    if let Some(hwnd) = with_window(|w| {
        w.cycle.reset();
        w.hwnd
    }) {
        let _ = unsafe { KillTimer(Some(hwnd), CYCLE_TIMER) };
    }
}

/// Resize handles along the edges, and a handle to drag it by everywhere
/// else.
fn hit_test(hwnd: HWND, lparam: LPARAM) -> u32 {
    let x = i32::from((lparam.0 & 0xFFFF) as u16 as i16);
    let y = i32::from(((lparam.0 >> 16) & 0xFFFF) as u16 as i16);
    let mut r = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut r) }.is_err() {
        return HTCAPTION;
    }
    let border = RESIZE_BORDER * unsafe { GetDpiForWindow(hwnd) } as i32 / 96;
    let left = x < r.left + border;
    let right = x >= r.right - border;
    let top = y < r.top + border;
    let bottom = y >= r.bottom - border;
    match (left, right, top, bottom) {
        (true, _, true, _) => HTTOPLEFT,
        (_, true, true, _) => HTTOPRIGHT,
        (true, _, _, true) => HTBOTTOMLEFT,
        (_, true, _, true) => HTBOTTOMRIGHT,
        (true, ..) => HTLEFT,
        (_, true, ..) => HTRIGHT,
        (_, _, true, _) => HTTOP,
        (.., true) => HTBOTTOM,
        _ => HTCAPTION,
    }
}

fn paint_background(hwnd: HWND, hdc: HDC) {
    let mut r = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut r) };
    if GLASS.get() {
        let _ = unsafe { FillRect(hdc, &r, HBRUSH(GetStockObject(BLACK_BRUSH).0)) };
        return;
    }
    // The base background of Windows' own dark and light windows.
    let color = if DARK.get() {
        COLORREF(0x0020_2020)
    } else {
        COLORREF(0x00F3_F3F3)
    };
    unsafe {
        let brush = CreateSolidBrush(color);
        let _ = FillRect(hdc, &r, brush);
        let _ = DeleteObject(brush.into());
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // The whole window is client area: no title bar, no visible border
        // beyond the one Windows draws around the glass.
        WM_NCCALCSIZE if wparam.0 != 0 => LRESULT(0),
        WM_NCHITTEST => LRESULT(hit_test(hwnd, lparam) as isize),
        // Whatever size a placement asks for, down to a pixel.
        WM_GETMINMAXINFO => {
            // SAFETY: lParam points at the MINMAXINFO to fill in.
            let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
            info.ptMinTrackSize = POINT { x: 1, y: 1 };
            LRESULT(0)
        }
        WM_ERASEBKGND => {
            paint_background(hwnd, HDC(wparam.0 as *mut _));
            LRESULT(1)
        }
        WM_HOTKEY => {
            on_hotkey(wparam.0);
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == CYCLE_TIMER => {
            on_cycle_timer();
            LRESULT(0)
        }
        WM_CLOSE => {
            ON_CLOSE.with(|c| {
                if let Ok(c) = c.try_borrow()
                    && let Some(f) = c.as_ref()
                {
                    f();
                }
            });
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
