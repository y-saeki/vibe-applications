// A release build is a tray application: no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Off Windows only the tests use these modules.
#![cfg_attr(not(windows), allow(dead_code))]

mod config;
mod cycle;
mod draft;
mod hotkey;
mod layout;
#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    win::app::run();
}

// The platform-independent modules are built and tested anywhere; the
// application itself only exists on Windows.
#[cfg(not(windows))]
fn main() {
    eprintln!("winwin runs on Windows only.");
    std::process::exit(1);
}
