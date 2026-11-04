//! Follows the right button everywhere through a low-level mouse hook. A
//! press over an application a gesture is for is kept from it: the hook
//! follows the pointer until the button comes up, then posts
//! `WM_APP_GESTURE` for the main window to act on, outside the hook. Windows
//! drops a hook that answers too slowly, so nothing here does more than
//! follow.
//!
//! A press that turns out to be a click is given back to the application by
//! `send::click_right`, whose events carry [`MARK`] so that the hook lets
//! them through.

use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromPoint};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HC_ACTION, HHOOK, MSLLHOOKSTRUCT, PostMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_MOUSE_LL, WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_RBUTTONUP,
};

use meta_gesture::gesture::{Gesture, Tracker};

use super::{WM_APP_GESTURE, process};

/// What meta-gesture puts in `dwExtraInfo` of the input it sends ("MGST").
pub const MARK: usize = 0x4D47_5354;

/// Decides, as the button goes down over `hwnd` (a top-level window), the
/// application it belongs to: `Some(id)` when a gesture is for it.
pub type Follows = Box<dyn Fn(HWND) -> Option<String>>;

/// A press of the right button, from going down to coming up.
struct Press {
    window: HWND,
    app: String,
    tracker: Tracker,
}

/// What a press came to, waiting for the main window.
pub enum Finished {
    /// The pointer drew `gesture` (or nothing a gesture can be: `None`)
    /// over `window` of the application `app`.
    Gesture {
        window: HWND,
        app: String,
        gesture: Option<Gesture>,
    },
    /// The pointer hardly moved: the press was a click.
    Click,
}

thread_local! {
    static HOOK: Cell<Option<HHOOK>> = const { Cell::new(None) };
    static OWNER: Cell<Option<HWND>> = const { Cell::new(None) };
    static FOLLOWS: RefCell<Option<Follows>> = const { RefCell::new(None) };
    static PRESS: RefCell<Option<Press>> = const { RefCell::new(None) };
    static FINISHED: RefCell<Option<Finished>> = const { RefCell::new(None) };
}

/// Starts following the right button. `follows` decides each press;
/// `owner` gets `WM_APP_GESTURE` when one ends.
pub fn start(owner: HWND, follows: Follows) -> windows::core::Result<()> {
    stop();
    OWNER.set(Some(owner));
    FOLLOWS.with(|f| *f.borrow_mut() = Some(follows));
    let instance = unsafe { GetModuleHandleW(None) }?;
    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(hook), Some(instance.into()), 0) }?;
    HOOK.set(Some(hook));
    Ok(())
}

pub fn stop() {
    if let Some(hook) = HOOK.take() {
        let _ = unsafe { UnhookWindowsHookEx(hook) };
    }
    FOLLOWS.with(|f| f.borrow_mut().take());
    PRESS.with(|p| p.borrow_mut().take());
}

/// What the last press came to, once.
pub fn take_finished() -> Option<Finished> {
    FINISHED.with(|f| f.borrow_mut().take())
}

/// How many physical pixels a point (DIP) is on the monitor at `pt`, which
/// is what the hook's coordinates are in.
fn scale_at(pt: POINT) -> f64 {
    let monitor = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
    let (mut x, mut y) = (96, 96);
    if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) }.is_err() {
        return 1.0;
    }
    f64::from(x) / 96.0
}

fn point(pt: POINT) -> (f64, f64) {
    (f64::from(pt.x), f64::from(pt.y))
}

/// True keeps the event from the application.
fn on_event(message: u32, info: &MSLLHOOKSTRUCT) -> bool {
    if info.dwExtraInfo == MARK {
        return false;
    }
    match message {
        WM_RBUTTONDOWN => {
            let Some(window) = process::root_at(info.pt) else {
                return false;
            };
            let app = FOLLOWS.with(|f| {
                let f = f.try_borrow().ok()?;
                f.as_ref()?(window)
            });
            let Some(app) = app else {
                return false;
            };
            let press = Press {
                window,
                app,
                tracker: Tracker::new(point(info.pt), scale_at(info.pt)),
            };
            PRESS.with(|p| p.try_borrow_mut().map(|mut p| *p = Some(press)).is_ok())
        }
        WM_MOUSEMOVE => {
            PRESS.with(|p| {
                if let Ok(mut p) = p.try_borrow_mut()
                    && let Some(press) = p.as_mut()
                {
                    press.tracker.move_to(point(info.pt));
                }
            });
            // The pointer moves as it always does.
            false
        }
        WM_RBUTTONUP => {
            let Some(press) = PRESS.with(|p| p.try_borrow_mut().ok()?.take()) else {
                return false;
            };
            let finished = if press.tracker.is_gesture() {
                Finished::Gesture {
                    window: press.window,
                    app: press.app,
                    gesture: press.tracker.gesture(),
                }
            } else {
                Finished::Click
            };
            FINISHED.with(|f| {
                if let Ok(mut f) = f.try_borrow_mut() {
                    *f = Some(finished);
                }
            });
            if let Some(owner) = OWNER.get() {
                let _ = unsafe { PostMessageW(Some(owner), WM_APP_GESTURE, WPARAM(0), LPARAM(0)) };
            }
            true
        }
        _ => false,
    }
}

unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION, lParam points at the event's details.
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        if on_event(wparam.0 as u32, info) {
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
