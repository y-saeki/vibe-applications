mod commands;
mod fonts;
#[cfg(target_os = "macos")]
mod menu;
mod state;

use tauri::{AppHandle, Manager, WebviewWindow};

/// Smallest window size we will restore, matching `minWidth`/`minHeight` in
/// `tauri.conf.json`.
const MIN_WIDTH: f64 = 320.0;
const MIN_HEIGHT: f64 = 200.0;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::load_state,
            commands::save_state,
            commands::list_fonts,
            commands::quit_app,
        ])
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .expect("the main window is declared in tauri.conf.json");
            // The window starts hidden (`visible: false`) so the saved size can
            // be applied before anything is drawn. The position is deliberately
            // not restored: the window is always centered.
            restore_window_size(app.handle(), &window);
            window.show()?;
            #[cfg(target_os = "macos")]
            menu::install(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn restore_window_size(app: &AppHandle, window: &WebviewWindow) {
    let Ok(path) = state::path(app) else {
        return;
    };
    let Ok(saved) = state::load(&path) else {
        return;
    };
    let (Some(width), Some(height)) = (saved.window_width, saved.window_height) else {
        return;
    };
    if width < MIN_WIDTH || height < MIN_HEIGHT || !width.is_finite() || !height.is_finite() {
        return;
    }
    if let Err(err) = window.set_size(tauri::LogicalSize::new(width, height)) {
        eprintln!("draftpad: failed to restore window size: {err}");
        return;
    }
    if let Err(err) = window.center() {
        eprintln!("draftpad: failed to center window: {err}");
    }
}
