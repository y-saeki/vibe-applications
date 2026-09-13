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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    /// A directory under the system temp directory that deletes itself, so the
    /// tests need no temp-file crate.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let dir = std::env::temp_dir().join(format!(
                "draftpad-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).expect("the system temp directory is writable");
            Self(dir)
        }

        fn state(&self) -> PathBuf {
            self.0.join(FILE_NAME)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_missing_file_loads_as_the_defaults() {
        let dir = TempDir::new();

        let state = load(&dir.state()).expect("a missing file is not an error");

        assert!(state.text.is_empty());
        assert_eq!(state.language, "markdown");
        assert_eq!(state.font_size, 13);
        assert_eq!(state.tab_size, 4);
        assert!(state.quick_suggestions);
        assert!(!state.always_on_top);
        assert_eq!(state.window_width, None);
    }

    #[test]
    fn saving_and_loading_round_trips() {
        let dir = TempDir::new();
        let written = State {
            text: "書きかけの下書き\n🌏".into(),
            language: "rust".into(),
            editor_mode: "vim".into(),
            theme: "dark".into(),
            font_size: 24,
            font_family: "BIZ UDGothic".into(),
            tab_size: 2,
            quick_suggestions: false,
            always_on_top: true,
            window_width: Some(800.0),
            window_height: Some(600.0),
        };

        save(&dir.state(), &written).expect("the state is written");
        let read = load(&dir.state()).expect("the state is read back");

        assert_eq!(read.text, written.text);
        assert_eq!(read.language, "rust");
        assert_eq!(read.editor_mode, "vim");
        assert_eq!(read.theme, "dark");
        assert_eq!(read.font_size, 24);
        assert_eq!(read.font_family, "BIZ UDGothic");
        assert_eq!(read.tab_size, 2);
        assert!(!read.quick_suggestions);
        assert!(read.always_on_top);
        assert_eq!(read.window_width, Some(800.0));
        assert_eq!(read.window_height, Some(600.0));
    }

    #[test]
    fn a_field_an_older_version_never_wrote_falls_back_to_its_default() {
        let dir = TempDir::new();
        fs::write(dir.state(), r#"{"text":"昔のファイル"}"#).expect("the file is written");

        let state = load(&dir.state()).expect("a partial file still loads");

        assert_eq!(state.text, "昔のファイル");
        assert_eq!(state.language, "markdown");
        assert_eq!(state.font_size, 13);
    }

    #[test]
    fn a_file_that_cannot_be_parsed_is_moved_aside_rather_than_discarded() {
        let dir = TempDir::new();
        fs::write(dir.state(), "{ これは JSON ではない").expect("the file is written");

        let state = load(&dir.state()).expect("a broken file loads as the defaults");

        assert!(state.text.is_empty());
        assert!(!dir.state().exists());
        let broken = fs::read_to_string(dir.state().with_extension("json.broken"))
            .expect("the unparseable file is kept");
        assert_eq!(broken, "{ これは JSON ではない");
    }

    #[test]
    fn saving_creates_the_directory_and_leaves_no_temporary_file_behind() {
        let dir = TempDir::new();
        let path = dir
            .0
            .join("not")
            .join("created")
            .join("yet")
            .join(FILE_NAME);

        save(&path, &State::default()).expect("the directory is created on the way");

        assert!(path.exists());
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn saving_over_an_existing_file_replaces_it() {
        let dir = TempDir::new();
        let first = State {
            text: "一回目".into(),
            ..State::default()
        };
        let second = State {
            text: "二回目".into(),
            ..State::default()
        };

        save(&dir.state(), &first).expect("the first write succeeds");
        save(&dir.state(), &second).expect("the second write succeeds");

        assert_eq!(
            load(&dir.state()).expect("the state is read back").text,
            "二回目"
        );
    }
}
