//! Persistent application state: the draft text plus every user setting,
//! stored as one `state.json` in the platform app-data directory.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const FILE_NAME: &str = "state.json";

/// Mirrors `State` in `src/state.ts`. Every field has a default so that a
/// file written by an older or newer version still loads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub text: String,
    pub language: String,
    pub editor_mode: String,
    pub theme: String,
    pub font_size: u32,
    pub font_family: String,
    pub tab_size: u32,
    pub quick_suggestions: bool,
    pub always_on_top: bool,
    pub window_width: Option<f64>,
    pub window_height: Option<f64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            text: String::new(),
            language: "markdown".into(),
            editor_mode: "normal".into(),
            theme: "system".into(),
            font_size: 13,
            font_family: String::new(),
            tab_size: 4,
            quick_suggestions: true,
            always_on_top: false,
            window_width: None,
            window_height: None,
        }
    }
}

/// `<app data dir>/state.json`.
pub fn path(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join(FILE_NAME))
}

/// Loads the state file. A missing file yields the defaults. A file that
/// cannot be parsed is moved aside as `state.json.broken` so the draft is
/// never silently discarded, and the defaults are returned.
pub fn load(path: &Path) -> io::Result<State> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(State::default()),
        Err(err) => return Err(err),
    };
    match serde_json::from_str(&raw) {
        Ok(state) => Ok(state),
        Err(err) => {
            let broken = path.with_extension("json.broken");
            eprintln!(
                "draftpad: {} could not be parsed ({err}); moving it to {}",
                path.display(),
                broken.display()
            );
            fs::rename(path, &broken)?;
            Ok(State::default())
        }
    }
}

/// Writes the state atomically: serialize into a sibling temp file, fsync it,
/// then rename it over the target. A crash mid-write therefore leaves either
/// the previous file or the new one, never a truncated draft.
pub fn save(path: &Path, state: &State) -> io::Result<()> {
    let dir = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "state path has no parent directory",
        )
    })?;
    fs::create_dir_all(dir)?;
    let json = serde_json::to_vec_pretty(state)?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(&json)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}
