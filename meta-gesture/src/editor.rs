//! What the settings window edits: the config as it stands in the window,
//! which set and which command are selected, and the edits themselves. The
//! Windows window (`src/win/settings_ui.rs`) uses it directly, the macOS one
//! through `ffi.rs`. Nothing is checked until the window saves.
//!
//! The window lists the sets with the common set first: row 0 is the common
//! set, row `i + 1` is `config.sets[i]`.
//!
//! Nothing here touches the operating system, so it is tested on every
//! platform.

use crate::config::{Action, App, COMMON_NAME, Command, Config, GestureSet, Place, Problem};
use crate::gesture::{Direction, Gesture};
use crate::preset::{self, Preset};

/// A line of the list of sets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetRow {
    pub name: String,
    /// The applications, as the list shows them under the name.
    pub apps: String,
    pub commands: usize,
}

/// A line of the list of commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandRow {
    pub name: String,
    pub gesture: String,
}

/// A line of the selected command's keys: what the applications that have
/// nothing of their own do (`app` is `None`), or what one application does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyRow {
    pub app: Option<App>,
    pub label: String,
    /// The keys or menu item set on this line.
    pub action: Option<Action>,
}

/// The label of the line of keys for every application of a set.
pub const SET_KEYS_LABEL: &str = "セットの既定";
/// The same for the common set, whose keys go to every application.
pub const COMMON_KEYS_LABEL: &str = "すべてのアプリ";

#[derive(Clone, Debug)]
pub struct Editor {
    config: Config,
    saved: Config,
    /// The selected set: `None` is the common set.
    set: Option<usize>,
    command: Option<usize>,
}

impl Editor {
    pub fn new(config: Config) -> Editor {
        Editor {
            saved: config.clone(),
            config,
            set: None,
            command: None,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn is_changed(&self) -> bool {
        self.config != self.saved
    }

    /// Puts an imported config in place of what is being edited. Saving is
    /// still needed for it to take effect.
    pub fn replace(&mut self, config: Config) {
        self.config = config;
        self.set = None;
        self.command = None;
    }

    // Sets

    pub fn set_rows(&self) -> Vec<SetRow> {
        let common = SetRow {
            name: COMMON_NAME.into(),
            apps: "すべてのアプリ".into(),
            commands: self.config.common.len(),
        };
        let sets = self.config.sets.iter().map(|s| SetRow {
            name: s.name.clone(),
            apps: if s.apps.is_empty() {
                "アプリなし".into()
            } else {
                s.apps
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join("、")
            },
            commands: s.commands.len(),
        });
        std::iter::once(common).chain(sets).collect()
    }

    /// The selected line of the list of sets.
    pub fn selected_set_row(&self) -> usize {
        self.set.map_or(0, |i| i + 1)
    }

    pub fn is_common(&self) -> bool {
        self.set.is_none()
    }

    pub fn select_set_row(&mut self, row: usize) {
        let set = row.checked_sub(1).filter(|&i| i < self.config.sets.len());
        if row != 0 && set.is_none() {
            return;
        }
        if set != self.set {
            self.set = set;
            self.command = None;
        }
    }

    pub fn set_name(&self) -> &str {
        self.config.set_name(self.set)
    }

    /// Adds a set after the others and selects it.
    pub fn add_set(&mut self) {
        let name = self.unique(
            "新しいセット",
            self.config.sets.iter().map(|s| s.name.as_str()),
        );
        self.config.sets.push(GestureSet {
            name,
            ..Default::default()
        });
        self.set = Some(self.config.sets.len() - 1);
        self.command = None;
    }

    /// Deletes the selected set. The common set stays.
    pub fn delete_set(&mut self) {
        let Some(i) = self.set else {
            return;
        };
        self.config.sets.remove(i);
        self.set = (!self.config.sets.is_empty()).then(|| i.min(self.config.sets.len() - 1));
        self.command = None;
    }

    pub fn rename_set(&mut self, name: &str) {
        if let Some(set) = self.set.and_then(|i| self.config.sets.get_mut(i)) {
            set.name = name.to_string();
        }
    }

    pub fn can_move_set(&self, up: bool) -> bool {
        match self.set {
            None => false,
            Some(i) if up => i > 0,
            Some(i) => i + 1 < self.config.sets.len(),
        }
    }

    pub fn move_set(&mut self, up: bool) {
        if !self.can_move_set(up) {
            return;
        }
        let i = self.set.unwrap_or_default();
        let j = if up { i - 1 } else { i + 1 };
        self.config.sets.swap(i, j);
        self.set = Some(j);
    }

    // Applications

    /// The selected set's applications; none for the common set.
    pub fn apps(&self) -> &[App] {
        self.set
            .and_then(|i| self.config.sets.get(i))
            .map_or(&[], |s| s.apps.as_slice())
    }

    /// Adds `app` to the selected set. An application is in one set at
    /// most: one in another set is refused, with why.
    pub fn add_app(&mut self, app: App) -> Result<(), String> {
        let Some(i) = self.set else {
            return Ok(());
        };
        if let Some(other) = self.config.set_of(&app.id) {
            if other == i {
                return Ok(());
            }
            return Err(format!(
                "{} はセット「{}」に入っています",
                app.name, self.config.sets[other].name
            ));
        }
        fill_in(&mut self.config.sets[i], self.config.os, &app);
        self.config.sets[i].apps.push(app);
        Ok(())
    }

    /// Adds a set made from `preset` after the others and selects it: its
    /// commands, and the applications of `running` it knows that are in no
    /// set yet, with their keys.
    pub fn add_preset(&mut self, preset: &Preset, running: &[App]) {
        let name = self.unique(
            preset.name,
            self.config.sets.iter().map(|s| s.name.as_str()),
        );
        let mut set = GestureSet {
            name,
            apps: Vec::new(),
            commands: preset
                .commands
                .iter()
                .map(|c| Command {
                    name: c.name.into(),
                    gesture: c.gesture(),
                    preset: Some(c.id.into()),
                    ..Default::default()
                })
                .collect(),
        };
        for app in running {
            if preset.application(app).is_some()
                && self.config.set_of(&app.id).is_none()
                && !set.apps.iter().any(|a| a.is(&app.id))
            {
                fill_in(&mut set, self.config.os, app);
                set.apps.push(app.clone());
            }
        }
        self.config.sets.push(set);
        self.set = Some(self.config.sets.len() - 1);
        self.command = None;
    }

    /// Takes the application at `index` out of the selected set, with the
    /// keys its commands had for it.
    pub fn remove_app(&mut self, index: usize) {
        let Some(set) = self.set.and_then(|i| self.config.sets.get_mut(i)) else {
            return;
        };
        if index >= set.apps.len() {
            return;
        }
        let app = set.apps.remove(index);
        for c in &mut set.commands {
            c.apps.retain(|id, _| !app.is(id));
        }
    }

    // Commands

    pub fn command_rows(&self) -> Vec<CommandRow> {
        self.config
            .commands(self.set)
            .iter()
            .map(|c| CommandRow {
                name: c.name.clone(),
                gesture: c.gesture.to_string(),
            })
            .collect()
    }

    pub fn selected_command_row(&self) -> Option<usize> {
        self.command
    }

    pub fn select_command(&mut self, index: Option<usize>) {
        self.command = index.filter(|&i| i < self.config.commands(self.set).len());
    }

    pub fn command(&self) -> Option<&Command> {
        self.command
            .and_then(|i| self.config.commands(self.set).get(i))
    }

    fn command_mut(&mut self) -> Option<&mut Command> {
        let i = self.command?;
        self.config.commands_mut(self.set)?.get_mut(i)
    }

    /// Adds a command after the others and selects it.
    pub fn add_command(&mut self) {
        let name = self.unique(
            "新しいコマンド",
            self.config
                .commands(self.set)
                .iter()
                .map(|c| c.name.as_str()),
        );
        let Some(commands) = self.config.commands_mut(self.set) else {
            return;
        };
        commands.push(Command {
            name,
            ..Default::default()
        });
        self.command = Some(commands.len() - 1);
    }

    pub fn delete_command(&mut self) {
        let Some(i) = self.command else {
            return;
        };
        let Some(commands) = self.config.commands_mut(self.set) else {
            return;
        };
        if i < commands.len() {
            commands.remove(i);
        }
        self.command = (!commands.is_empty()).then(|| i.min(commands.len() - 1));
    }

    pub fn can_move_command(&self, up: bool) -> bool {
        let len = self.config.commands(self.set).len();
        match self.command {
            None => false,
            Some(i) if up => i > 0,
            Some(i) => i + 1 < len,
        }
    }

    pub fn move_command(&mut self, up: bool) {
        if !self.can_move_command(up) {
            return;
        }
        let i = self.command.unwrap_or_default();
        let j = if up { i - 1 } else { i + 1 };
        if let Some(commands) = self.config.commands_mut(self.set) {
            commands.swap(i, j);
            self.command = Some(j);
        }
    }

    pub fn rename_command(&mut self, name: &str) {
        if let Some(c) = self.command_mut() {
            c.name = name.to_string();
        }
    }

    /// Adds a stroke to the selected command's gesture, unless it repeats
    /// the last one or the gesture is full.
    pub fn push_direction(&mut self, d: Direction) {
        if let Some(c) = self.command_mut() {
            c.gesture.push(d);
        }
    }

    pub fn pop_direction(&mut self) {
        if let Some(c) = self.command_mut() {
            c.gesture.pop();
        }
    }

    pub fn clear_gesture(&mut self) {
        if let Some(c) = self.command_mut() {
            c.gesture = Gesture::default();
        }
    }

    /// The other command of the set with the selected one's gesture, which
    /// the window points out as soon as it happens.
    pub fn gesture_conflict(&self) -> Option<String> {
        let i = self.command?;
        let commands = self.config.commands(self.set);
        let c = commands.get(i)?;
        if c.gesture.is_empty() {
            return None;
        }
        commands
            .iter()
            .enumerate()
            .find(|(j, o)| *j != i && o.gesture == c.gesture)
            .map(|(_, o)| format!("「{}」と同じジェスチャーです", o.name))
    }

    /// The selected command's lines of keys: the set's, then one for each
    /// of its applications.
    pub fn key_rows(&self) -> Vec<KeyRow> {
        let Some(c) = self.command() else {
            return Vec::new();
        };
        let first = KeyRow {
            app: None,
            label: if self.is_common() {
                COMMON_KEYS_LABEL
            } else {
                SET_KEYS_LABEL
            }
            .into(),
            action: c.action.clone(),
        };
        let apps = self.apps().iter().map(|a| KeyRow {
            app: Some(a.clone()),
            label: a.name.clone(),
            action: c
                .apps
                .iter()
                .find(|(id, _)| a.is(id))
                .map(|(_, action)| action.clone()),
        });
        std::iter::once(first).chain(apps).collect()
    }

    /// Sets the keys or menu item of the selected command's line `row` of
    /// [`Editor::key_rows`]; `None` clears them.
    pub fn set_action(&mut self, row: usize, action: Option<Action>) {
        let app = match row {
            0 => None,
            n => match self.apps().get(n - 1) {
                Some(a) => Some(a.id.clone()),
                None => return,
            },
        };
        let Some(c) = self.command_mut() else {
            return;
        };
        match app {
            None => c.action = action,
            Some(id) => {
                c.apps.retain(|app, _| !app.eq_ignore_ascii_case(&id));
                if let Some(a) = action {
                    c.apps.insert(id, a);
                }
            }
        }
    }

    /// The config to save, or what keeps it from being saved. The set or
    /// command the problem is in is selected, so that the window shows it.
    pub fn check(&mut self) -> Result<Config, Problem> {
        match self.config.check() {
            Ok(()) => Ok(self.config.clone()),
            Err(problem) => {
                match problem.place {
                    Place::Set { set } => {
                        self.set = set;
                        self.command = None;
                    }
                    Place::Command { set, command } => {
                        self.set = set;
                        self.command = Some(command);
                    }
                }
                Err(problem)
            }
        }
    }

    /// The config has been saved as it stands.
    pub fn saved(&mut self) {
        self.saved = self.config.clone();
    }

    /// `base`, or `base 2`, `base 3` and so on: the first that none of
    /// `taken` is.
    fn unique<'a>(&self, base: &str, taken: impl Iterator<Item = &'a str> + Clone) -> String {
        (1..)
            .map(|n| {
                if n == 1 {
                    base.to_string()
                } else {
                    format!("{base} {n}")
                }
            })
            .find(|name| !taken.clone().any(|t| t == name))
            .unwrap_or_default()
    }
}

/// Gives `app` the keys its preset knows for each command of `set` made
/// from a preset, where it has none of its own yet.
fn fill_in(set: &mut GestureSet, os: crate::config::Os, app: &App) {
    for c in &mut set.commands {
        let Some((preset, command)) = c.preset.as_deref().and_then(preset::command) else {
            continue;
        };
        let Some(keys) = preset.application(app).and_then(|a| a.keys(os, command.id)) else {
            continue;
        };
        if !c.apps.keys().any(|id| app.is(id)) {
            c.apps.insert(app.id.clone(), Action::Keys(keys));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, name: &str) -> App {
        App {
            id: id.into(),
            name: name.into(),
        }
    }

    fn keys(s: &str) -> Action {
        Action::Keys(s.parse().unwrap())
    }

    /// A set "ブラウザ" with Chrome and Firefox, and a command 戻る ← with
    /// keys for the set.
    fn browsers() -> Editor {
        let mut e = Editor::new(Config::default());
        e.add_set();
        e.rename_set("ブラウザ");
        e.add_app(app("chrome.exe", "Chrome")).unwrap();
        e.add_app(app("firefox.exe", "Firefox")).unwrap();
        e.add_command();
        e.rename_command("戻る");
        e.push_direction(Direction::Left);
        e.set_action(0, Some(keys("Alt+Left")));
        e
    }

    #[test]
    fn lists_the_common_set_first() {
        let e = browsers();
        let rows = e.set_rows();
        assert_eq!(rows[0].name, "共通");
        assert_eq!(rows[1].name, "ブラウザ");
        assert_eq!(rows[1].apps, "Chrome、Firefox");
        assert_eq!(rows[1].commands, 1);
        assert_eq!(e.selected_set_row(), 1);
        assert!(e.is_changed());
    }

    #[test]
    fn adds_sets_and_commands_under_names_not_taken() {
        let mut e = Editor::new(Config::default());
        e.add_set();
        e.add_set();
        assert_eq!(e.set_name(), "新しいセット 2");
        e.add_command();
        e.add_command();
        assert_eq!(e.command().unwrap().name, "新しいコマンド 2");
        assert_eq!(e.selected_command_row(), Some(1));
    }

    #[test]
    fn selecting_another_set_lets_go_of_the_command() {
        let mut e = browsers();
        assert_eq!(e.selected_command_row(), Some(0));
        e.select_set_row(0);
        assert!(e.is_common());
        assert_eq!(e.selected_command_row(), None);
        assert!(e.apps().is_empty());
        // A line that is not there changes nothing.
        e.select_set_row(5);
        assert!(e.is_common());
    }

    #[test]
    fn an_application_is_in_one_set() {
        let mut e = browsers();
        // Again in the same set: nothing happens.
        assert_eq!(e.add_app(app("CHROME.EXE", "Chrome")), Ok(()));
        assert_eq!(e.apps().len(), 2);
        e.add_set();
        assert_eq!(
            e.add_app(app("chrome.exe", "Chrome")),
            Err("Chrome はセット「ブラウザ」に入っています".into())
        );
    }

    #[test]
    fn removing_an_application_takes_its_keys() {
        let mut e = browsers();
        e.set_action(2, Some(keys("Ctrl+OemComma")));
        assert_eq!(e.key_rows()[2].action, Some(keys("Ctrl+OemComma")));
        e.remove_app(1);
        assert_eq!(e.apps().len(), 1);
        assert!(e.command().unwrap().apps.is_empty());
    }

    #[test]
    fn shows_the_keys_of_the_set_and_of_each_application() {
        let mut e = browsers();
        e.set_action(1, Some(keys("Ctrl+OemComma")));
        let rows = e.key_rows();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].label, SET_KEYS_LABEL);
        assert_eq!(rows[0].action, Some(keys("Alt+Left")));
        assert_eq!(rows[1].label, "Chrome");
        assert_eq!(rows[1].action, Some(keys("Ctrl+OemComma")));
        assert_eq!(rows[2].action, None);
        e.set_action(1, None);
        assert!(e.command().unwrap().apps.is_empty());
        e.select_set_row(0);
        e.add_command();
        assert_eq!(e.key_rows().len(), 1);
        assert_eq!(e.key_rows()[0].label, COMMON_KEYS_LABEL);
    }

    #[test]
    fn a_line_chooses_a_menu_item_in_place_of_keys() {
        let mut e = browsers();
        let menu = Action::Menu(vec!["タブ".into(), "右側のタブを閉じる".into()]);
        e.set_action(1, Some(menu.clone()));
        assert_eq!(e.key_rows()[1].action, Some(menu.clone()));
        assert_eq!(e.command().unwrap().apps["chrome.exe"], menu);
        // Keys put back in its place replace it.
        e.set_action(1, Some(keys("Ctrl+W")));
        assert_eq!(e.key_rows()[1].action, Some(keys("Ctrl+W")));
        e.set_action(0, Some(menu.clone()));
        assert_eq!(e.key_rows()[0].action, Some(menu));
        assert!(e.check().is_ok());
    }

    #[test]
    fn edits_the_gesture_stroke_by_stroke() {
        let mut e = browsers();
        e.clear_gesture();
        e.push_direction(Direction::Down);
        e.push_direction(Direction::Down);
        e.push_direction(Direction::Right);
        assert_eq!(e.command_rows()[0].gesture, "↓→");
        e.pop_direction();
        assert_eq!(e.command_rows()[0].gesture, "↓");
    }

    #[test]
    fn points_out_a_gesture_used_twice() {
        let mut e = browsers();
        e.add_command();
        assert_eq!(e.gesture_conflict(), None);
        e.push_direction(Direction::Left);
        assert_eq!(
            e.gesture_conflict(),
            Some("「戻る」と同じジェスチャーです".into())
        );
    }

    #[test]
    fn moves_and_deletes() {
        let mut e = browsers();
        e.add_command();
        e.rename_command("進む");
        assert!(e.can_move_command(true));
        assert!(!e.can_move_command(false));
        e.move_command(true);
        assert_eq!(e.command_rows()[0].name, "進む");
        assert_eq!(e.selected_command_row(), Some(0));
        e.delete_command();
        assert_eq!(e.command().unwrap().name, "戻る");
        e.add_set();
        assert!(e.can_move_set(true));
        e.move_set(true);
        assert_eq!(e.set_rows()[1].name, "新しいセット");
        e.delete_set();
        assert_eq!(e.set_name(), "ブラウザ");
        e.delete_set();
        assert!(e.is_common());
        // The common set cannot be deleted.
        e.delete_set();
        assert!(e.is_common());
    }

    #[test]
    fn a_failed_check_selects_where_the_problem_is() {
        let mut e = browsers();
        e.add_command();
        e.select_set_row(0);
        let problem = e.check().unwrap_err();
        assert!(problem.message.contains("ジェスチャーがありません"));
        assert_eq!(e.selected_set_row(), 1);
        assert_eq!(e.selected_command_row(), Some(1));
        e.delete_command();
        let config = e.check().unwrap();
        assert!(e.is_changed());
        e.saved();
        assert!(!e.is_changed());
        assert_eq!(config.sets[0].commands.len(), 1);
    }

    #[test]
    fn an_import_replaces_everything_until_saved() {
        let mut e = browsers();
        e.saved();
        e.replace(Config::default());
        assert!(e.is_changed());
        assert!(e.is_common());
        assert_eq!(e.set_rows().len(), 1);
    }

    #[test]
    fn a_preset_makes_a_set_with_the_running_applications_it_knows() {
        let mut e = Editor::new(Config::default());
        let running = [
            app("chrome.exe", "Google Chrome"),
            app("notepad.exe", "メモ帳"),
            app("msedge.exe", "Microsoft Edge"),
        ];
        e.add_preset(&preset::BROWSERS, &running);
        assert_eq!(e.set_name(), "ブラウザ");
        assert_eq!(e.apps().len(), 2);
        let rows = e.command_rows();
        assert_eq!(rows.len(), preset::BROWSERS.commands.len());
        assert_eq!(rows[0].name, "戻る");
        assert_eq!(rows[0].gesture, "←");
        e.select_command(Some(0));
        let os = e.config().os;
        let rows = e.key_rows();
        // The set has no keys of its own; each browser has its own.
        assert_eq!(rows[0].action, None);
        assert!(rows[1].action.is_some());
        assert_eq!(
            rows[1].action,
            preset::BROWSERS.applications[0]
                .keys(os, "back")
                .map(Action::Keys)
        );
        // It saves as it is, and a second one takes another name and no
        // application already in the first.
        assert!(e.check().is_ok());
        e.add_preset(&preset::BROWSERS, &running);
        assert_eq!(e.set_name(), "ブラウザ 2");
        assert!(e.apps().is_empty());
    }

    #[test]
    fn an_application_added_to_a_preset_set_gets_its_keys() {
        let mut e = Editor::new(Config::default());
        e.add_preset(&preset::BROWSERS, &[]);
        e.add_app(app("chrome.exe", "Google Chrome")).unwrap();
        e.add_app(app("notepad.exe", "メモ帳")).unwrap();
        e.select_command(Some(0));
        let rows = e.key_rows();
        assert_eq!(
            rows[1].action,
            preset::BROWSERS.applications[0]
                .keys(e.config().os, "back")
                .map(Action::Keys)
        );
        assert_eq!(rows[2].action, None);
        // Keys set by hand are not filled over.
        e.set_action(1, Some(keys("F1")));
        let mut config = e.config().clone();
        fill_in(
            &mut config.sets[0],
            config.os,
            &app("chrome.exe", "Google Chrome"),
        );
        assert_eq!(config.sets[0].commands[0].apps["chrome.exe"], keys("F1"));
        // A command made by hand has no preset to fill in from.
        e.add_command();
        e.add_app(app("msedge.exe", "Microsoft Edge")).unwrap();
        assert!(e.command().unwrap().apps.is_empty());
    }
}
