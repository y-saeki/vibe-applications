//! The config file: the gesture sets, the applications each one is for, and
//! for each gesture the command it stands for and the keys that carry the
//! command out in each application. Also what a gesture made over an
//! application comes to ([`Config::lookup`]).
//!
//! ```toml
//! os = "windows"
//!
//! [[common]]
//! name = "ウィンドウを閉じる"
//! gesture = "↓→"
//! keys = "Ctrl+W"
//!
//! [[sets]]
//! name = "ブラウザ"
//! apps = [
//!   { id = "chrome.exe", name = "Google Chrome" },
//!   { id = "firefox.exe", name = "Firefox" },
//! ]
//!
//! [[sets.commands]]
//! name = "戻る"
//! gesture = "←"
//! keys = "Alt+Left"
//! apps = { "firefox.exe" = "Alt+Left" }
//! ```
//!
//! An application is known by what the platform tells them apart by: the
//! executable's file name on Windows, the bundle identifier on macOS. Those
//! and the keys mean nothing on the other platform, so a file says which
//! one it is for and is not read on the other.
//!
//! On macOS a command can choose an item of the application's menu bar
//! instead of sending keys: `menu = ["ファイル", "閉じる"]` in place of
//! `keys`, or `{ menu = [...] }` in place of an application's keys.
//!
//! Nothing here touches the operating system, so it is tested on every
//! platform.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::gesture::Gesture;
use crate::keys::{Combo, Look};

/// The platform a config file is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Windows,
    Macos,
}

impl Os {
    /// The platform this build runs on. Every platform but macOS reads
    /// Windows files, so that the library's tests run anywhere.
    pub const CURRENT: Os = if cfg!(target_os = "macos") {
        Os::Macos
    } else {
        Os::Windows
    };

    pub fn label(self) -> &'static str {
        match self {
            Os::Windows => "Windows",
            Os::Macos => "macOS",
        }
    }

    pub fn look(self) -> Look {
        match self {
            Os::Windows => Look::Windows,
            Os::Macos => Look::Mac,
        }
    }
}

/// An application a set is for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct App {
    /// What the platform tells it apart by.
    pub id: String,
    /// What the settings window shows it as.
    pub name: String,
}

impl App {
    pub fn is(&self, id: &str) -> bool {
        self.id.eq_ignore_ascii_case(id)
    }
}

/// What a command does in an application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Send these keys.
    Keys(Combo),
    /// Choose the item of the menu bar with these titles, from the menu
    /// bar's item down. macOS only.
    Menu(Vec<String>),
}

impl Action {
    pub fn keys(&self) -> Option<Combo> {
        match self {
            Action::Keys(keys) => Some(*keys),
            Action::Menu(_) => None,
        }
    }

    pub fn menu(&self) -> Option<&[String]> {
        match self {
            Action::Keys(_) => None,
            Action::Menu(path) => Some(path),
        }
    }
}

/// A gesture and what it stands for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    pub gesture: Gesture,
    /// What an application of the set that has nothing of its own in
    /// `apps` does; every application, for a command of the common set.
    pub action: Option<Action>,
    /// What one application of the set does, by its id.
    pub apps: BTreeMap<String, Action>,
    /// The command of a preset this one was made from
    /// (`preset::Command::id`), which fills in the keys of an application
    /// added to the set.
    pub preset: Option<String>,
}

impl Command {
    /// What the application `id` does.
    pub fn action_for(&self, id: &str) -> Option<&Action> {
        self.apps
            .iter()
            .find(|(app, _)| app.eq_ignore_ascii_case(id))
            .map(|(_, a)| a)
            .or(self.action.as_ref())
    }
}

/// Gestures for some applications.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GestureSet {
    pub name: String,
    pub apps: Vec<App>,
    pub commands: Vec<Command>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub os: Os,
    /// Gestures for every application. A set's own gesture wins over one of
    /// these for the applications of the set.
    pub common: Vec<Command>,
    pub sets: Vec<GestureSet>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            os: Os::CURRENT,
            common: Vec::new(),
            sets: Vec::new(),
        }
    }
}

/// What a gesture made over an application comes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Send `keys` to the application: the gesture is `command`.
    Send { command: String, keys: Combo },
    /// Choose the application's menu item `path`: the gesture is `command`.
    Menu { command: String, path: Vec<String> },
    /// The gesture is `command`, which has no keys for this application.
    Unassigned { command: String },
    /// No command has this gesture.
    Unknown,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(PathBuf, io::Error),
    Parse(PathBuf, String),
    /// The file is for the other platform.
    OtherOs(PathBuf, Os),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(path, e) => write!(f, "{}: {e}", path.display()),
            ConfigError::Parse(path, e) => write!(f, "{} を読めません。\n{e}", path.display()),
            ConfigError::OtherOs(path, os) => write!(
                f,
                "{} は {} 用の設定です。{} では読み込めません。",
                path.display(),
                os.label(),
                Os::CURRENT.label()
            ),
        }
    }
}

// The file as it is written: names and keys as text, checked when it is
// turned into a `Config`.

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    os: Os,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    common: Vec<FileCommand>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sets: Vec<FileSet>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileSet {
    name: String,
    #[serde(default)]
    apps: Vec<App>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    commands: Vec<FileCommand>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileCommand {
    name: String,
    gesture: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    keys: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    menu: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    apps: BTreeMap<String, FileAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preset: Option<String>,
}

/// What one application does: keys as text, or a menu item.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum FileAction {
    Keys(String),
    Menu { menu: Vec<String> },
}

/// Reads keys or a menu item, `at` saying where they are for a message.
/// Menu items are for macOS only.
fn read_action(
    keys: Option<String>,
    menu: Option<Vec<String>>,
    os: Os,
    at: &str,
) -> Result<Option<Action>, String> {
    match (keys, menu) {
        (None, None) => Ok(None),
        (Some(_), Some(_)) => Err(format!("{at}: キーとメニュー項目の両方があります")),
        (Some(keys), None) => keys
            .parse()
            .map(|k| Some(Action::Keys(k)))
            .map_err(|e| format!("{at}のキー: {e}")),
        (None, Some(path)) => {
            if os != Os::Macos {
                return Err(format!(
                    "{at}: メニュー項目は {} では使えません",
                    os.label()
                ));
            }
            if path.is_empty() || path.iter().any(|t| t.trim().is_empty()) {
                return Err(format!("{at}のメニュー項目に空の名前があります"));
            }
            Ok(Some(Action::Menu(path)))
        }
    }
}

fn write_action(action: Option<&Action>, look: Look) -> (Option<String>, Option<Vec<String>>) {
    match action {
        None => (None, None),
        Some(Action::Keys(k)) => (Some(k.text(look)), None),
        Some(Action::Menu(path)) => (None, Some(path.clone())),
    }
}

impl FileCommand {
    fn read(self, look_up: &str, os: Os) -> Result<Command, String> {
        let at = format!("{look_up}「{}」", self.name);
        let gesture = self
            .gesture
            .parse()
            .map_err(|e| format!("{at}のジェスチャー: {e}"))?;
        let action = read_action(self.keys, self.menu, os, &at)?;
        let apps = self
            .apps
            .into_iter()
            .map(|(app, a)| {
                let (keys, menu) = match a {
                    FileAction::Keys(keys) => (Some(keys), None),
                    FileAction::Menu { menu } => (None, Some(menu)),
                };
                let action = read_action(keys, menu, os, &format!("{at}（{app}）"))?;
                Ok(action.map(|a| (app, a)))
            })
            .filter_map(Result::transpose)
            .collect::<Result<_, String>>()?;
        Ok(Command {
            name: self.name,
            gesture,
            action,
            apps,
            preset: self.preset,
        })
    }

    fn write(c: &Command, look: Look) -> FileCommand {
        let (keys, menu) = write_action(c.action.as_ref(), look);
        FileCommand {
            name: c.name.clone(),
            gesture: c.gesture.to_string(),
            keys,
            menu,
            apps: c
                .apps
                .iter()
                .map(|(app, a)| {
                    let a = match a {
                        Action::Keys(k) => FileAction::Keys(k.text(look)),
                        Action::Menu(path) => FileAction::Menu { menu: path.clone() },
                    };
                    (app.clone(), a)
                })
                .collect(),
            preset: c.preset.clone(),
        }
    }
}

/// Where the problem a check finds lies, for the settings window to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// The common set when `set` is `None`.
    Set {
        set: Option<usize>,
    },
    Command {
        set: Option<usize>,
        command: usize,
    },
}

/// What is wrong with a config, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub place: Place,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

pub const COMMON_NAME: &str = "共通";

impl Config {
    /// The set's name as the settings window and its messages show it.
    pub fn set_name(&self, set: Option<usize>) -> &str {
        match set {
            None => COMMON_NAME,
            Some(i) => self.sets.get(i).map_or("", |s| s.name.as_str()),
        }
    }

    pub fn commands(&self, set: Option<usize>) -> &[Command] {
        match set {
            None => &self.common,
            Some(i) => self.sets.get(i).map_or(&[], |s| s.commands.as_slice()),
        }
    }

    pub fn commands_mut(&mut self, set: Option<usize>) -> Option<&mut Vec<Command>> {
        match set {
            None => Some(&mut self.common),
            Some(i) => self.sets.get_mut(i).map(|s| &mut s.commands),
        }
    }

    /// The set the application `id` belongs to.
    pub fn set_of(&self, id: &str) -> Option<usize> {
        self.sets
            .iter()
            .position(|s| s.apps.iter().any(|a| a.is(id)))
    }

    /// Whether a right-button press over the application `id` is followed
    /// as a gesture. Over an application no gesture is for, the button is
    /// left alone.
    pub fn follows(&self, id: &str) -> bool {
        !self.common.is_empty()
            || self
                .set_of(id)
                .is_some_and(|i| !self.sets[i].commands.is_empty())
    }

    /// What `gesture` made over the application `id` comes to: the
    /// command of the application's set with that gesture, or else the
    /// common one.
    pub fn lookup(&self, id: &str, gesture: &Gesture) -> Outcome {
        let own = self.set_of(id).map(|i| &self.sets[i].commands);
        let command = own
            .into_iter()
            .chain([&self.common])
            .flat_map(|commands| commands.iter())
            .find(|c| c.gesture == *gesture);
        match command {
            Some(c) => match c.action_for(id) {
                Some(Action::Keys(keys)) => Outcome::Send {
                    command: c.name.clone(),
                    keys: *keys,
                },
                Some(Action::Menu(path)) => Outcome::Menu {
                    command: c.name.clone(),
                    path: path.clone(),
                },
                None => Outcome::Unassigned {
                    command: c.name.clone(),
                },
            },
            None => Outcome::Unknown,
        }
    }

    /// The first thing that keeps this config from being saved: a set or
    /// a command without a name, two sets of one name, an application in
    /// two sets, a command without a gesture, or two commands of a set
    /// with one gesture.
    pub fn check(&self) -> Result<(), Problem> {
        let problem = |place, message| Err(Problem { place, message });
        for (i, set) in self.sets.iter().enumerate() {
            let place = Place::Set { set: Some(i) };
            if set.name.trim().is_empty() {
                return problem(place, "名前のないセットがあります".into());
            }
            if set.name.trim() == COMMON_NAME
                || self.sets[..i]
                    .iter()
                    .any(|s| s.name.trim() == set.name.trim())
            {
                return problem(
                    place,
                    format!("セット「{}」と同じ名前のセットがあります", set.name),
                );
            }
            for app in &set.apps {
                if let Some(other) = self.sets[..i]
                    .iter()
                    .find(|s| s.apps.iter().any(|a| a.is(&app.id)))
                {
                    return problem(
                        place,
                        format!(
                            "{} がセット「{}」と「{}」の両方に入っています",
                            app.name, other.name, set.name
                        ),
                    );
                }
            }
        }
        let sets = std::iter::once(None).chain((0..self.sets.len()).map(Some));
        for set in sets {
            let name = self.set_name(set);
            let commands = self.commands(set);
            for (i, c) in commands.iter().enumerate() {
                let place = Place::Command { set, command: i };
                if c.name.trim().is_empty() {
                    return problem(
                        place,
                        format!("セット「{name}」に名前のないコマンドがあります"),
                    );
                }
                if c.gesture.is_empty() {
                    return problem(
                        place,
                        format!(
                            "セット「{name}」のコマンド「{}」にジェスチャーがありません",
                            c.name
                        ),
                    );
                }
                if let Some(other) = commands[..i].iter().find(|o| o.gesture == c.gesture) {
                    return problem(
                        place,
                        format!(
                            "セット「{name}」の「{}」と「{}」が同じジェスチャー {} です",
                            other.name, c.name, c.gesture
                        ),
                    );
                }
            }
        }
        Ok(())
    }

    fn from_file(file: File) -> Result<Config, String> {
        let common = file
            .common
            .into_iter()
            .map(|c| c.read(&format!("セット「{COMMON_NAME}」のコマンド"), file.os))
            .collect::<Result<_, _>>()?;
        let sets = file
            .sets
            .into_iter()
            .map(|s| {
                let look_up = format!("セット「{}」のコマンド", s.name);
                Ok(GestureSet {
                    commands: s
                        .commands
                        .into_iter()
                        .map(|c| c.read(&look_up, file.os))
                        .collect::<Result<_, String>>()?,
                    name: s.name,
                    apps: s.apps,
                })
            })
            .collect::<Result<_, String>>()?;
        let config = Config {
            os: file.os,
            common,
            sets,
        };
        config.check().map_err(|p| p.message)?;
        Ok(config)
    }

    /// Reads a file's text. A file for the other platform is refused.
    pub fn parse(text: &str, path: &Path) -> Result<Config, ConfigError> {
        Config::parse_on(text, path, Os::CURRENT)
    }

    /// Reads a file's text as the platform `os` does, so that the tests
    /// read what macOS reads on any platform.
    fn parse_on(text: &str, path: &Path, os: Os) -> Result<Config, ConfigError> {
        let file: File =
            toml::from_str(text).map_err(|e| ConfigError::Parse(path.into(), e.to_string()))?;
        if file.os != os {
            return Err(ConfigError::OtherOs(path.into(), file.os));
        }
        Config::from_file(file).map_err(|e| ConfigError::Parse(path.into(), e))
    }

    pub fn to_toml(&self) -> String {
        let look = self.os.look();
        let file = File {
            os: self.os,
            common: self
                .common
                .iter()
                .map(|c| FileCommand::write(c, look))
                .collect(),
            sets: self
                .sets
                .iter()
                .map(|s| FileSet {
                    name: s.name.clone(),
                    apps: s.apps.clone(),
                    commands: s
                        .commands
                        .iter()
                        .map(|c| FileCommand::write(c, look))
                        .collect(),
                })
                .collect(),
        };
        toml::to_string(&file).expect("the config always serializes")
    }

    /// Reads the config, writing an empty one first when there is none.
    pub fn load_or_create(path: &Path) -> Result<Config, ConfigError> {
        match fs::read_to_string(path) {
            Ok(text) => Config::parse(&text, path),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let config = Config::default();
                config.save(path)?;
                Ok(config)
            }
            Err(e) => Err(ConfigError::Io(path.into(), e)),
        }
    }

    /// Reads a file to import, writing nothing.
    pub fn read(path: &Path) -> Result<Config, ConfigError> {
        let text = fs::read_to_string(path).map_err(|e| ConfigError::Io(path.into(), e))?;
        Config::parse(&text, path)
    }

    /// Writes the file whole: to a file beside it first, then in its place,
    /// so that a failure leaves the old one.
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

    fn combo(s: &str) -> Combo {
        s.parse().unwrap()
    }

    fn g(s: &str) -> Gesture {
        s.parse().unwrap()
    }

    /// The example in the module's documentation, for this platform.
    fn example() -> String {
        let os = match Os::CURRENT {
            Os::Windows => "windows",
            Os::Macos => "macos",
        };
        format!(
            r#"
os = "{os}"

[[common]]
name = "ウィンドウを閉じる"
gesture = "↓→"
keys = "Ctrl+W"

[[sets]]
name = "ブラウザ"
apps = [
  {{ id = "chrome.exe", name = "Google Chrome" }},
  {{ id = "firefox.exe", name = "Firefox" }},
]

[[sets.commands]]
name = "戻る"
gesture = "←"
keys = "Alt+Left"
apps = {{ "firefox.exe" = "Alt+Left" }}

[[sets.commands]]
name = "右側のタブを閉じる"
gesture = "↓→"
"#
        )
    }

    #[test]
    fn reads_a_hand_written_file() {
        let config = parse(&example()).unwrap();
        assert_eq!(config.os, Os::CURRENT);
        assert_eq!(config.common.len(), 1);
        assert_eq!(config.common[0].action, Some(Action::Keys(combo("Ctrl+W"))));
        let set = &config.sets[0];
        assert_eq!(set.name, "ブラウザ");
        assert_eq!(set.apps[1].name, "Firefox");
        assert_eq!(set.commands[0].gesture, g("←"));
        assert_eq!(
            set.commands[0].apps["firefox.exe"],
            Action::Keys(combo("Alt+Left"))
        );
        assert_eq!(set.commands[1].action, None);
    }

    #[test]
    fn writes_what_it_reads() {
        let config = parse(&example()).unwrap();
        assert_eq!(parse(&config.to_toml()).unwrap(), config);
        let empty = Config::default();
        assert_eq!(parse(&empty.to_toml()).unwrap(), empty);
        // The preset a command was made from is kept.
        let mut config = config;
        config.sets[0].commands[0].preset = Some("back".into());
        let text = config.to_toml();
        assert!(text.contains(r#"preset = "back""#));
        assert_eq!(parse(&text).unwrap(), config);
    }

    #[test]
    fn writes_the_keys_with_the_platforms_names() {
        let mut config = Config::default();
        config.common.push(Command {
            name: "閉じる".into(),
            gesture: g("↓→"),
            action: Some(Action::Keys(combo("Cmd+Option+W"))),
            apps: BTreeMap::new(),
            preset: None,
        });
        config.os = Os::Macos;
        assert!(config.to_toml().contains(r#"keys = "Option+Cmd+W""#));
        config.os = Os::Windows;
        assert!(config.to_toml().contains(r#"keys = "Win+Alt+W""#));
    }

    #[test]
    fn refuses_a_file_for_the_other_platform() {
        let other = match Os::CURRENT {
            Os::Windows => "macos",
            Os::Macos => "windows",
        };
        let text = example().replace(
            &format!("os = \"{}\"", Os::CURRENT.label().to_lowercase()),
            &format!("os = \"{other}\""),
        );
        assert!(matches!(parse(&text), Err(ConfigError::OtherOs(..))));
        // A file that does not say is not read either.
        assert!(matches!(
            parse("[[sets]]\nname = \"a\""),
            Err(ConfigError::Parse(..))
        ));
    }

    #[test]
    fn says_what_is_wrong_and_where() {
        let message = |text: String| match parse(&text) {
            Err(ConfigError::Parse(_, e)) => e,
            other => panic!("{other:?}"),
        };
        assert!(
            message(example().replace("\"Alt+Left\"\napps", "\"Alt+Foo\"\napps"))
                .contains("セット「ブラウザ」のコマンド「戻る」のキー")
        );
        assert!(
            message(example().replace("gesture = \"←\"", "gesture = \"←←\"")).contains("同じ方向")
        );
        assert!(
            message(example().replace("gesture = \"←\"", "gesture = \"↓→\""))
                .contains("同じジェスチャー")
        );
    }

    fn parse_mac(text: &str) -> Result<Config, ConfigError> {
        Config::parse_on(text, Path::new("config.toml"), Os::Macos)
    }

    const MENUS: &str = r#"
os = "macos"

[[common]]
name = "ウィンドウを閉じる"
gesture = "↓→"
menu = ["ファイル", "ウインドウを閉じる"]

[[sets]]
name = "ブラウザ"
apps = [
  { id = "com.google.Chrome", name = "Google Chrome" },
  { id = "com.apple.Safari", name = "Safari" },
]

[[sets.commands]]
name = "右側のタブを閉じる"
gesture = "↓→"
keys = "Cmd+W"
apps = { "com.google.Chrome" = { menu = ["タブ", "右側のタブを閉じる"] } }
"#;

    #[test]
    fn reads_and_writes_menu_items_on_macos() {
        let config = parse_mac(MENUS).unwrap();
        let path = |p: &[&str]| p.iter().map(|t| t.to_string()).collect::<Vec<_>>();
        assert_eq!(
            config.common[0].action,
            Some(Action::Menu(path(&["ファイル", "ウインドウを閉じる"])))
        );
        let c = &config.sets[0].commands[0];
        assert_eq!(c.action, Some(Action::Keys(combo("Cmd+W"))));
        assert_eq!(
            c.apps["com.google.Chrome"],
            Action::Menu(path(&["タブ", "右側のタブを閉じる"]))
        );
        assert_eq!(parse_mac(&config.to_toml()).unwrap(), config);

        // The application's menu item, the set's keys, the common menu item.
        assert_eq!(
            config.lookup("com.google.Chrome", &g("↓→")),
            Outcome::Menu {
                command: "右側のタブを閉じる".into(),
                path: path(&["タブ", "右側のタブを閉じる"])
            }
        );
        assert_eq!(
            config.lookup("com.apple.Safari", &g("↓→")),
            Outcome::Send {
                command: "右側のタブを閉じる".into(),
                keys: combo("Cmd+W")
            }
        );
        assert_eq!(
            config.lookup("com.apple.TextEdit", &g("↓→")),
            Outcome::Menu {
                command: "ウィンドウを閉じる".into(),
                path: path(&["ファイル", "ウインドウを閉じる"])
            }
        );
    }

    #[test]
    fn menu_items_are_for_macos_only() {
        let message = |r: Result<Config, ConfigError>| match r {
            Err(ConfigError::Parse(_, e)) => e,
            other => panic!("{other:?}"),
        };
        let windows = MENUS
            .replace("os = \"macos\"", "os = \"windows\"")
            .replace("Cmd+W", "Ctrl+W");
        let windows = Config::parse_on(&windows, Path::new("config.toml"), Os::Windows);
        assert!(message(windows).contains("メニュー項目は Windows では使えません"));
        let both = MENUS.replace(
            "gesture = \"↓→\"\nmenu",
            "gesture = \"↓→\"\nkeys = \"Cmd+W\"\nmenu",
        );
        assert!(message(parse_mac(&both)).contains("両方"));
        let empty = MENUS.replace(
            "[\"ファイル\", \"ウインドウを閉じる\"]",
            "[\"ファイル\", \" \"]",
        );
        assert!(message(parse_mac(&empty)).contains("空の名前"));
        assert!(
            message(parse_mac(&MENUS.replace(
                "menu = [\"タブ\", \"右側のタブを閉じる\"]",
                "menu = []"
            )))
            .contains("空の名前")
        );
    }

    #[test]
    fn checks_what_the_settings_window_saves() {
        let mut config = parse(&example()).unwrap();
        assert_eq!(config.check(), Ok(()));

        let mut c = config.clone();
        c.sets[0].name = " ".into();
        assert_eq!(c.check().unwrap_err().place, Place::Set { set: Some(0) });

        let mut c = config.clone();
        c.sets.push(GestureSet {
            name: "ブラウザ".into(),
            ..Default::default()
        });
        assert!(c.check().unwrap_err().message.contains("同じ名前"));

        let mut c = config.clone();
        c.sets.push(GestureSet {
            name: "その他".into(),
            apps: vec![App {
                id: "CHROME.EXE".into(),
                name: "Chrome".into(),
            }],
            ..Default::default()
        });
        let p = c.check().unwrap_err();
        assert_eq!(p.place, Place::Set { set: Some(1) });
        assert!(p.message.contains("両方"));

        let mut c = config.clone();
        c.common[0].gesture = Gesture::default();
        assert_eq!(
            c.check().unwrap_err().place,
            Place::Command {
                set: None,
                command: 0
            }
        );

        config.sets[0].commands[1].name = String::new();
        assert_eq!(
            config.check().unwrap_err().place,
            Place::Command {
                set: Some(0),
                command: 1
            }
        );
    }

    #[test]
    fn a_sets_own_gesture_wins_over_a_common_one() {
        let config = parse(&example()).unwrap();
        // ↓→ is the set's, which has no keys for Chrome.
        assert_eq!(
            config.lookup("chrome.exe", &g("↓→")),
            Outcome::Unassigned {
                command: "右側のタブを閉じる".into()
            }
        );
        // Elsewhere it is the common one.
        assert_eq!(
            config.lookup("notepad.exe", &g("↓→")),
            Outcome::Send {
                command: "ウィンドウを閉じる".into(),
                keys: combo("Ctrl+W")
            }
        );
        assert_eq!(config.lookup("notepad.exe", &g("←")), Outcome::Unknown);
    }

    #[test]
    fn an_application_has_its_own_keys_or_the_sets() {
        let mut config = parse(&example()).unwrap();
        config.sets[0].commands[0]
            .apps
            .insert("firefox.exe".into(), Action::Keys(combo("Ctrl+OemComma")));
        let back = |app| config.lookup(app, &g("←"));
        assert_eq!(
            back("FIREFOX.EXE"),
            Outcome::Send {
                command: "戻る".into(),
                keys: combo("Ctrl+OemComma")
            }
        );
        assert_eq!(
            back("chrome.exe"),
            Outcome::Send {
                command: "戻る".into(),
                keys: combo("Alt+Left")
            }
        );
    }

    #[test]
    fn follows_the_button_only_where_a_gesture_is_for() {
        let mut config = parse(&example()).unwrap();
        assert!(config.follows("notepad.exe"));
        config.common.clear();
        assert!(config.follows("chrome.exe"));
        assert!(!config.follows("notepad.exe"));
        config.sets[0].commands.clear();
        assert!(!config.follows("chrome.exe"));
    }

    #[test]
    fn saves_and_creates_the_file() {
        let dir = std::env::temp_dir().join(format!("meta-gesture-test-{}", std::process::id()));
        let path = dir.join("sub").join("config.toml");
        let created = Config::load_or_create(&path).unwrap();
        assert_eq!(created, Config::default());
        let config = parse(&example()).unwrap();
        config.save(&path).unwrap();
        assert_eq!(Config::read(&path).unwrap(), config);
        assert!(!path.with_extension("toml.tmp").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
