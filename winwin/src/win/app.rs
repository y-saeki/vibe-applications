//! The resident part: a hidden window that owns the global shortcuts and the
//! notification-area icon, and the message loop that drives everything.

use std::cell::RefCell;
use std::path::PathBuf;

use windows::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NIM_SETVERSION, NIN_SELECT, NINF_KEY, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, FindWindowW, GetCursorPos, GetMessageW, HICON, KillTimer,
    MF_SEPARATOR, MF_STRING, MSG, PostMessageW, PostQuitMessage, RegisterClassExW,
    RegisterWindowMessageW, SetForegroundWindow, SetTimer, TPM_BOTTOMALIGN, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE,
    WM_CONTEXTMENU, WM_DESTROY, WM_HOTKEY, WM_TIMER, WNDCLASSEXW, WS_OVERLAPPED,
};
use windows::core::{HSTRING, PCSTR, PCWSTR, w};

use super::{
    APP_NAME, WM_APP_OPEN_SETTINGS, WM_APP_SETTINGS_CLOSED, WM_APP_TRAY, config_path,
    copy_to_field, error_box, icon, loword, modifiers_held, mover, settings,
};
use crate::config::{Config, Theme};
use crate::cycle::{self, Binding, Cycle};

/// The installer finds a running winwin by this name to close it
/// (installer/installer.nsi, MAIN_CLASS).
const CLASS_NAME: PCWSTR = w!("winwin.main");
const TRAY_ID: u32 = 1;
const MENU_SETTINGS: usize = 1;
const MENU_QUIT: usize = 2;
/// Enter or Space on the focused icon; the headers define it as this.
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;
/// Watches for the modifiers of a cycling shortcut to be let go. It runs
/// only between such a press and that release.
const CYCLE_TIMER: usize = 1;
const CYCLE_POLL_MS: u32 = 15;

struct App {
    hwnd: HWND,
    config: Config,
    /// The config's shortcuts by key combination; hotkey id `n` is
    /// `bindings[n - 1]`.
    bindings: Vec<Binding>,
    cycle: Cycle,
    path: PathBuf,
    icon: HICON,
    /// Broadcast when Explorer (re)starts, which empties the notification
    /// area.
    taskbar_created: u32,
    /// How many hotkey ids are registered, from 1 up.
    registered: i32,
    /// The settings window, while it is open.
    settings: Option<settings::Child>,
    /// Whether the user has already been told that elevated windows cannot
    /// be moved; once per run is enough.
    told_access_denied: bool,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` on the application state. Never call anything that can pump
/// messages (a message box, SetWindowPos on our own windows) inside `f`: the
/// window procedure would try to borrow the state again.
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok()?.as_mut().map(f))
}

pub fn run() {
    // One copy at a time. A second one asks the first to show its settings,
    // which is what someone starting winwin again most likely wants.
    let _mutex = unsafe { CreateMutexW(None, false, w!("Local\\winwin.single-instance")) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        if let Ok(existing) = unsafe { FindWindowW(CLASS_NAME, None) } {
            let _ =
                unsafe { PostMessageW(Some(existing), WM_APP_OPEN_SETTINGS, WPARAM(0), LPARAM(0)) };
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

    let path = config_path();
    let (config, load_error) = match Config::load_or_create(&path) {
        Ok(config) => (config, None),
        Err(e) => (Config::default(), Some(e.to_string())),
    };
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            hwnd,
            bindings: cycle::bindings(&config),
            config,
            cycle: Cycle::default(),
            path,
            icon: icon::create(dpi),
            taskbar_created: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
            registered: 0,
            settings: None,
            told_access_denied: false,
        })
    });

    add_tray_icon();
    if let Some(e) = load_error {
        error_box(
            None,
            &format!(
                "{e}\n\nショートカットは登録されていません。設定を開いて保存し直すか、ファイルを修正してください。"
            ),
        );
    }
    register_hotkeys();

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
    // one: only top-level windows receive the TaskbarCreated broadcast.
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

fn tray_data(app: &App) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: app.hwnd,
        uID: TRAY_ID,
        ..Default::default()
    }
}

fn add_tray_icon() {
    let Some(mut data) = with_app(|app| {
        let mut data = tray_data(app);
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

fn remove_tray_icon() {
    if let Some(data) = with_app(|app| tray_data(app)) {
        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
    }
}

fn notify(title: &str, text: &str) {
    let Some(mut data) = with_app(|app| tray_data(app)) else {
        return;
    };
    data.uFlags = NIF_INFO;
    data.dwInfoFlags = NIIF_WARNING;
    copy_to_field(&mut data.szInfoTitle, title);
    copy_to_field(&mut data.szInfo, text);
    let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
}

fn unregister_hotkeys() {
    stop_cycle();
    with_app(|app| {
        for id in 1..=app.registered {
            let _ = unsafe { UnregisterHotKey(Some(app.hwnd), id) };
        }
        app.registered = 0;
    });
}

/// Registers every key combination in the config once, and says which ones
/// another application already holds.
fn register_hotkeys() {
    let failed = with_app(|app| {
        let mut failed = Vec::new();
        for (i, b) in app.bindings.iter().enumerate() {
            let id = i as i32 + 1;
            let modifiers = HOT_KEY_MODIFIERS(b.keys.modifiers) | MOD_NOREPEAT;
            if unsafe { RegisterHotKey(Some(app.hwnd), id, modifiers, b.keys.vk) }.is_err() {
                failed.push(b.keys.to_string());
            }
        }
        app.registered = app.bindings.len() as i32;
        failed
    })
    .unwrap_or_default();
    if !failed.is_empty() {
        notify(
            "使えないショートカットがあります",
            &format!(
                "他のアプリケーションか Windows が使用中です:\n{}",
                failed.join("\n")
            ),
        );
    }
}

fn on_hotkey(id: usize) {
    let picked = with_app(|app| {
        let index = id.wrapping_sub(1);
        let b = app.bindings.get(index)?;
        let entry = app.cycle.press(index, b.placements.len());
        Some((app.hwnd, b.placements[entry], b.cycles()))
    })
    .flatten();
    let Some((hwnd, placement, cycles)) = picked else {
        return;
    };
    if cycles {
        // Restarting an already running timer just resets its interval.
        unsafe { SetTimer(Some(hwnd), CYCLE_TIMER, CYCLE_POLL_MS, None) };
    }
    if let Err(mover::MoveError::AccessDenied) = mover::place_foreground(&placement) {
        let first = with_app(|app| !std::mem::replace(&mut app.told_access_denied, true));
        if first == Some(true) {
            notify(
                "このウィンドウは動かせません",
                "管理者として実行されているウィンドウは、winwin も管理者として実行しているときだけ動かせます。",
            );
        }
    }
}

/// Letting go of any modifier of the cycling shortcut counts as letting go:
/// the next press starts from its first entry.
fn on_cycle_timer() {
    let modifiers = with_app(|app| {
        let b = app.cycle.active()?;
        app.bindings.get(b).map(|b| b.keys.modifiers)
    })
    .flatten();
    if modifiers.is_none_or(|m| !modifiers_held(m)) {
        stop_cycle();
    }
}

fn stop_cycle() {
    if let Some(hwnd) = with_app(|app| {
        app.cycle.reset();
        app.hwnd
    }) {
        let _ = unsafe { KillTimer(Some(hwnd), CYCLE_TIMER) };
    }
}

fn open_settings() {
    let Some((hwnd, open)) = with_app(|app| {
        if let Some(child) = &app.settings {
            child.bring_forward();
        }
        (app.hwnd, app.settings.is_some())
    }) else {
        return;
    };
    if open {
        return;
    }
    // Shortcuts are released for as long as the window is open, so that
    // pressing one while editing does not move the settings window about.
    unregister_hotkeys();
    match settings::open(hwnd) {
        Ok(child) => {
            with_app(|app| app.settings = Some(child));
        }
        Err(e) => {
            error_box(None, &format!("設定を開けませんでした。\n{e}"));
            register_hotkeys();
        }
    }
}

/// The settings window has closed, saved or not. The file is the truth
/// either way, so it is read again rather than handed over.
fn on_settings_closed() {
    let Some((path, child)) = with_app(|app| (app.path.clone(), app.settings.take())) else {
        return;
    };
    let Some(child) = child else {
        return;
    };
    child.release();
    match Config::load_or_create(&path) {
        Ok(config) => {
            with_app(|app| {
                app.bindings = cycle::bindings(&config);
                app.config = config;
            });
        }
        Err(e) => error_box(None, &e.to_string()),
    }
    register_hotkeys();
}

/// Makes the tray menu light or dark as the settings say, following Windows
/// by default as the menus of Explorer and most other applications do.
/// Windows offers this only through two undocumented uxtheme.dll exports,
/// known by ordinal: 135 is SetPreferredAppMode (1 follows Windows, 2 forces
/// dark, 3 forces light; on 1809 it is AllowDarkModeForApp, where any of
/// them allows dark), 136 FlushMenuThemes. Should a Windows update drop them,
/// the menu is simply light.
fn apply_menu_theme(theme: Theme) {
    let mode = match theme {
        Theme::System => 1,
        Theme::Dark => 2,
        Theme::Light => 3,
    };
    unsafe {
        let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) else {
            return;
        };
        if let Some(f) = GetProcAddress(uxtheme, PCSTR(135 as *const u8)) {
            // SAFETY: ordinal 135 takes one int on every Windows version
            // that has it.
            let set_preferred_app_mode: unsafe extern "system" fn(i32) -> i32 =
                std::mem::transmute(f);
            set_preferred_app_mode(mode);
        }
        if let Some(f) = GetProcAddress(uxtheme, PCSTR(136 as *const u8)) {
            // SAFETY: ordinal 136 takes nothing and returns nothing.
            let flush_menu_themes: unsafe extern "system" fn() = std::mem::transmute(f);
            flush_menu_themes();
        }
    }
}

fn show_menu(hwnd: HWND) {
    // Every time, so that a change of theme since the last menu is picked
    // up.
    if let Some(theme) = with_app(|app| app.config.theme) {
        apply_menu_theme(theme);
    }
    let Ok(menu) = (unsafe { CreatePopupMenu() }) else {
        return;
    };
    unsafe {
        let _ = AppendMenuW(menu, MF_STRING, MENU_SETTINGS, w!("設定を開く(&S)"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(menu, MF_STRING, MENU_QUIT, w!("終了(&X)"));
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
        MENU_SETTINGS => open_settings(),
        MENU_QUIT => {
            let _ = unsafe { DestroyWindow(hwnd) };
        }
        _ => {}
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            on_hotkey(wparam.0);
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == CYCLE_TIMER => {
            on_cycle_timer();
            LRESULT(0)
        }
        WM_APP_TRAY => {
            // NOTIFYICON_VERSION_4: the event is in the low word of lParam.
            match loword(lparam.0 as usize) {
                NIN_SELECT | NIN_KEYSELECT => open_settings(),
                WM_CONTEXTMENU => show_menu(hwnd),
                _ => {}
            }
            LRESULT(0)
        }
        WM_APP_OPEN_SETTINGS => {
            open_settings();
            LRESULT(0)
        }
        WM_APP_SETTINGS_CLOSED => {
            on_settings_closed();
            LRESULT(0)
        }
        WM_DESTROY => {
            if let Some(child) = with_app(|app| app.settings.take()).flatten() {
                child.terminate();
            }
            unregister_hotkeys();
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
                add_tray_icon();
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}
