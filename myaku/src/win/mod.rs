//! Everything that talks to Win32: the notification-area icon and its menu
//! (`app.rs`), the list that opens from it (`flyout.rs`), and reading the
//! processes (`sampler.rs`), and the choices kept between runs
//! (`settings.rs`).

mod app;
mod autostart;
mod flyout;
mod icon;
mod sampler;
mod settings;
mod theme;

use std::sync::OnceLock;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW, WM_APP};
use windows::core::HSTRING;

use myaku::runner::Runner;

pub const APP_NAME: &str = "myaku";

/// Messages the main window takes from the rest of the application.
pub const WM_APP_TRAY: u32 = WM_APP + 1;
/// Posted by the sampling thread after each reading.
pub const WM_APP_UPDATED: u32 = WM_APP + 2;
/// Sent by a second copy of myaku before it exits, so that starting myaku
/// again while it is running opens the list instead of doing nothing.
pub const WM_APP_SHOW: u32 = WM_APP + 3;

/// The history, read once a second on its own thread from start-up.
static RUNNER: OnceLock<Runner> = OnceLock::new();

pub fn runner() -> &'static Runner {
    RUNNER.get().expect("the runner starts before any window")
}

pub fn main() {
    app::run();
}

pub fn error_box(owner: Option<HWND>, text: &str) {
    unsafe {
        MessageBoxW(
            owner,
            &HSTRING::from(text),
            &HSTRING::from(APP_NAME),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// Copies `text` into a fixed-size UTF-16 field of a Win32 struct, cutting it
/// short when it does not fit and always leaving the terminating NUL.
pub fn copy_to_field(field: &mut [u16], text: &str) {
    let max = field.len() - 1;
    let mut n = 0;
    for unit in text.encode_utf16().take(max) {
        field[n] = unit;
        n += 1;
    }
    field[n] = 0;
}

pub fn loword(v: usize) -> u32 {
    (v & 0xFFFF) as u32
}
