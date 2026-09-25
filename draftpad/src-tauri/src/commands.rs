//! Commands invoked from the webview (`invoke(...)` in `src/state.ts`,
//! `src/preferences.ts`, `src/context-menu.ts` and `src/main.ts`).

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, WebviewWindow, Window};

use crate::fonts;
use crate::menu::{self, ContextState};
use crate::state::{self, State};

/// Everything the frontend needs at startup in a single round trip.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Loaded {
    pub state: State,
    /// `std::env::consts::OS`: "macos", "windows", "linux", ...
    pub platform: &'static str,
    pub version: String,
    /// Set when the Windows jump list task started this process, so the
    /// frontend opens the Preferences panel as soon as it is wired up.
    pub open_preferences: bool,
    /// macOS: how far down the window the middle of its traffic lights is,
    /// in points, so that the page's title bar can line its buttons up with
    /// them. `None` elsewhere, and whenever it cannot be told.
    pub traffic_lights_center: Option<f64>,
}

#[cfg(target_os = "windows")]
fn started_for_preferences() -> bool {
    crate::jumplist::started_for_preferences()
}

#[cfg(not(target_os = "windows"))]
fn started_for_preferences() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn traffic_lights_center(window: &WebviewWindow) -> Option<f64> {
    crate::traffic_lights::center(window)
}

#[cfg(not(target_os = "macos"))]
fn traffic_lights_center(_window: &WebviewWindow) -> Option<f64> {
    None
}

/// Where `state.json` lives, with the error as the webview receives it.
fn state_path(app: &AppHandle) -> Result<PathBuf, String> {
    state::path(app).map_err(|err| err.to_string())
}

/// Synchronous so that it runs on the main thread, which is the only one
/// AppKit answers `traffic_lights_center` on.
#[tauri::command]
pub fn load_state(app: AppHandle, window: WebviewWindow) -> Result<Loaded, String> {
    let path = state_path(&app)?;
    let state = state::load(&path).map_err(|err| err.to_string())?;
    Ok(Loaded {
        state,
        platform: std::env::consts::OS,
        version: app.package_info().version.to_string(),
        open_preferences: started_for_preferences(),
        traffic_lights_center: traffic_lights_center(&window),
    })
}

/// Async so the file write happens off the main thread.
#[tauri::command]
pub async fn save_state(app: AppHandle, state: State) -> Result<(), String> {
    let path = state_path(&app)?;
    state::save(&path, &state).map_err(|err| err.to_string())
}

/// Async because enumerating fonts can take a noticeable moment.
#[tauri::command]
pub async fn list_fonts() -> Vec<String> {
    fonts::families()
}

/// The frontend flushes the state first, then calls this to exit.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Opens the right-click menu the page asked for instead of the webview's own.
///
/// Synchronous on purpose: a command without `async` runs on the main thread,
/// which is where a native menu has to be put up.
#[tauri::command]
pub fn show_context_menu(window: Window, state: ContextState) {
    if let Err(err) = menu::show_context(&window, &state) {
        eprintln!("draftpad: failed to open the context menu: {err}");
    }
}

/// Moves the menu bar's accelerators to the keys the preferences panel gave
/// those commands: an accelerator in muda's spelling for each item by id, or
/// `None` for an item left without a key. macOS only; there is no menu bar
/// anywhere else, and the page does not call this there.
///
/// Synchronous so that it runs on the main thread, where the menu lives.
#[tauri::command]
pub fn set_menu_shortcuts(app: AppHandle, shortcuts: HashMap<String, Option<String>>) {
    #[cfg(target_os = "macos")]
    menu::set_shortcuts(&app, &shortcuts);
    #[cfg(not(target_os = "macos"))]
    let _ = (app, shortcuts);
}
