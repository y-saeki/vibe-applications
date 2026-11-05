//! The resident part: a hidden window that owns the notification-area icon
//! and the mouse hook, and the message loop that drives everything.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use windows::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NIM_SETVERSION, NIN_SELECT, NINF_KEY, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, FindWindowW, GetCursorPos, GetMessageW, HICON, MF_SEPARATOR,
    MF_STRING, MSG, PostMessageW, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW,
    SetForegroundWindow, TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WM_CONTEXTMENU, WM_DESTROY, WNDCLASSEXW,
    WS_OVERLAPPED,
};
use windows::core::{HSTRING, PCWSTR, w};

use super::{
    APP_NAME, WM_APP_GESTURE, WM_APP_OPEN_SETTINGS, WM_APP_SETTINGS_CLOSED, WM_APP_TRAY,
    config_path, copy_to_field, error_box, icon, loword, mousehook, process, send, settings, toast,
};
use meta_gesture::config::{Config, Outcome};

/// The installer finds a running meta-gesture by this name to close it
/// (installer/installer.nsi, MAIN_CLASS).
const CLASS_NAME: PCWSTR = w!("meta-gesture.main");
const TRAY_ID: u32 = 1;
const MENU_SETTINGS: usize = 1;
const MENU_QUIT: usize = 2;
/// Enter or Space on the focused icon; the headers define it as this.
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;

struct App {
    hwnd: HWND,
    /// Shared with the mouse hook, which reads it as each press starts.
    config: Rc<RefCell<Config>>,
    path: PathBuf,
    icon: HICON,
    /// Broadcast when Explorer (re)starts, which empties the notification
    /// area.
    taskbar_created: u32,
    /// The settings window, while it is open.
    settings: Option<settings::Child>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` on the application state. Never call anything that can pump
/// messages (a message box, a menu) inside `f`: the window procedure would
/// try to borrow the state again.
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok()?.as_mut().map(f))
}

pub fn run() {
    // One copy at a time. A second one asks the first to show its settings,
    // which is what someone starting meta-gesture again most likely wants.
    let _mutex = unsafe { CreateMutexW(None, false, w!("Local\\meta-gesture.single-instance")) };
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
    let config = Rc::new(RefCell::new(config));
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            hwnd,
            config: config.clone(),
            path,
            icon: icon::create(dpi),
            taskbar_created: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
            settings: None,
        })
    });

    add_tray_icon();
    if let Some(e) = load_error {
        error_box(
            None,
            &format!(
                "{e}\n\nジェスチャーは使えません。設定を開いて保存し直すか、ファイルを修正してください。"
            ),
        );
    }
    let follows: mousehook::Follows = Box::new(move |window| {
        if process::is_own(window) {
            return None;
        }
        let id = process::app_id(window)?;
        let config = config.try_borrow().ok()?;
        config.follows(&id).then_some(id)
    });
    if let Err(e) = mousehook::start(hwnd, follows) {
        error_box(None, &format!("マウスの操作を受け取れませんでした。\n{e}"));
    }

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

/// A press of the right button has ended: a click is given back, a gesture
/// is looked up and carried out.
fn on_gesture() {
    let Some(finished) = mousehook::take_finished() else {
        return;
    };
    let (window, app, gesture) = match finished {
        mousehook::Finished::Click => {
            send::click_right();
            return;
        }
        mousehook::Finished::Gesture {
            window,
            app,
            gesture,
        } => (window, app, gesture),
    };
    // A move that drew no stroke, or too many, does nothing.
    let Some(gesture) = gesture else {
        return;
    };
    let Some(outcome) = with_app(|a| a.config.borrow().lookup(&app, &gesture)) else {
        return;
    };
    match outcome {
        Outcome::Send { command, keys } => {
            toast::show(&command);
            if send::bring_forward(window) {
                send::keys(keys);
            } else {
                toast::show(&format!("{command}: ウィンドウを前面に出せませんでした"));
            }
        }
        // A Windows config has no menu items: reading it refuses them.
        Outcome::Unassigned { command } | Outcome::Menu { command, .. } => {
            toast::show(&format!("{command}: このアプリには未設定"));
        }
        Outcome::Unknown => toast::show(&format!("{gesture}: 未登録のジェスチャー")),
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
    match settings::open(hwnd) {
        Ok(child) => {
            with_app(|app| app.settings = Some(child));
        }
        Err(e) => error_box(None, &format!("設定を開けませんでした。\n{e}")),
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
            with_app(|app| *app.config.borrow_mut() = config);
        }
        Err(e) => {
            notify("設定を読めませんでした", &e.to_string());
        }
    }
}

fn show_menu(hwnd: HWND) {
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
        WM_APP_GESTURE => {
            on_gesture();
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
            mousehook::stop();
            if let Some(child) = with_app(|app| app.settings.take()).flatten() {
                child.terminate();
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
