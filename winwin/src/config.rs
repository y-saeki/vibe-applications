//! The config file: which shortcut puts the foreground window where.
//!
//! Nothing here touches Win32, so it is tested on every platform.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::hotkey::Hotkey;
use crate::layout::{Anchor, Length, Placement};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shortcut {
    pub name: String,
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
    Invalid(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(path, e) => write!(f, "{}: {e}", path.display()),
            ConfigError::Parse(path, e) => write!(f, "{} を読めません。\n{e}", path.display()),
            ConfigError::Invalid(e) => f.write_str(e),
        }
    }
}

fn shortcut(name: &str, keys: &str, anchor: Anchor, width: f64, height: f64) -> Shortcut {
    Shortcut {
        name: name.into(),
        keys: keys.parse().expect("default shortcuts are valid"),
        placement: Placement {
            anchor,
            width: Length::Percent(width),
            height: Length::Percent(height),
            offset_x: Length::ZERO,
            offset_y: Length::ZERO,
        },
    }
}

impl Config {
    /// What a first run starts with.
    pub fn defaults() -> Config {
        const THIRD: f64 = 100.0 / 3.0;
        Config {
            shortcuts: vec![
                shortcut("左半分", "Ctrl+Alt+Left", Anchor::Left, 50.0, 100.0),
                shortcut("右半分", "Ctrl+Alt+Right", Anchor::Right, 50.0, 100.0),
                shortcut("上半分", "Ctrl+Alt+Up", Anchor::Top, 100.0, 50.0),
                shortcut("下半分", "Ctrl+Alt+Down", Anchor::Bottom, 100.0, 50.0),
                shortcut(
                    "画面いっぱい",
                    "Ctrl+Alt+Enter",
                    Anchor::Center,
                    100.0,
                    100.0,
                ),
                shortcut("中央", "Ctrl+Alt+C", Anchor::Center, 60.0, 80.0),
                shortcut("左上", "Ctrl+Alt+U", Anchor::TopLeft, 50.0, 50.0),
                shortcut("右上", "Ctrl+Alt+I", Anchor::TopRight, 50.0, 50.0),
                shortcut("左下", "Ctrl+Alt+J", Anchor::BottomLeft, 50.0, 50.0),
                shortcut("右下", "Ctrl+Alt+K", Anchor::BottomRight, 50.0, 50.0),
                shortcut("左 1/3", "Ctrl+Alt+D", Anchor::Left, THIRD, 100.0),
                shortcut("中央 1/3", "Ctrl+Alt+F", Anchor::Center, THIRD, 100.0),
                shortcut("右 1/3", "Ctrl+Alt+G", Anchor::Right, THIRD, 100.0),
                shortcut("左 2/3", "Ctrl+Alt+E", Anchor::Left, 2.0 * THIRD, 100.0),
                shortcut("右 2/3", "Ctrl+Alt+T", Anchor::Right, 2.0 * THIRD, 100.0),
            ],
        }
    }

    /// Everything a config must satisfy beyond parsing: one action per
    /// shortcut, and a name to show for each.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let mut seen: HashMap<Hotkey, &str> = HashMap::new();
        for s in &self.shortcuts {
            if s.name.trim().is_empty() {
                return Err(ConfigError::Invalid(format!(
                    "{} に名前がありません",
                    s.keys
                )));
            }
            if let Some(other) = seen.insert(s.keys, &s.name) {
                return Err(ConfigError::Invalid(format!(
                    "「{other}」と「{}」が同じショートカット {} を使っています",
                    s.name, s.keys
                )));
            }
        }
        Ok(())
    }

    pub fn parse(text: &str, path: &Path) -> Result<Config, ConfigError> {
        let config: Config =
            toml::from_str(text).map_err(|e| ConfigError::Parse(path.into(), e.to_string()))?;
        config.validate()?;
        Ok(config)
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
        self.validate()?;
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
        config.validate().unwrap();
        let reread = parse(&config.to_toml()).unwrap();
        assert_eq!(reread.shortcuts.len(), config.shortcuts.len());
        for (a, b) in reread.shortcuts.iter().zip(&config.shortcuts) {
            assert_eq!(a.name, b.name);
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
            name = "作業用"
            keys = "win+alt+1"
            anchor = "bottom-right"
            width = "1280px"
            height = "70%"
            offset_x = "-16px"
            "#,
        )
        .unwrap();
        let s = &config.shortcuts[0];
        assert_eq!(s.keys.to_string(), "Win+Alt+1");
        assert_eq!(s.placement.anchor, Anchor::BottomRight);
        assert_eq!(s.placement.width, Length::Pixels(1280.0));
        assert_eq!(s.placement.height, Length::Percent(70.0));
        assert_eq!(s.placement.offset_x, Length::Pixels(-16.0));
        assert_eq!(s.placement.offset_y, Length::ZERO);
    }

    #[test]
    fn leaves_zero_offsets_out_of_the_file() {
        let text = Config::defaults().to_toml();
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
            name = "x"
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
    fn rejects_a_shortcut_used_twice() {
        let mut config = Config::defaults();
        config.shortcuts[1].keys = config.shortcuts[0].keys;
        let e = config.validate().unwrap_err().to_string();
        assert!(e.contains("左半分") && e.contains("右半分"), "{e}");
    }

    #[test]
    fn rejects_a_nameless_shortcut() {
        let mut config = Config::defaults();
        config.shortcuts[0].name = "  ".into();
        assert!(config.validate().is_err());
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
