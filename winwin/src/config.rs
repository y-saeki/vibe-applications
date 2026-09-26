//! The config file: which shortcut puts the foreground window where.
//!
//! Nothing here touches Win32, so it is tested on every platform.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::hotkey::Hotkey;
use crate::layout::{Anchor, Length, Placement};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shortcut {
    #[serde(with = "hotkey_string")]
    pub keys: Hotkey,
    #[serde(flatten)]
    pub placement: Placement,
}

// Hotkey is kept free of serde; the file spells it the way Display writes it.
mod hotkey_string {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    use crate::hotkey::Hotkey;

    pub fn serialize<S: Serializer>(h: &Hotkey, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&h.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Hotkey, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, rename = "shortcut")]
    pub shortcuts: Vec<Shortcut>,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(PathBuf, io::Error),
    Parse(PathBuf, String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(path, e) => write!(f, "{}: {e}", path.display()),
            ConfigError::Parse(path, e) => write!(f, "{} を読めません。\n{e}", path.display()),
        }
    }
}

fn shortcut(keys: &str, anchor: Anchor, width: f64, height: f64) -> Shortcut {
    Shortcut {
        keys: keys.parse().expect("default shortcuts are valid"),
        placement: Placement {
            anchor,
            width: Length::Percent(width),
            height: Length::Percent(height),
        },
    }
}

impl Config {
    /// What a first run starts with.
    pub fn defaults() -> Config {
        const THIRD: f64 = 100.0 / 3.0;
        Config {
            shortcuts: vec![
                shortcut("Ctrl+Alt+Left", Anchor::Left, 50.0, 100.0),
                shortcut("Ctrl+Alt+Right", Anchor::Right, 50.0, 100.0),
                shortcut("Ctrl+Alt+Up", Anchor::Top, 100.0, 50.0),
                shortcut("Ctrl+Alt+Down", Anchor::Bottom, 100.0, 50.0),
                shortcut("Ctrl+Alt+Enter", Anchor::Center, 100.0, 100.0),
                shortcut("Ctrl+Alt+C", Anchor::Center, 60.0, 80.0),
                shortcut("Ctrl+Alt+U", Anchor::TopLeft, 50.0, 50.0),
                shortcut("Ctrl+Alt+I", Anchor::TopRight, 50.0, 50.0),
                shortcut("Ctrl+Alt+J", Anchor::BottomLeft, 50.0, 50.0),
                shortcut("Ctrl+Alt+K", Anchor::BottomRight, 50.0, 50.0),
                shortcut("Ctrl+Alt+D", Anchor::Left, THIRD, 100.0),
                shortcut("Ctrl+Alt+F", Anchor::Center, THIRD, 100.0),
                shortcut("Ctrl+Alt+G", Anchor::Right, THIRD, 100.0),
                shortcut("Ctrl+Alt+E", Anchor::Left, 2.0 * THIRD, 100.0),
                shortcut("Ctrl+Alt+T", Anchor::Right, 2.0 * THIRD, 100.0),
            ],
        }
    }

    /// Entries may share a shortcut; pressing it steps through them (see
    /// cycle.rs). Fields this version does not know, such as the `name` and
    /// `offset_x`/`offset_y` of earlier versions, are ignored.
    pub fn parse(text: &str, path: &Path) -> Result<Config, ConfigError> {
        toml::from_str(text).map_err(|e| ConfigError::Parse(path.into(), e.to_string()))
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("the config always serializes")
    }

    /// Reads the file, or writes the defaults there first when there is none,
    /// so that a first run leaves a file to find.
    pub fn load_or_create(path: &Path) -> Result<Config, ConfigError> {
        match fs::read_to_string(path) {
            Ok(text) => Config::parse(&text, path),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let config = Config::defaults();
                config.save(path)?;
                Ok(config)
            }
            Err(e) => Err(ConfigError::Io(path.into(), e)),
        }
    }

    /// Writes through a temporary file, so that a crash halfway leaves the
    /// previous config rather than half of the new one.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let io_err = |e| ConfigError::Io(path.into(), e);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(io_err)?;
        }
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, self.to_toml()).map_err(io_err)?;
        fs::rename(&tmp, path).map_err(io_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::parse(text, Path::new("config.toml"))
    }

    #[test]
    fn defaults_are_valid_and_round_trip() {
        let config = Config::defaults();
        let reread = parse(&config.to_toml()).unwrap();
        assert_eq!(reread.shortcuts.len(), config.shortcuts.len());
        for (a, b) in reread.shortcuts.iter().zip(&config.shortcuts) {
            assert_eq!(a.keys, b.keys);
            assert_eq!(a.placement.anchor, b.placement.anchor);
            // Written with three decimals, so a third comes back a hair off.
            let close = |x: Length, y: Length| (x.value() - y.value()).abs() < 0.001;
            assert!(close(a.placement.width, b.placement.width));
            assert!(close(a.placement.height, b.placement.height));
        }
    }

    #[test]
    fn reads_a_hand_written_file() {
        let config = parse(
            r#"
            [[shortcut]]
            keys = "win+alt+1"
            anchor = "bottom-right"
            width = "1280px"
            height = "70%"
            "#,
        )
        .unwrap();
        let s = &config.shortcuts[0];
        assert_eq!(s.keys.to_string(), "Win+Alt+1");
        assert_eq!(s.placement.anchor, Anchor::BottomRight);
        assert_eq!(s.placement.width, Length::Pixels(1280.0));
        assert_eq!(s.placement.height, Length::Percent(70.0));
    }

    #[test]
    fn reads_a_file_from_before_names_and_offsets_were_dropped() {
        let config = parse(
            r#"
            [[shortcut]]
            name = "右下"
            keys = "Ctrl+Alt+K"
            anchor = "bottom-right"
            width = "50%"
            height = "50%"
            offset_x = "-16px"
            offset_y = "-16px"
            "#,
        )
        .unwrap();
        assert_eq!(config.shortcuts[0].keys.to_string(), "Ctrl+Alt+K");
        let text = config.to_toml();
        assert!(!text.contains("name"), "{text}");
        assert!(!text.contains("offset"), "{text}");
    }

    #[test]
    fn an_empty_file_has_no_shortcuts() {
        assert_eq!(parse("").unwrap().shortcuts.len(), 0);
    }

    #[test]
    fn rejects_bad_values_with_the_reason() {
        let bad_keys = r#"
            [[shortcut]]
            keys = "Ctrl+Alt+Nope"
            anchor = "left"
            width = "50%"
            height = "100%"
        "#;
        let e = parse(bad_keys).unwrap_err().to_string();
        assert!(e.contains("Nope"), "{e}");

        let bad_anchor = bad_keys
            .replace("Ctrl+Alt+Nope", "Ctrl+Alt+L")
            .replace("\"left\"", "\"middle\"");
        assert!(matches!(parse(&bad_anchor), Err(ConfigError::Parse(..))));
    }

    #[test]
    fn accepts_a_shortcut_used_twice_and_keeps_the_order() {
        let text = r#"
            [[shortcut]]
            keys = "Ctrl+Shift+Left"
            anchor = "left"
            width = "50%"
            height = "100%"

            [[shortcut]]
            keys = "Ctrl+Shift+Left"
            anchor = "left"
            width = "66.667%"
            height = "100%"
        "#;
        let config = parse(text).unwrap();
        let widths: Vec<String> = config
            .shortcuts
            .iter()
            .map(|s| s.placement.width.to_string())
            .collect();
        assert_eq!(widths, ["50%", "66.667%"]);
        assert_eq!(parse(&config.to_toml()).unwrap(), config);
    }

    #[test]
    fn saves_and_creates_the_file() {
        let dir = std::env::temp_dir().join(format!("winwin-test-{}", std::process::id()));
        let path = dir.join("nested").join("config.toml");
        let _ = fs::remove_dir_all(&dir);

        let created = Config::load_or_create(&path).unwrap();
        assert!(path.exists());
        assert_eq!(created.shortcuts.len(), Config::defaults().shortcuts.len());

        let mut changed = created.clone();
        changed.shortcuts.truncate(1);
        changed.save(&path).unwrap();
        assert_eq!(Config::load_or_create(&path).unwrap().shortcuts.len(), 1);
        assert!(!path.with_extension("toml.tmp").exists());

        fs::remove_dir_all(&dir).unwrap();
    }
}
