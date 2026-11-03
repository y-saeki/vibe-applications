//! Which application a window belongs to. An application is known by its
//! executable's file name in lower case (`chrome.exe`), and shown by the
//! description its executable carries ("Google Chrome"), or by the file
//! name without `.exe` when it carries none.

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GA_ROOT, GW_OWNER, GWL_EXSTYLE, GetAncestor, GetWindow, GetWindowLongW,
    GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible, WS_EX_TOOLWINDOW,
    WindowFromPoint,
};
use windows::core::{BOOL, HSTRING, PWSTR};

use meta_gesture::config::App;

/// The top-level window at `pt`, in physical pixels.
pub fn root_at(pt: POINT) -> Option<HWND> {
    let hwnd = unsafe { WindowFromPoint(pt) };
    if hwnd.is_invalid() {
        return None;
    }
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    (!root.is_invalid()).then_some(root)
}

fn process_id(hwnd: HWND) -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid
}

/// Whether `hwnd` is one of meta-gesture's own windows.
pub fn is_own(hwnd: HWND) -> bool {
    process_id(hwnd) == unsafe { GetCurrentProcessId() }
}

/// The path of the executable whose process owns `hwnd`.
pub fn executable(hwnd: HWND) -> Option<PathBuf> {
    let pid = process_id(hwnd);
    if pid == 0 {
        return None;
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    let read = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    };
    let _ = unsafe { CloseHandle(process) };
    read.ok()?;
    Some(PathBuf::from(String::from_utf16_lossy(
        &buf[..len as usize],
    )))
}

/// The application's id: the executable's file name, in lower case.
pub fn id_of(path: &Path) -> Option<String> {
    Some(path.file_name()?.to_string_lossy().to_lowercase())
}

/// The id of the application that owns `hwnd`.
pub fn app_id(hwnd: HWND) -> Option<String> {
    id_of(&executable(hwnd)?)
}

/// The application of the executable at `path`.
pub fn app(path: &Path) -> Option<App> {
    let id = id_of(path)?;
    let name = description(path).unwrap_or_else(|| {
        path.file_stem()
            .map_or_else(|| id.clone(), |s| s.to_string_lossy().into_owned())
    });
    Some(App { id, name })
}

/// The FileDescription of the executable's version resource, in the first
/// language it lists.
fn description(path: &Path) -> Option<String> {
    let file = HSTRING::from(path.as_os_str());
    let size = unsafe { GetFileVersionInfoSizeW(&file, None) };
    if size == 0 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    unsafe { GetFileVersionInfoW(&file, None, size, data.as_mut_ptr().cast()) }.ok()?;
    let query = |sub: &str| -> Option<(*const u8, u32)> {
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        let found = unsafe {
            VerQueryValueW(
                data.as_ptr().cast(),
                &HSTRING::from(sub),
                &mut ptr,
                &mut len,
            )
        };
        (found.as_bool() && !ptr.is_null() && len > 0).then_some((ptr as *const u8, len))
    };
    // An array of (language, code page) pairs, two WORDs each.
    let (ptr, len) = query(r"\VarFileInfo\Translation")?;
    if len < 4 {
        return None;
    }
    // SAFETY: the translation array is at least one pair long.
    let pair = unsafe { std::slice::from_raw_parts(ptr as *const u16, 2) };
    let (ptr, len) = query(&format!(
        r"\StringFileInfo\{:04x}{:04x}\FileDescription",
        pair[0], pair[1]
    ))?;
    // SAFETY: for a string, len is its length in characters, the
    // terminating NUL included.
    let text = unsafe { std::slice::from_raw_parts(ptr as *const u16, len as usize) };
    let text = String::from_utf16_lossy(text);
    let text = text.trim_end_matches('\0').trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// The applications that have a window on the screen now, but for
/// meta-gesture itself, each once, by name.
pub fn running() -> Vec<App> {
    unsafe extern "system" fn each(hwnd: HWND, found: LPARAM) -> BOOL {
        // SAFETY: `found` points at the list below.
        let found = unsafe { &mut *(found.0 as *mut Vec<HWND>) };
        let shown = unsafe { IsWindowVisible(hwnd) }.as_bool();
        let owned = unsafe { GetWindow(hwnd, GW_OWNER) }.is_ok();
        let tool = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32 & WS_EX_TOOLWINDOW.0 != 0;
        let titled = unsafe { GetWindowTextLengthW(hwnd) } > 0;
        if shown && !owned && !tool && titled {
            found.push(hwnd);
        }
        true.into()
    }
    let mut windows: Vec<HWND> = Vec::new();
    let _ = unsafe { EnumWindows(Some(each), LPARAM(&mut windows as *mut _ as isize)) };
    let mut apps: Vec<App> = Vec::new();
    for hwnd in windows {
        if is_own(hwnd) {
            continue;
        }
        let Some(app) = executable(hwnd).and_then(|p| app(&p)) else {
            continue;
        };
        if !apps.iter().any(|a| a.id == app.id) {
            apps.push(app);
        }
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}
