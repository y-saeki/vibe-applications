//! Everything that talks to Win32, the network or 7-Zip.

mod extract;
mod icon;
mod net;
mod registry;
mod settings_ui;
mod sevenzip;

use std::path::PathBuf;

use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::HSTRING;

use crate::cli::{self, Command};
use crate::config::Config;
use crate::shell::APP_NAME;
use crate::store::Store;

pub fn main() {
    match cli::parse(std::env::args_os().skip(1)) {
        Command::Settings => settings_ui::run(),
        Command::Extract(archives) => extract::run(&archives),
        Command::Register => {
            // The installer runs this without a window to show an error in,
            // and the settings window registers again whenever it opens.
            let config = Config::load(&config_path()).unwrap_or_default();
            let _ = registry::register(config.context_menu);
        }
        Command::Unregister => {
            let _ = registry::unregister();
        }
    }
}

/// `%APPDATA%\eleven-zip\config.toml`.
pub fn config_path() -> PathBuf {
    known_folder("APPDATA").join(APP_NAME).join("config.toml")
}

/// `%LOCALAPPDATA%\eleven-zip\7-Zip`: kept on this computer, since it is
/// downloaded again wherever eleven-zip runs.
pub fn store() -> Store {
    Store::new(known_folder("LOCALAPPDATA").join(APP_NAME).join("7-Zip"))
}

fn known_folder(var: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn error_box(text: &str) {
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(text),
            &HSTRING::from(APP_NAME),
            MB_OK | MB_ICONERROR,
        )
    };
}

/// Seconds since the Unix epoch, as the store records checks.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
