//! The resident part: a hidden window that owns the global shortcuts and the
//! notification-area icon, and the message loop that drives everything.

use std::cell::RefCell;
use std::path::PathBuf;

use windows::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NIM_SETVERSION, NIN_SELECT, NINF_KEY, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CHILDID_SELF, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DestroyWindow, DispatchMessageW, EVENT_OBJECT_LOCATIONCHANGE,
    EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZESTART, FindWindowW, GetCursorPos, GetMessageW,
    GetWindowThreadProcessId, HICON, IsWindow, MF_SEPARATOR, MF_STRING, MSG, OBJID_WINDOW,
    PostMessageW, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, SetForegroundWindow,
    TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
    TranslateMessage, WINDOW_EX_STYLE, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
    WM_CONTEXTMENU, WM_DESTROY, WM_HOTKEY, WM_TIMER, WNDCLASSEXW, WS_OVERLAPPED,
};
use windows::core::{HSTRING, PCSTR, PCWSTR, w};

use super::{
    APP_NAME, WM_APP_OPEN_SETTINGS, WM_APP_SETTINGS_CLOSED, WM_APP_TRAY, config_path,
    copy_to_field, error_box, icon, loword, mover, settings, shortcuts::Shortcuts,
};
use crate::config::{Config, Theme};
use crate::cycle;
use crate::restore::Sizes;

/// The installer finds a running winwin by this name to close it
/// (installer/installer.nsi, MAIN_CLASS).
const CLASS_NAME: PCWSTR = w!("winwin.main");
const TRAY_ID: u32 = 1;
const MENU_SETTINGS: usize = 1;
const MENU_QUIT: usize = 2;
/// Enter or Space on the focused icon; the headers define it as this.
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;

struct App {
    hwnd: HWND,
    config: Config,
    /// The config's shortcuts, while they are registered.
    shortcuts: Shortcuts,
    path: PathBuf,
    icon: HICON,
    /// Broadcast when Explorer (re)starts, which empties the notification
    /// area.
    taskbar_created: u32,
    /// The settings window, while it is open.
    settings: Option<settings::Child>,
    /// Whether the user has already been told that elevated windows cannot
    /// be moved; once per run is enough.
    told_access_denied: bool,
    /// The sizes of the windows the shortcuts have placed, to give back
    /// when one is dragged away.
    sizes: Sizes,
    /// Hears the start and end of every window's move or resize.
    move_size_hook: HWINEVENTHOOK,
    /// Hears the window being dragged move, until it gets its size back.
    drag_hook: Option<(HWINEVENTHOOK, isize)>,
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
            config,
            shortcuts: Shortcuts::new(hwnd),
            path,
            icon: icon::create(dpi),
            taskbar_created: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
            settings: None,
            told_access_denied: false,
            sizes: Sizes::default(),
            move_size_hook: install_move_size_hook(),
            drag_hook: None,
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
    with_app(|app| app.shortcuts.clear());
}

/// Registers the config's shortcuts, and says which ones another
/// application already holds.
fn register_hotkeys() {
    let failed =
        with_app(|app| app.shortcuts.set(cycle::bindings(&app.config))).unwrap_or_default();
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
    let Some(placement) = with_app(|app| app.shortcuts.on_hotkey(id)).flatten() else {
        return;
    };
    match mover::place_foreground(&placement) {
        Ok(Some(placed)) => {
            with_app(|app| {
                app.sizes
                    .retain(|window| unsafe { IsWindow(Some(HWND(window as _))) }.as_bool());
                app.sizes.placed(
                    placed.hwnd.0 as isize,
                    placed.before,
                    placed.dpi,
                    placed.after,
                );
            });
        }
        Ok(None) => {}
        Err(mover::MoveError::AccessDenied) => {
            let first = with_app(|app| !std::mem::replace(&mut app.told_access_denied, true));
            if first == Some(true) {
                notify(
                    "このウィンドウは動かせません",
                    "管理者として実行されているウィンドウは、winwin も管理者として実行しているときだけ動かせます。",
                );
            }
        }
    }
}

/// Out of context, so the calls arrive through the message loop and nothing
/// is loaded into other processes. Only the start and end of a move or
/// resize come, not the steps in between.
fn install_move_size_hook() -> HWINEVENTHOOK {
    unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_MOVESIZESTART,
            EVENT_SYSTEM_MOVESIZEEND,
            None,
            Some(on_move_size),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    }
}

fn cursor_pos() -> (i32, i32) {
    let mut cursor = POINT::default();
    let _ = unsafe { GetCursorPos(&mut cursor) };
    (cursor.x, cursor.y)
}

/// Gives a placed window its size back when it is dragged away
/// (`restore.rs` decides). Between the start and the end, the window's own
/// thread is watched for it to move, and only until it has.
extern "system" fn on_move_size(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    if hwnd.is_invalid() || object != OBJID_WINDOW.0 || child != CHILDID_SELF as i32 {
        return;
    }
    let window = hwnd.0 as isize;
    if with_app(|app| app.sizes.tracks(window)) != Some(true) {
        return;
    }
    let Some(current) = mover::window_rect(hwnd) else {
        return;
    };
    match event {
        EVENT_SYSTEM_MOVESIZESTART => {
            stop_watching_drag();
            let resizing = mover::is_resize_cursor();
            if with_app(|app| app.sizes.drag_started(window, current, resizing)) == Some(true) {
                watch_drag(hwnd);
            }
        }
        EVENT_SYSTEM_MOVESIZEEND => {
            stop_watching_drag();
            let arranged = mover::is_arranged(hwnd);
            let dpi = mover::monitor_dpi(hwnd);
            let cursor = cursor_pos();
            let back = with_app(|app| app.sizes.drag_ended(window, current, dpi, cursor, arranged))
                .flatten();
            if let Some(rect) = back {
                mover::put(hwnd, rect);
            }
        }
        _ => {}
    }
}

/// Hears `hwnd` move, through the location changes of its own thread only.
fn watch_drag(hwnd: HWND) {
    let mut process = 0;
    let thread = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process)) };
    if thread == 0 {
        return;
    }
    let hook = unsafe {
        SetWinEventHook(
            EVENT_OBJECT_LOCATIONCHANGE,
            EVENT_OBJECT_LOCATIONCHANGE,
            None,
            Some(on_drag_step),
            process,
            thread,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    if hook.is_invalid() {
        return;
    }
    with_app(|app| app.drag_hook = Some((hook, hwnd.0 as isize)));
}

fn stop_watching_drag() {
    if let Some((hook, _)) = with_app(|app| app.drag_hook.take()).flatten() {
        let _ = unsafe { UnhookWinEvent(hook) };
    }
}

/// The dragged window moved. The first time it leaves the place winwin put
/// it, it gets its size back. Only the size: the position stays with the
/// system's move loop, which moves the window from where the drag started,
/// so that changing it would have the two fight.
extern "system" fn on_drag_step(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    if object != OBJID_WINDOW.0 || child != CHILDID_SELF as i32 {
        return;
    }
    let window = hwnd.0 as isize;
    let dragged = with_app(|app| app.drag_hook.map(|(_, w)| w)).flatten();
    if dragged != Some(window) {
        return;
    }
    let Some(current) = mover::window_rect(hwnd) else {
        return;
    };
    let dpi = mover::monitor_dpi(hwnd);
    let cursor = cursor_pos();
    let size = with_app(|app| app.sizes.moved(window, current, dpi, cursor)).flatten();
    if let Some((width, height)) = size {
        stop_watching_drag();
        mover::resize(hwnd, width, height);
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
            with_app(|app| app.config = config);
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
        WM_TIMER if with_app(|app| app.shortcuts.on_timer(wparam.0)) == Some(true) => LRESULT(0),
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
            stop_watching_drag();
            if let Some(hook) = with_app(|app| app.move_size_hook) {
                let _ = unsafe { UnhookWinEvent(hook) };
            }
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
