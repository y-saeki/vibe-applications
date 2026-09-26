//! Everything that talks to Win32.

mod app;
mod autostart;
mod icon;
mod mover;
mod settings;
mod settings_ui;

use std::path::PathBuf;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MESSAGEBOX_STYLE, MessageBoxW};
use windows::core::HSTRING;

pub const APP_NAME: &str = "winwin";

/// The resident part, or with `--settings` the settings window.
pub fn main() {
    if std::env::args().nth(1).as_deref() == Some(settings::ARG) {
        settings_ui::run();
    } else {
        app::run();
    }
}

/// Messages the main window takes from the rest of the application.
pub const WM_APP_TRAY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;
pub const WM_APP_SETTINGS_CLOSED: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 2;
/// Sent by a second copy of winwin before it exits, so that starting winwin
/// again while it is running opens the settings instead of doing nothing.
pub const WM_APP_OPEN_SETTINGS: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 3;

/// `%APPDATA%\winwin\config.toml`.
pub fn config_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(APP_NAME).join("config.toml")
}

pub fn message_box(owner: Option<HWND>, text: &str, style: MESSAGEBOX_STYLE) -> i32 {
    unsafe { MessageBoxW(owner, &HSTRING::from(text), &HSTRING::from(APP_NAME), style).0 }
}

pub fn error_box(owner: Option<HWND>, text: &str) {
    message_box(owner, text, MB_OK | MB_ICONERROR);
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
