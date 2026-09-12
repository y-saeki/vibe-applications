//! Commands invoked from the webview (`invoke(...)` in `src/state.ts` and
//! `src/preferences.ts`).

use serde::Serialize;
use tauri::AppHandle;

use crate::fonts;
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
}

#[cfg(target_os = "windows")]
fn started_for_preferences() -> bool {
    crate::jumplist::started_for_preferences()
}

#[cfg(not(target_os = "windows"))]
fn started_for_preferences() -> bool {
    false
}

#[tauri::command]
pub fn load_state(app: AppHandle) -> Result<Loaded, String> {
    let path = state::path(&app).map_err(|err| err.to_string())?;
    let state = state::load(&path).map_err(|err| err.to_string())?;
    Ok(Loaded {
        state,
        platform: std::env::consts::OS,
        version: app.package_info().version.to_string(),
        open_preferences: started_for_preferences(),
    })
}

/// Async so the file write happens off the main thread.
#[tauri::command]
pub async fn save_state(app: AppHandle, state: State) -> Result<(), String> {
    let path = state::path(&app).map_err(|err| err.to_string())?;
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
