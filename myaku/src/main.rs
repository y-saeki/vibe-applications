// A release build is a notification-area application: no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    win::main();
}

// The platform-independent part is the library, built and tested anywhere;
// this executable is the Windows application. macOS has its own app in
// macos/, built on the same library.
#[cfg(not(windows))]
fn main() {
    eprintln!("This executable is the Windows application; see macos/ for the Mac.");
    std::process::exit(1);
}
