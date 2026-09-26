//! Starting winwin at sign-in, through the current user's `Run` key.

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::{HSTRING, w};

use super::APP_NAME;

const RUN_KEY: windows::core::PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");

/// What the `Run` value holds: this executable's path, quoted.
fn command() -> String {
    let mut buf = vec![0u16; 1024];
    loop {
        let len = unsafe { GetModuleFileNameW(None, &mut buf) } as usize;
        if len < buf.len() {
            return format!("\"{}\"", String::from_utf16_lossy(&buf[..len]));
        }
        buf.resize(buf.len() * 2, 0);
    }
}

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

fn open_run_key() -> Result<Key, WIN32_ERROR> {
    let mut key = HKEY::default();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
    };
    if status == ERROR_SUCCESS {
        Ok(Key(key))
    } else {
        Err(status)
    }
}

/// Whether sign-in starts this copy of winwin. An entry left behind by a copy
/// that has since moved does not count: it would start nothing.
pub fn is_enabled() -> bool {
    let Ok(key) = open_run_key() else {
        return false;
    };
    let name = HSTRING::from(APP_NAME);
    let mut size = 0u32;
    let status = unsafe { RegQueryValueExW(key.0, &name, None, None, None, Some(&mut size)) };
    if status != ERROR_SUCCESS || size == 0 {
        return false;
    }
    let mut data = vec![0u16; (size as usize).div_ceil(2)];
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            &name,
            None,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return false;
    }
    let value = String::from_utf16_lossy(&data);
    value
        .trim_end_matches('\0')
        .eq_ignore_ascii_case(&command())
}

pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let key = open_run_key().map_err(|e| format!("レジストリを開けません ({})", e.0))?;
    let name = HSTRING::from(APP_NAME);
    let status = if enabled {
        let value: Vec<u16> = command().encode_utf16().chain([0]).collect();
        let bytes: Vec<u8> = value.iter().flat_map(|u| u.to_le_bytes()).collect();
        unsafe { RegSetValueExW(key.0, &name, None, REG_SZ, Some(&bytes)) }
    } else {
        match unsafe { RegDeleteValueW(key.0, &name) } {
            ERROR_FILE_NOT_FOUND => ERROR_SUCCESS,
            other => other,
        }
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("自動起動の設定を変更できません ({})", status.0))
    }
}
