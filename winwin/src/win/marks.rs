//! What winwin leaves on the windows it has placed: the size to give back
//! and the place a reset goes back to (`restore::Remembered`), as three
//! window properties. They go away with the window, and outlive winwin, so
//! that a winwin started again can still give the size back.
//!
//! Windows asks whoever adds a property to remove it before the window is
//! destroyed. winwin cannot know when another application's window goes; the
//! system frees the properties of a destroyed window, and these are plain
//! numbers that need no cleaning up of their own.

use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::UI::WindowsAndMessaging::{GetPropW, RemovePropW, SetPropW};
use windows::core::{PCWSTR, w};

use crate::restore::Remembered;

const SIZE: PCWSTR = w!("winwin.size");
const PLACED: PCWSTR = w!("winwin.placed");
const ORIGIN: PCWSTR = w!("winwin.origin");

pub fn write(hwnd: HWND, kept: &Remembered) {
    let Some([size, placed, origin]) = kept.pack() else {
        clear(hwnd);
        return;
    };
    unsafe {
        let _ = SetPropW(hwnd, SIZE, Some(HANDLE(size as usize as *mut _)));
        let _ = SetPropW(hwnd, PLACED, Some(HANDLE(placed as usize as *mut _)));
        let _ = SetPropW(hwnd, ORIGIN, Some(HANDLE(origin as usize as *mut _)));
    }
}

pub fn read(hwnd: HWND) -> Option<Remembered> {
    let value = |name| unsafe { GetPropW(hwnd, name) }.0 as usize as u64;
    Remembered::unpack([value(SIZE), value(PLACED), value(ORIGIN)])
}

pub fn clear(hwnd: HWND) {
    unsafe {
        let _ = RemovePropW(hwnd, SIZE);
        let _ = RemovePropW(hwnd, PLACED);
        let _ = RemovePropW(hwnd, ORIGIN);
    }
}
