//! What a gesture does to the application: bringing its window forward and
//! sending it keys, or giving it back the click the hook kept from it. All
//! the input sent carries `mousehook::MARK`.

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEINPUT,
    SendInput, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, SW_RESTORE,
    SetForegroundWindow, ShowWindow,
};

use meta_gesture::keys::Combo;
use meta_gesture::vk;

use super::mousehook::MARK;

/// Makes `hwnd` the foreground window, unless it already is. Windows lets a
/// process do so only in some cases (SetForegroundWindow's remarks), so
/// when the plain call is refused it is tried again with this thread's
/// input attached to the foreground window's. False when it still is not.
pub fn bring_forward(hwnd: HWND) -> bool {
    let is_front = || unsafe { GetForegroundWindow() } == hwnd;
    if is_front() {
        return true;
    }
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        if SetForegroundWindow(hwnd).as_bool() && is_front() {
            return true;
        }
        let front = GetForegroundWindow();
        let theirs = GetWindowThreadProcessId(front, None);
        let ours = GetCurrentThreadId();
        let attached =
            theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        if attached {
            let _ = AttachThreadInput(ours, theirs, false);
        }
    }
    is_front()
}

fn key(code: u32, up: bool, extended: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(code as u16),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MARK,
            },
        },
    }
}

/// Presses `combo` in the foreground window: the modifiers down, the key
/// down and up, the modifiers up.
pub fn keys(combo: Combo) {
    let modifiers = vk::modifier_codes(combo.modifiers);
    let code = vk::code(combo.key);
    let extended = vk::is_extended(combo.key);
    let mut inputs: Vec<INPUT> = modifiers.iter().map(|&m| key(m, false, false)).collect();
    inputs.push(key(code, false, extended));
    inputs.push(key(code, true, extended));
    inputs.extend(modifiers.iter().rev().map(|&m| key(m, true, false)));
    unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
}

fn mouse(flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MARK,
            },
        },
    }
}

/// A right click where the pointer is, in place of the one the hook kept.
pub fn click_right() {
    let inputs = [mouse(MOUSEEVENTF_RIGHTDOWN), mouse(MOUSEEVENTF_RIGHTUP)];
    unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
}
