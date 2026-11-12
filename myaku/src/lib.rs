//! myaku's platform-independent part: the per-process history, what the
//! list shows, the parsing of what each operating system reports, and the
//! icon. The Windows application (`main.rs`, `win/`) and the macOS app
//! (`macos/`, through `ffi.rs`) are built on it.

pub mod art;
pub mod format;
pub mod mac_sample;
pub mod monitor;
pub mod place;
pub mod resource;
pub mod runner;
pub mod sample;
pub mod stats;
pub mod win_sample;

#[cfg(feature = "ffi")]
pub mod ffi;

#[cfg(feature = "ffi")]
uniffi::setup_scaffolding!();
