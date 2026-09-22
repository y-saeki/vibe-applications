//! Where macOS puts the window's traffic lights (macOS only).
//!
//! The page lays its own title bar under the transparent one AppKit draws
//! (`titleBarStyle: "Overlay"`), and the buttons on it have to sit on the
//! same line as the traffic lights. How far down that line is depends on the
//! macOS release, so it is read off the close button rather than assumed.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSWindow, NSWindowButton};
use tauri::WebviewWindow;

/// How far the middle of the traffic lights is from the top of `window`, in
/// points, which is what the page counts in. `None` when it cannot be told.
///
/// AppKit may only be asked on the main thread, which is where a command
/// without `async` runs.
pub fn center(window: &WebviewWindow) -> Option<f64> {
    MainThreadMarker::new()?;
    let pointer = window.ns_window().ok()?.cast::<NSWindow>();
    // SAFETY: Tauri hands out the window's own NSWindow, which lives as long
    // as `window` does, and this is the main thread.
    let ns_window = unsafe { pointer.as_ref() }?;
    let close = ns_window.standardWindowButton(NSWindowButton::CloseButton)?;
    // In the window's own coordinates, which start at its bottom edge.
    let frame = close.convertRect_toView(close.bounds(), None);
    if frame.size.height <= 0.0 {
        return None;
    }
    let from_top = ns_window.frame().size.height - (frame.origin.y + frame.size.height / 2.0);
    (from_top.is_finite() && from_top > 0.0).then_some(from_top)
}
