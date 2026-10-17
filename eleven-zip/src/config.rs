//! The config file: what the settings window's switches are set to.
//!
//! Nothing here touches Win32, so it is tested on every platform.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Every field has a default, so a file that leaves one out, or an older
/// version's file, still reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Opens what was extracted in Explorer once extraction succeeds.
    pub open_in_explorer: bool,
    /// An archive holding a single folder or file extracts it next to the
    /// archive, rather than into a folder named after the archive that would
    /// then hold nothing but it.
    pub unwrap_single_item: bool,
    /// Puts the archive, or every volume of a split one, in the Recycle Bin once
    /// it has extracted without errors or warnings.
    pub trash_archive: bool,
    /// eleven-zip で展開 in the context menu of archives.
    pub context_menu: bool,
    /// Checks for a new 7-Zip once a day and installs it.
    pub auto_update: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            open_in_explorer: true,
            unwrap_single_item: true,
            trash_archive: false,
            context_menu: true,
            auto_update: true,
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(PathBuf, io::Error),
    Parse(PathBuf, String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(path, e) => {
                write!(
                    f,
                    "設定ファイルを読み書きできません: {}\n{e}",
                    path.display()
                )
            }
            ConfigError::Parse(path, e) => {
                write!(
                    f,
                    "設定ファイルの内容が正しくありません: {}\n{e}",
                    path.display()
                )
            }
        }
    }
}

impl Config {
    /// The file's settings, or the defaults while there is no file: it is
    /// written only once a setting is changed.
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        match fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|e| ConfigError::Parse(path.into(), e.to_string()))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(ConfigError::Io(path.into(), e)),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let io_err = |e| ConfigError::Io(path.into(), e);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(io_err)?;
        }
        let text = toml::to_string(self).expect("the config always serializes");
        fs::write(path, text).map_err(io_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("eleven-zip-config-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("config.toml")
    }

    #[test]
    fn everything_but_trashing_is_on_by_default() {
        let c = Config::default();
        assert!(c.open_in_explorer && c.unwrap_single_item && c.context_menu && c.auto_update);
        assert!(!c.trash_archive);
    }

    #[test]
    fn a_missing_file_reads_as_the_defaults_and_is_not_created() {
        let path = temp_path("missing");
        assert_eq!(Config::load(&path).unwrap(), Config::default());
        assert!(!path.exists());
    }

    #[test]
    fn round_trip() {
        let path = temp_path("round-trip");
        let c = Config {
            open_in_explorer: false,
            unwrap_single_item: true,
            trash_archive: true,
            context_menu: false,
            auto_update: false,
        };
        c.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), c);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn fields_left_out_take_their_defaults() {
        let c: Config = toml::from_str("auto_update = false\n").unwrap();
        assert_eq!(
            c,
            Config {
                auto_update: false,
                ..Config::default()
            }
        );
    }

    #[test]
    fn a_broken_file_is_an_error() {
        let path = temp_path("broken");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "open_in_explorer = \"yes\"\n").unwrap();
        assert!(matches!(Config::load(&path), Err(ConfigError::Parse(..))));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
