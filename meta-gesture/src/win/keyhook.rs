//! Catches every key while the settings window records a shortcut, before
//! Windows acts on it: Win alone would open the Start menu, and Alt the
//! window's menu. A low-level keyboard hook is the only way to see those, and
//! it is installed only for as long as the recording dialog is open.

use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetWindowThreadProcessId, HC_ACTION, HHOOK,
    KBDLLHOOKSTRUCT, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_SYSKEYDOWN,
};

/// Where keys go: a virtual-key code, and whether it went down.
type Target = Box<dyn Fn(u32, bool)>;

thread_local! {
    static HOOK: Cell<Option<HHOOK>> = const { Cell::new(None) };
    static TARGET: RefCell<Option<Target>> = const { RefCell::new(None) };
}

/// Sends every key to `target` (virtual-key code, pressed) and keeps it from
/// everything else, while one of this process's windows is in front. Keys
/// typed into another application pass through untouched.
pub fn start(target: impl Fn(u32, bool) + 'static) -> windows::core::Result<()> {
    stop();
    let instance = unsafe { GetModuleHandleW(None) }?;
    let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), Some(instance.into()), 0) }?;
    HOOK.set(Some(hook));
    TARGET.with(|t| *t.borrow_mut() = Some(Box::new(target)));
    Ok(())
}

pub fn stop() {
    if let Some(hook) = HOOK.take() {
        let _ = unsafe { UnhookWindowsHookEx(hook) };
    }
    TARGET.with(|t| t.borrow_mut().take());
}

fn in_front() -> bool {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid)) };
    pid == unsafe { GetCurrentProcessId() }
}

unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && in_front() {
        // SAFETY: for HC_ACTION, lParam points at the key's details.
        let key = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        let taken = TARGET.with(|t| {
            let t = t.try_borrow().ok()?;
            let f = t.as_ref()?;
            f(key.vkCode, down);
            Some(())
        });
        if taken.is_some() {
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
