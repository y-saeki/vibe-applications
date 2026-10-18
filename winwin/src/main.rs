// A release build is a tray application: no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    win::main();
}

// The platform-independent part is the library, built and tested anywhere;
// the application itself only exists on Windows.
#[cfg(not(windows))]
fn main() {
    eprintln!("winwin runs on Windows only.");
    std::process::exit(1);
}
