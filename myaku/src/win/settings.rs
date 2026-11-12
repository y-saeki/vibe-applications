//! The choices myaku keeps between runs, under the current user's
//! `Software\myaku` key.

use std::sync::atomic::{AtomicU8, Ordering};

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_DWORD, REG_OPTION_NON_VOLATILE,
    RegCloseKey, RegCreateKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::w;

use myaku::format::Scale;

const KEY: windows::core::PCWSTR = w!(r"Software\myaku");
const MEMORY_SCALE: windows::core::PCWSTR = w!("MemoryScale");
const KEEP_ON_SCREEN: windows::core::PCWSTR = w!("KeepOnScreen");
const PINNED_X: windows::core::PCWSTR = w!("PinnedX");
const PINNED_Y: windows::core::PCWSTR = w!("PinnedY");

/// The memory scale as read or last set: 0 not read yet, 1 linear, 2 log.
static CACHED: AtomicU8 = AtomicU8::new(0);

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

fn open_key() -> Option<Key> {
    let mut key = HKEY::default();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            KEY,
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
    };
    (status == ERROR_SUCCESS).then_some(Key(key))
}

fn read(name: windows::core::PCWSTR) -> Option<u32> {
    let key = open_key()?;
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            name,
            None,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    (status == ERROR_SUCCESS).then_some(value)
}

fn write(name: windows::core::PCWSTR, value: u32) {
    let Some(key) = open_key() else {
        return;
    };
    let _ = unsafe { RegSetValueExW(key.0, name, None, REG_DWORD, Some(&value.to_le_bytes())) };
}

fn read_memory_scale() -> Scale {
    if read(MEMORY_SCALE) == Some(1) {
        Scale::Log
    } else {
        Scale::Linear
    }
}

/// How the memory graph is drawn.
pub fn memory_scale() -> Scale {
    match CACHED.load(Ordering::Relaxed) {
        1 => Scale::Linear,
        2 => Scale::Log,
        _ => {
            let scale = read_memory_scale();
            store(scale);
            scale
        }
    }
}

fn store(scale: Scale) {
    let v = match scale {
        Scale::Linear => 1,
        Scale::Log => 2,
    };
    CACHED.store(v, Ordering::Relaxed);
}

/// Draws the memory graph by `scale` from now on, and keeps the choice for
/// the next run when the registry can be written.
pub fn set_memory_scale(scale: Scale) {
    store(scale);
    write(
        MEMORY_SCALE,
        match scale {
            Scale::Linear => 0,
            Scale::Log => 1,
        },
    );
}

/// Whether the list stays on screen when the focus moves elsewhere.
pub fn keep_on_screen() -> bool {
    read(KEEP_ON_SCREEN) == Some(1)
}

pub fn set_keep_on_screen(on: bool) {
    write(KEEP_ON_SCREEN, on.into());
}

/// Where the list kept on screen was last left: its top-left corner in
/// screen pixels. None until it has been moved.
pub fn pinned_origin() -> Option<(i32, i32)> {
    Some((read(PINNED_X)? as i32, read(PINNED_Y)? as i32))
}

pub fn set_pinned_origin(x: i32, y: i32) {
    write(PINNED_X, x as u32);
    write(PINNED_Y, y as u32);
}
