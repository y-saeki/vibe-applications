//! Tells the page when fingers rest on the trackpad and when they leave it.
//!
//! A page only sees wheel events, which carry no sign of whether a finger is
//! moving them or the scroll is coasting on after the fingers have lifted, and
//! in WKWebView it sees none at all over a message's frame. AppKit knows both:
//! a scroll event's `phase` follows the fingers (`mayBegin` when they touch,
//! `began`/`changed` as they move, `ended` or `cancelled` when they lift), and
//! the coasting that follows has a `momentumPhase` and no `phase`. A local
//! event monitor sees every scroll event the app dispatches, whatever view is
//! under the cursor, so the page hears of each touch and release here.
//!
//! Pull to reload uses this to start a pull only while fingers are down and to
//! reload only when they lift.

/// The event the page listens for; its payload is `"down"` or `"up"`.
#[cfg(target_os = "macos")]
const EVENT: &str = "trackpad";

/// Starts reporting touches to every webview. Called once, at startup.
#[cfg(target_os = "macos")]
pub fn watch(app: &tauri::AppHandle) {
    let handle = app.clone();
    // AppKit's event monitors belong to the main thread.
    let _ = app.run_on_main_thread(move || install(handle));
}

#[cfg(not(target_os = "macos"))]
pub fn watch(_app: &tauri::AppHandle) {}

#[cfg(target_os = "macos")]
fn install(app: tauri::AppHandle) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2_app_kit::{NSEvent, NSEventMask};
    use tauri::Emitter;

    let handler = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit passes the event it is about to dispatch, alive for
        // the length of this call.
        let phase = unsafe { event.as_ref() }.phase();
        if let Some(touch) = touch_of(phase.0) {
            let _ = app.emit(EVENT, touch);
        }
        // Handed back untouched, so the event goes on to the webview as before.
        event.as_ptr()
    });
    // SAFETY: the handler returns the event it was given, which is valid.
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::ScrollWheel, &handler)
    };
    // The monitor lasts as long as the app; it is never removed.
    std::mem::forget(monitor);
}

/// What a scroll event's `phase` says about the fingers, from its raw bits.
/// Momentum events and a mouse wheel's have no phase and say nothing.
#[cfg(any(target_os = "macos", test))]
fn touch_of(phase: usize) -> Option<&'static str> {
    // NSEventPhase: began 1, stationary 2, changed 4, ended 8, cancelled 16,
    // mayBegin 32.
    const BEGAN: usize = 1;
    const ENDED: usize = 8;
    const CANCELLED: usize = 16;
    const MAY_BEGIN: usize = 32;
    if phase & (MAY_BEGIN | BEGAN) != 0 {
        Some("down")
    } else if phase & (ENDED | CANCELLED) != 0 {
        Some("up")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::touch_of;

    #[test]
    fn fingers_touching_and_moving_are_down() {
        assert_eq!(touch_of(32), Some("down"));
        assert_eq!(touch_of(1), Some("down"));
    }

    #[test]
    fn fingers_lifting_are_up() {
        assert_eq!(touch_of(8), Some("up"));
        assert_eq!(touch_of(16), Some("up"));
    }

    #[test]
    fn moving_momentum_and_wheels_say_nothing() {
        // changed, stationary, and none (momentum, a mouse wheel).
        assert_eq!(touch_of(4), None);
        assert_eq!(touch_of(2), None);
        assert_eq!(touch_of(0), None);
    }
}
