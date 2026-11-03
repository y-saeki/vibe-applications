//! The part of meta-gesture that decides what it does, apart from any
//! operating system: the config file, which gesture the pointer drew and
//! what it comes to in an application, the keys sent, and what the settings
//! window edits and checks. The Windows application (`main.rs` and
//! `src/win/`) is built on it, and the macOS app (`macos/`) calls it through
//! the bindings in `ffi.rs`.
//!
//! Nothing here touches the operating system, so it builds and is tested on
//! every platform.

pub mod art;
pub mod config;
pub mod editor;
#[cfg(feature = "ffi")]
mod ffi;
pub mod gesture;
// Used by build.rs; the crate only tests it.
#[cfg(test)]
mod icon_res;
pub mod keys;
pub mod mac;
pub mod vk;

#[cfg(feature = "ffi")]
uniffi::setup_scaffolding!();
