#[cfg(target_os = "windows")]
mod autofill;
mod commands;
#[cfg(target_os = "windows")]
mod context_menu;
#[cfg(target_os = "macos")]
mod edit_menu;
mod fonts;
#[cfg(target_os = "windows")]
mod jumplist;
mod menu;
mod state;
#[cfg(target_os = "macos")]
mod traffic_lights;
mod window_position;

use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewWindow};

/// Smallest window size we will restore, matching `minWidth`/`minHeight` in
/// `tauri.conf.json`.
const MIN_WIDTH: f64 = 320.0;
const MIN_HEIGHT: f64 = 200.0;

/// How long the main window may stay hidden waiting for the frontend to show
/// it. Only a frontend that fails to start ever gets this far.
const SHOW_FALLBACK: Duration = Duration::from_secs(10);

pub fn run() {
    let builder = tauri::Builder::default();
    // A jump list task can only start the executable again, so the second
    // process forwards its arguments to this one and exits.
    #[cfg(target_os = "windows")]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            if argv.iter().any(|arg| arg == jumplist::RESET_POSITION_ARG) {
                window_position::reset(&window);
            }
            let _ = window.set_focus();
        }
        if argv.iter().any(|arg| arg == jumplist::PREFERENCES_ARG) {
            // Same path the macOS menu takes; `src/commands.ts` maps the id.
            if let Err(err) = tauri::Emitter::emit(app, "menu", "preferences") {
                eprintln!("draftpad: failed to forward the jump list task: {err}");
            }
        }
    }));
    builder
        // "Paste as plain text" reads the clipboard through this plugin; the
        // webview may only call `read_text` (`capabilities/default.json`).
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            commands::load_state,
            commands::save_state,
            commands::list_fonts,
            commands::quit_app,
            commands::show_context_menu,
            commands::set_menu_shortcuts,
        ])
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .expect("the main window is declared in tauri.conf.json");
            // The window starts hidden (`visible: false`) so the saved size and
            // position can be applied before anything is drawn.
            restore_window(app.handle(), &window);
            // Showing it is the frontend's job: Windows anchors the IME's
            // composition string and candidate list to the caret, and a window
            // that is activated before anything inside it holds the caret
            // leaves both at the top-left of the screen. `src/main.ts` shows
            // the window once the editor has the focus.
            show_after_fallback(window.clone());
            // Both the menu bar and the right-click menu report through this.
            menu::forward_events(app.handle());
            #[cfg(target_os = "macos")]
            {
                // Before the menu is built: AppKit reads the defaults while
                // the app starts.
                edit_menu::disable_input_items();
                menu::install(app.handle())?;
            }
            #[cfg(target_os = "windows")]
            {
                // The page draws the whole title bar, window buttons included
                // (`src/index.html`), so that the bar can carry draftpad's own
                // buttons and run on into the pane bar under it. Done here
                // rather than in `tauri.conf.json` because macOS keeps its
                // frame and lays the page under a transparent title bar
                // instead. The window is still hidden, so the frame it had is
                // never seen.
                if let Err(err) = window.set_decorations(false) {
                    eprintln!("draftpad: failed to remove the window frame: {err}");
                }
                autofill::disable(&window);
                context_menu::install(&window);
                jumplist::install();
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        // The Edit menu is AppKit's to add to until the app has finished
        // launching, which is what `RunEvent::Ready` says has happened.
        .run(|_app, _event| {
            #[cfg(target_os = "macos")]
            if matches!(_event, tauri::RunEvent::Ready) {
                edit_menu::trim(menu::EDIT_TITLE, menu::FIND);
            }
        });
}

fn restore_window(app: &AppHandle, window: &WebviewWindow) {
    let saved = state::path(app)
        .ok()
        .and_then(|path| state::load(&path).ok())
        .unwrap_or_default();
    restore_window_size(window, saved.window_width, saved.window_height);
    if started_for_reset() {
        window_position::reset(window);
    } else {
        window_position::restore(window, saved.window_x, saved.window_y);
    }
}

fn restore_window_size(window: &WebviewWindow, width: Option<f64>, height: Option<f64>) {
    let (Some(width), Some(height)) = (width, height) else {
        return;
    };
    if width < MIN_WIDTH || height < MIN_HEIGHT || !width.is_finite() || !height.is_finite() {
        return;
    }
    if let Err(err) = window.set_size(tauri::LogicalSize::new(width, height)) {
        eprintln!("draftpad: failed to restore window size: {err}");
    }
}

/// True when the jump list's "ウィンドウ位置をリセット" task started this
/// process, so that the window opens where that task puts it rather than where
/// it was left.
#[cfg(target_os = "windows")]
fn started_for_reset() -> bool {
    jumplist::started_with(jumplist::RESET_POSITION_ARG)
}

#[cfg(not(target_os = "windows"))]
fn started_for_reset() -> bool {
    false
}

/// Shows `window` if the frontend has not shown it within [`SHOW_FALLBACK`], so
/// that a frontend which fails to start leaves a visible window rather than an
/// invisible process.
fn show_after_fallback(window: WebviewWindow) {
    std::thread::spawn(move || {
        std::thread::sleep(SHOW_FALLBACK);
        // Showing a window the frontend already showed would pull it back in
        // front of whatever the user moved on to.
        if window.is_visible().unwrap_or(false) {
            return;
        }
        if let Err(err) = window.show() {
            eprintln!("draftpad: failed to show the window: {err}");
        }
    });
}
