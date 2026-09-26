//! Opening the settings window from the resident part.
//!
//! The window (settings_ui.rs) runs in a process of its own, `winwin.exe
//! --settings`: WinUI ends its message loop when its last window closes and
//! cannot start again in the same process, while the resident part lives
//! on. The resident part only starts that process, brings its window
//! forward when asked to open it again, and hears when it ends.

use std::ffi::c_void;

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::{
    CREATE_NO_WINDOW, CreateProcessW, INFINITE, PROCESS_INFORMATION, RegisterWaitForSingleObject,
    STARTUPINFOW, TerminateProcess, UnregisterWait, WT_EXECUTEONLYONCE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, PostMessageW, SW_RESTORE, SetForegroundWindow, ShowWindow,
};
use windows::core::{BOOL, PWSTR};

use super::WM_APP_SETTINGS_CLOSED;

/// The argument that makes winwin.exe the settings window.
pub const ARG: &str = "--settings";

/// A running settings window.
pub struct Child {
    process: HANDLE,
    pid: u32,
    wait: HANDLE,
}

/// Starts the settings window. `owner` gets WM_APP_SETTINGS_CLOSED when it
/// ends, saved or not.
pub fn open(owner: HWND) -> Result<Child, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command: Vec<u16> = format!("\"{}\" {ARG}", exe.display())
        .encode_utf16()
        .chain([0])
        .collect();
    let startup = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();
    // CREATE_NO_WINDOW keeps a debug build, which is a console program, from
    // opening a console for it.
    unsafe {
        CreateProcessW(
            None,
            Some(PWSTR(command.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_NO_WINDOW,
            None,
            None,
            &startup,
            &mut info,
        )
    }
    .map_err(|e| e.to_string())?;
    let _ = unsafe { CloseHandle(info.hThread) };
    // The click on the tray icon made us the foreground process; pass that on
    // so the new window comes up in front.
    let _ = unsafe { AllowSetForegroundWindow(info.dwProcessId) };

    let mut wait = HANDLE::default();
    let registered = unsafe {
        RegisterWaitForSingleObject(
            &mut wait,
            info.hProcess,
            Some(on_exit),
            Some(owner.0 as *const c_void),
            INFINITE,
            WT_EXECUTEONLYONCE,
        )
    };
    if let Err(e) = registered {
        unsafe {
            let _ = TerminateProcess(info.hProcess, 1);
            let _ = CloseHandle(info.hProcess);
        }
        return Err(e.to_string());
    }
    Ok(Child {
        process: info.hProcess,
        pid: info.dwProcessId,
        wait,
    })
}

/// Runs on a thread-pool thread when the process ends.
unsafe extern "system" fn on_exit(owner: *mut c_void, _timed_out: bool) {
    let _ = unsafe {
        PostMessageW(
            Some(HWND(owner)),
            WM_APP_SETTINGS_CLOSED,
            WPARAM(0),
            LPARAM(0),
        )
    };
}

impl Child {
    /// Brings the window forward, restoring it if it was minimized.
    pub fn bring_forward(&self) {
        unsafe extern "system" fn find(hwnd: HWND, found: LPARAM) -> BOOL {
            // SAFETY: `found` points at the (pid, window) pair below.
            let found = unsafe { &mut *(found.0 as *mut (u32, HWND)) };
            let mut pid = 0;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            let top_level = unsafe { GetWindow(hwnd, GW_OWNER) }.is_err();
            if pid == found.0 && top_level && unsafe { IsWindowVisible(hwnd) }.as_bool() {
                found.1 = hwnd;
                return false.into();
            }
            true.into()
        }
        let mut found = (self.pid, HWND::default());
        let _ = unsafe { EnumWindows(Some(find), LPARAM(&mut found as *mut _ as isize)) };
        let hwnd = found.1;
        if hwnd.is_invalid() {
            return;
        }
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
    }

    /// Lets go of a process that has ended.
    pub fn release(self) {
        unsafe {
            let _ = UnregisterWait(self.wait);
            let _ = CloseHandle(self.process);
        }
    }

    /// Ends the window without saving, as quitting winwin (or the installer
    /// closing it) always has.
    pub fn terminate(self) {
        let _ = unsafe { TerminateProcess(self.process, 0) };
        self.release();
    }
}
