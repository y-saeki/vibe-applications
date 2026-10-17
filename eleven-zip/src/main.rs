// A release build has no console window: Explorer starts eleven-zip for every
// archive it extracts, and the settings window is a window of its own.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Off Windows only the tests use these modules.
#![cfg_attr(not(windows), allow(dead_code))]

mod archive;
mod art;
mod cli;
mod config;
// Used by build.rs; the crate only tests it.
#[cfg(test)]
mod icon_res;
mod release;
mod shell;
mod store;
#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    win::main();
}

// The platform-independent modules are built and tested anywhere; the
// application itself only exists on Windows.
#[cfg(not(windows))]
fn main() {
    eprintln!("eleven-zip runs on Windows only.");
    std::process::exit(1);
}
