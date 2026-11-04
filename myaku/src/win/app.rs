//! The resident part: a hidden window that owns the notification-area icon
//! and its menu, and the message loop that drives everything.

use std::cell::RefCell;

use windows::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NIN_SELECT,
    NINF_KEY, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, FindWindowW, GetCursorPos, GetMessageW, HICON, MF_CHECKED,
    MF_SEPARATOR, MF_STRING, MF_UNCHECKED, MSG, PostMessageW, PostQuitMessage, RegisterClassExW,
    RegisterWindowMessageW, SetForegroundWindow, TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WM_CONTEXTMENU, WM_DESTROY,
    WM_DPICHANGED, WM_SETTINGCHANGE, WNDCLASSEXW, WS_OVERLAPPED,
};
use windows::core::{HSTRING, PCSTR, PCWSTR, w};

use super::{
    APP_NAME, RUNNER, WM_APP_SHOW, WM_APP_TRAY, WM_APP_UPDATED, autostart, copy_to_field,
    error_box, flyout, icon, loword, sampler, theme,
};
use myaku::runner::Runner;

/// The installer finds a running myaku by this name to close it
/// (installer/installer.nsi, MAIN_CLASS).
const CLASS_NAME: PCWSTR = w!("myaku.main");
pub const TRAY_ID: u32 = 1;
const MENU_AUTOSTART: usize = 1;
const MENU_QUIT: usize = 2;
/// Enter or Space on the focused icon; the headers define it as this.
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;

struct App {
    hwnd: HWND,
    icon: HICON,
    /// Broadcast when Explorer (re)starts, which empties the notification
    /// area.
    taskbar_created: u32,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` on the application state. Never call anything that can send a
/// message to one of myaku's windows inside `f`: the window procedure would
/// try to borrow the state again.
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok()?.as_mut().map(f))
}

pub fn run() {
    // One copy at a time. A second one asks the first to open its list,
    // which is what someone starting myaku again most likely wants.
    let _mutex = unsafe { CreateMutexW(None, false, w!("Local\\myaku.single-instance")) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        if let Ok(existing) = unsafe { FindWindowW(CLASS_NAME, None) } {
            let _ = unsafe { PostMessageW(Some(existing), WM_APP_SHOW, WPARAM(0), LPARAM(0)) };
        }
        return;
    }

    let hwnd = match create_main_window() {
        Ok(hwnd) => hwnd,
        Err(e) => {
            error_box(None, &format!("起動できませんでした。\n{e}"));
            return;
        }
    };
    apply_menu_theme();

    // The window handle crosses to the sampling thread as a number, as
    // HWND is not Send; posting to it from any thread is allowed.
    let target = hwnd.0 as isize;
    let _ = RUNNER.set(Runner::start(
        sampler::read,
        false,
        Box::new(move || unsafe {
            let _ = PostMessageW(
                Some(HWND(target as _)),
                WM_APP_UPDATED,
                WPARAM(0),
                LPARAM(0),
            );
        }),
    ));

    if let Err(e) = flyout::create() {
        error_box(None, &format!("起動できませんでした。\n{e}"));
        return;
    }

    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            hwnd,
            icon: icon::tray(unsafe { GetDpiForSystem() }, theme::taskbar_is_dark()),
            taskbar_created: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
        })
    });
    add_tray_icon();

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn create_main_window() -> windows::core::Result<HWND> {
    let instance = unsafe { GetModuleHandleW(None) }?;
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wndproc),
        hInstance: instance.into(),
        lpszClassName: CLASS_NAME,
        ..Default::default()
    };
    unsafe { RegisterClassExW(&class) };
    // A top-level window that is never shown, rather than a message-only
    // one: only top-level windows receive the TaskbarCreated broadcast and
    // WM_SETTINGCHANGE.
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            CLASS_NAME,
            &HSTRING::from(APP_NAME),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }
}

fn tray_data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        ..Default::default()
    }
}

fn add_tray_icon() {
    let Some(mut data) = with_app(|app| {
        let mut data = tray_data(app.hwnd);
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = WM_APP_TRAY;
        data.hIcon = app.icon;
        copy_to_field(&mut data.szTip, APP_NAME);
        data
    }) else {
        return;
    };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &data);
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
    }
}

/// Draws the icon again: the taskbar turned light or dark, or the scale
/// changed.
fn refresh_tray_icon() {
    let Some((hwnd, old)) = with_app(|app| (app.hwnd, app.icon)) else {
        return;
    };
    let new = icon::tray(unsafe { GetDpiForSystem() }, theme::taskbar_is_dark());
    let mut data = tray_data(hwnd);
    data.uFlags = NIF_ICON;
    data.hIcon = new;
    let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
    with_app(|app| app.icon = new);
    let _ = unsafe { DestroyIcon(old) };
}

fn remove_tray_icon() {
    if let Some(data) = with_app(|app| tray_data(app.hwnd)) {
        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
    }
}

pub fn main_window() -> Option<HWND> {
    with_app(|app| app.hwnd)
}

/// Makes the menu light or dark as Windows is, as the menus of Explorer and
/// most other applications are. Windows offers this only through two
/// undocumented uxtheme.dll exports, known by ordinal: 135 is
/// SetPreferredAppMode (1 follows Windows), 136 FlushMenuThemes. Should a
/// Windows update drop them, the menu is simply light.
fn apply_menu_theme() {
    unsafe {
        let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) else {
            return;
        };
        if let Some(f) = GetProcAddress(uxtheme, PCSTR(135 as *const u8)) {
            // SAFETY: ordinal 135 takes one int on every Windows version
            // that has it.
            let set_preferred_app_mode: unsafe extern "system" fn(i32) -> i32 =
                std::mem::transmute(f);
            set_preferred_app_mode(1);
        }
        if let Some(f) = GetProcAddress(uxtheme, PCSTR(136 as *const u8)) {
            // SAFETY: ordinal 136 takes nothing and returns nothing.
            let flush_menu_themes: unsafe extern "system" fn() = std::mem::transmute(f);
            flush_menu_themes();
        }
    }
}

fn show_menu(hwnd: HWND) {
    let Ok(menu) = (unsafe { CreatePopupMenu() }) else {
        return;
    };
    let autostart = if autostart::is_enabled() {
        MF_CHECKED
    } else {
        MF_UNCHECKED
    };
    unsafe {
        let _ = AppendMenuW(
            menu,
            MF_STRING | autostart,
            MENU_AUTOSTART,
            w!("サインイン時に起動"),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(menu, MF_STRING, MENU_QUIT, w!("myaku を終了"));
    }
    let mut pt = POINT::default();
    let _ = unsafe { GetCursorPos(&mut pt) };
    // Without this the menu stays open when the user clicks elsewhere.
    let _ = unsafe { SetForegroundWindow(hwnd) };
    let chosen = unsafe {
        TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        )
    }
    .0 as usize;
    let _ = unsafe { DestroyMenu(menu) };
    match chosen {
        MENU_AUTOSTART => {
            if let Err(e) = autostart::set_enabled(!autostart::is_enabled()) {
                error_box(None, &e);
            }
        }
        MENU_QUIT => {
            let _ = unsafe { DestroyWindow(hwnd) };
        }
        _ => {}
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_APP_TRAY => {
            // NOTIFYICON_VERSION_4: the event is in the low word of lParam.
            match loword(lparam.0 as usize) {
                NIN_SELECT | NIN_KEYSELECT => flyout::toggle(),
                WM_CONTEXTMENU => {
                    flyout::hide();
                    show_menu(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_APP_UPDATED => {
            flyout::refresh();
            LRESULT(0)
        }
        WM_APP_SHOW => {
            flyout::show();
            LRESULT(0)
        }
        WM_SETTINGCHANGE | WM_DPICHANGED => {
            refresh_tray_icon();
            flyout::theme_changed();
            apply_menu_theme();
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_DESTROY => {
            remove_tray_icon();
            if let Some(icon) = with_app(|app| app.icon) {
                let _ = unsafe { DestroyIcon(icon) };
            }
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => {
            let taskbar_created = with_app(|app| app.taskbar_created);
            if taskbar_created.is_some_and(|m| m != 0 && m == msg) {
                // Explorer restarted, possibly at another scale.
                refresh_tray_icon();
                add_tray_icon();
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}
