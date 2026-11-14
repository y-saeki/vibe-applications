//! macOS: the slide fills the screen with nothing over it.
//!
//! The ordinary (native) fullscreen leaves the menu bar, and with it the title
//! bar, showing whenever the user has macOS keep the menu bar visible in full
//! screen; the app's presentation options cannot override that setting there.
//! So the window uses the simple fullscreen instead, the kind a slideshow
//! uses: it covers the screen where it is, with no title bar, and the menu bar
//! and the Dock are hidden for as long as it lasts.
//!
//! Switching it changes the window's style, and tao then makes its own view
//! the first responder. The web view would stop receiving pointer moves and
//! keys until clicked, so it is made the first responder again afterwards.

use std::sync::OnceLock;

use dispatch2::DispatchQueue;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationPresentationOptions, NSView, NSWindow};
use tauri::WebviewWindow;

/// The main window's WKWebView, as an address (`NSView *`).
static WEB_VIEW: OnceLock<usize> = OnceLock::new();

/// Looks up the web view once, at setup.
pub fn remember_web_view(window: &WebviewWindow) {
    let _ = window.with_webview(|webview| {
        let _ = WEB_VIEW.set(webview.inner() as usize);
    });
}

pub fn set(window: &WebviewWindow, on: bool) -> tauri::Result<()> {
    window.set_simple_fullscreen(on)?;
    let ns_window = window.ns_window()? as usize;
    // Queued behind the switch above, which runs on the main thread when the
    // event loop gets to it.
    window.run_on_main_thread(move || {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        if on {
            // Replaces the auto-hiding options the switch set. A valid
            // combination: hiding the menu bar requires hiding the Dock.
            // Leaving restores what was there before the switch.
            NSApplication::sharedApplication(mtm).setPresentationOptions(
                NSApplicationPresentationOptions::HideDock
                    | NSApplicationPresentationOptions::HideMenuBar,
            );
        }
        // Leaving changes the style on the main dispatch queue, so this goes
        // on that queue too, after it.
        DispatchQueue::main().exec_async(move || {
            let Some(&web_view) = WEB_VIEW.get() else {
                return;
            };
            // SAFETY: both are the main window's, which lives as long as the
            // app, and this runs on the main thread.
            let (ns_window, web_view) = unsafe {
                (
                    &*(ns_window as *const NSWindow),
                    &*(web_view as *const NSView),
                )
            };
            ns_window.makeFirstResponder(Some(web_view));
        });
    })
}
