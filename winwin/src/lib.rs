//! The part of winwin that decides what it does, apart from any operating
//! system: the config file, what a placement resolves to, the shortcuts and
//! how their presses are followed, what a dragged window gets back, and what
//! the settings window checks. The Windows application (`main.rs` and
//! `src/win/`) is built on it.
//!
//! Nothing here touches the operating system, so it builds and is tested on
//! every platform.

pub mod art;
pub mod config;
pub mod cycle;
pub mod draft;
pub mod hotkey;
// Used by build.rs; the crate only tests it.
#[cfg(test)]
mod icon_res;
pub mod layout;
pub mod restore;
pub mod vk;
