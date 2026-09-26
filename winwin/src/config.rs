//! The config file: which shortcut puts the foreground window where.
//!
//! Nothing here touches Win32, so it is tested on every platform.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::hotkey::Hotkey;
use crate::layout::{Anchor, Placement};

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

fn shortcut(keys: &str, anchor: Anchor, width: &str, height: &str) -> Shortcut {
    let valid = "default shortcuts are valid";
    Shortcut {
        keys: keys.parse().expect(valid),
        placement: Placement {
            anchor,
            width: width.parse().expect(valid),
            height: height.parse().expect(valid),
        },
    }
}

/// Whether a width or height in the file is written in percent, as versions
/// before ratios wrote them.
fn has_percent(text: &str) -> bool {
    let Ok(table) = text.parse::<toml::Table>() else {
        return false;
    };
    let Some(toml::Value::Array(shortcuts)) = table.get("shortcut") else {
        return false;
    };
    shortcuts
        .iter()
        .filter_map(toml::Value::as_table)
        .flat_map(|s| [s.get("width"), s.get("height")])
        .flatten()
        .filter_map(toml::Value::as_str)
        .any(|v| v.trim().ends_with('%'))
}

impl Config {
    /// What a first run starts with.
    pub fn defaults() -> Config {
        Config {
            shortcuts: vec![
                shortcut("Ctrl+Alt+Left", Anchor::Left, "1/2", "1"),
                shortcut("Ctrl+Alt+Right", Anchor::Right, "1/2", "1"),
                shortcut("Ctrl+Alt+Up", Anchor::Top, "1", "1/2"),
                shortcut("Ctrl+Alt+Down", Anchor::Bottom, "1", "1/2"),
                shortcut("Ctrl+Alt+Enter", Anchor::Center, "1", "1"),
                shortcut("Ctrl+Alt+C", Anchor::Center, "3/5", "4/5"),
                shortcut("Ctrl+Alt+U", Anchor::TopLeft, "1/2", "1/2"),
                shortcut("Ctrl+Alt+I", Anchor::TopRight, "1/2", "1/2"),
                shortcut("Ctrl+Alt+J", Anchor::BottomLeft, "1/2", "1/2"),
                shortcut("Ctrl+Alt+K", Anchor::BottomRight, "1/2", "1/2"),
                shortcut("Ctrl+Alt+D", Anchor::Left, "1/3", "1"),
                shortcut("Ctrl+Alt+F", Anchor::Center, "1/3", "1"),
                shortcut("Ctrl+Alt+G", Anchor::Right, "1/3", "1"),
                shortcut("Ctrl+Alt+E", Anchor::Left, "2/3", "1"),
                shortcut("Ctrl+Alt+T", Anchor::Right, "2/3", "1"),
            ],
        }
    }

    /// Entries may share a shortcut; pressing it steps through them (see
    /// cycle.rs). Widths and heights in percent, as earlier versions wrote
    /// them, are read as ratios. Fields this version does not know, such as the `name` and
    /// `offset_x`/`offset_y` of earlier versions, are ignored.
    pub fn parse(text: &str, path: &Path) -> Result<Config, ConfigError> {
        toml::from_str(text).map_err(|e| ConfigError::Parse(path.into(), e.to_string()))
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("the config always serializes")
    }

    /// Reads the file, or writes the defaults there first when there is none,
    /// so that a first run leaves a file to find. A file that measures in
    /// percent is written back in ratios once it has been read.
    pub fn load_or_create(path: &Path) -> Result<Config, ConfigError> {
        match fs::read_to_string(path) {
            Ok(text) => {
                let config = Config::parse(&text, path)?;
                if !has_percent(&text) {
                    return Ok(config);
                }
                // Read back what was written, so that 33.333% is 1/3 in
                // memory as it now is in the file.
                config.save(path)?;
                Config::parse(&config.to_toml(), path)
            }
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
    use crate::layout::Ratio;

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::parse(text, Path::new("config.toml"))
    }

    #[test]
    fn defaults_are_valid_and_round_trip() {
        let config = Config::defaults();
        let text = config.to_toml();
        assert!(text.contains(r#"width = "2/3""#), "{text}");
        assert_eq!(parse(&text).unwrap(), config);
    }

    #[test]
    fn reads_a_hand_written_file() {
        let config = parse(
            r#"
            [[shortcut]]
            keys = "win+alt+1"
            anchor = "bottom-right"
            width = " 2 / 3 "
            height = "0.7"
            "#,
        )
        .unwrap();
        let s = &config.shortcuts[0];
        assert_eq!(s.keys.to_string(), "Win+Alt+1");
        assert_eq!(s.placement.anchor, Anchor::BottomRight);
        assert_eq!(s.placement.width, "2/3".parse().unwrap());
        assert_eq!(s.placement.height, Ratio::new(0.7).unwrap());
    }

    #[test]
    fn reads_percent_from_earlier_versions_as_ratios() {
        let config = parse(
            r#"
            [[shortcut]]
            keys = "Ctrl+Alt+E"
            anchor = "left"
            width = "66.667%"
            height = "100%"
            "#,
        )
        .unwrap();
        let p = &config.shortcuts[0].placement;
        assert_eq!(
            (p.width.to_string(), p.height.to_string()),
            ("2/3".into(), "1".into())
        );
    }

    #[test]
    fn refuses_pixels_and_sizes_outside_the_screen() {
        for width in ["800px", "800", "150%", "3/2", "0", "-1/2"] {
            let text = format!(
                "[[shortcut]]\nkeys = \"Ctrl+Alt+L\"\nanchor = \"left\"\nwidth = \"{width}\"\nheight = \"1\"\n"
            );
            let e = parse(&text).unwrap_err().to_string();
            assert!(e.contains(width), "{width}: {e}");
        }
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
            width = "1/2"
            height = "1"
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
            width = "1/2"
            height = "1"

            [[shortcut]]
            keys = "Ctrl+Shift+Left"
            anchor = "left"
            width = "2/3"
            height = "1"
        "#;
        let config = parse(text).unwrap();
        let widths: Vec<String> = config
            .shortcuts
            .iter()
            .map(|s| s.placement.width.to_string())
            .collect();
        assert_eq!(widths, ["1/2", "2/3"]);
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

    #[test]
    fn rewrites_a_file_in_percent_as_ratios() {
        let dir = std::env::temp_dir().join(format!("winwin-test-pct-{}", std::process::id()));
        let path = dir.join("config.toml");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let ratios = "[[shortcut]]\nkeys = \"Ctrl+Alt+L\"\nanchor = \"left\"\n# mine\nwidth = \"1/2\"\nheight = \"1\"\n";
        fs::write(&path, ratios).unwrap();
        Config::load_or_create(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), ratios, "left alone");

        fs::write(&path, ratios.replace("1/2", "33.333%")).unwrap();
        let config = Config::load_or_create(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(r#"width = "1/3""#), "{text}");
        assert_eq!(parse(&text).unwrap(), config);

        fs::remove_dir_all(&dir).unwrap();
    }
}
