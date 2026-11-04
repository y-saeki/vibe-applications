//! The library as the macOS app (`macos/`) calls it, through the Swift
//! bindings UniFFI writes from this file. The app passes on what the system
//! tells it, such as a press of the right button or a key going down, and
//! does what comes back; what that is is decided here, as the rest of the
//! library decides it for `src/win/`.
//!
//! Built with the `ffi` feature only.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::config::{App, Config, Outcome};
use crate::editor::Editor;
use crate::gesture::{Direction, Tracker};
use crate::keys::{Look, Press, Recorder};
use crate::{art, mac, preset};

/// A lock that is never poisoned: a panic aborts the process.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().expect("the state is never poisoned")
}

/// What a press of the right button came to.
#[derive(Debug, PartialEq, uniffi::Enum)]
pub enum GestureResult {
    /// The pointer hardly moved: give the application its click back.
    Click,
    /// It moved, but drew no stroke, or too many: do nothing.
    Nothing,
    /// The gesture is `command`: send the key `code` with the modifier
    /// `flags` to the application.
    Send {
        command: String,
        code: u16,
        flags: u64,
    },
    /// The gesture is `command`, which has no keys for this application.
    Unassigned { command: String },
    /// The gesture is `command`, whose key a Mac keyboard does not have.
    Unsendable { command: String },
    /// No command has the gesture `gesture`.
    Unknown { gesture: String },
}

/// The resident part's state: the config, and the press of the right button
/// being followed.
#[derive(Default, uniffi::Object)]
pub struct Resident {
    state: Mutex<ResidentState>,
}

#[derive(Default)]
struct ResidentState {
    config: Config,
    press: Option<(String, Tracker)>,
}

#[uniffi::export]
impl Resident {
    #[uniffi::constructor]
    pub fn new() -> Resident {
        Resident::default()
    }

    /// Reads the config at `path`, writing an empty one first when there is
    /// none. Why it could not be read, keeping no gestures then.
    pub fn load(&self, path: String) -> Option<String> {
        let (config, problem) = match Config::load_or_create(Path::new(&path)) {
            Ok(config) => (config, None),
            Err(e) => (Config::default(), Some(e.to_string())),
        };
        lock(&self.state).config = config;
        problem
    }

    /// The right button went down at (`x`, `y`), in points, over the
    /// application `app` (its bundle identifier). True when a gesture is
    /// for that application: the press is then followed, and kept from it.
    pub fn begin(&self, app: String, x: f64, y: f64) -> bool {
        let mut state = lock(&self.state);
        if !state.config.follows(&app) {
            state.press = None;
            return false;
        }
        state.press = Some((app, Tracker::new((x, y), 1.0)));
        true
    }

    /// The pointer moved while the button is held.
    pub fn moved(&self, x: f64, y: f64) {
        if let Some((_, tracker)) = lock(&self.state).press.as_mut() {
            tracker.move_to((x, y));
        }
    }

    /// Whether a press is being followed, whose button coming up is
    /// [`Resident::ended`]'s.
    pub fn is_following(&self) -> bool {
        lock(&self.state).press.is_some()
    }

    /// The button came up.
    pub fn ended(&self) -> GestureResult {
        let mut state = lock(&self.state);
        let Some((app, tracker)) = state.press.take() else {
            return GestureResult::Nothing;
        };
        if !tracker.is_gesture() {
            return GestureResult::Click;
        }
        let Some(gesture) = tracker.gesture() else {
            return GestureResult::Nothing;
        };
        match state.config.lookup(&app, &gesture) {
            Outcome::Send { command, keys } => match mac::code(keys.key) {
                Some(code) => GestureResult::Send {
                    command,
                    code,
                    flags: mac::flags(keys.modifiers),
                },
                None => GestureResult::Unsendable { command },
            },
            Outcome::Unassigned { command } => GestureResult::Unassigned { command },
            Outcome::Unknown => GestureResult::Unknown {
                gesture: gesture.to_string(),
            },
        }
    }
}

/// A line of the list of sets.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct SetEntry {
    pub name: String,
    pub apps: String,
}

/// A line of the list of commands.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct CommandEntry {
    pub name: String,
    pub gesture: String,
}

/// An application of the selected set.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
}

/// A line of the selected command's keys.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct KeyEntry {
    pub label: String,
    /// The keys as caps, in a Mac's order and symbols; empty when none are
    /// set on the line.
    pub caps: Vec<String>,
}

/// The settings window's state: the config being edited and what is
/// selected. Line numbers are those of the lists the window shows.
#[derive(uniffi::Object)]
pub struct SettingsEditor {
    editor: Mutex<Editor>,
}

impl SettingsEditor {
    fn editor(&self) -> MutexGuard<'_, Editor> {
        lock(&self.editor)
    }
}

fn index(i: u32) -> usize {
    i as usize
}

#[uniffi::export]
impl SettingsEditor {
    /// The config at `path`, or why it could not be read.
    #[uniffi::constructor]
    pub fn open(path: String) -> Result<Arc<SettingsEditor>, SettingsError> {
        let config =
            Config::load_or_create(Path::new(&path)).map_err(|e| SettingsError::Failed {
                message: e.to_string(),
            })?;
        Ok(Arc::new(SettingsEditor {
            editor: Mutex::new(Editor::new(config)),
        }))
    }

    pub fn set_lines(&self) -> Vec<SetEntry> {
        self.editor()
            .set_rows()
            .into_iter()
            .map(|r| SetEntry {
                name: r.name,
                apps: r.apps,
            })
            .collect()
    }

    pub fn selected_set(&self) -> u32 {
        self.editor().selected_set_row() as u32
    }

    pub fn select_set(&self, line: u32) {
        self.editor().select_set_row(index(line));
    }

    pub fn is_common(&self) -> bool {
        self.editor().is_common()
    }

    pub fn add_set(&self) {
        self.editor().add_set();
    }

    pub fn delete_set(&self) {
        self.editor().delete_set();
    }

    pub fn can_move_set(&self, up: bool) -> bool {
        self.editor().can_move_set(up)
    }

    pub fn move_set(&self, up: bool) {
        self.editor().move_set(up);
    }

    pub fn set_name(&self) -> String {
        self.editor().set_name().to_string()
    }

    pub fn rename_set(&self, name: String) {
        self.editor().rename_set(&name);
    }

    pub fn apps(&self) -> Vec<AppEntry> {
        self.editor()
            .apps()
            .iter()
            .map(|a| AppEntry {
                id: a.id.clone(),
                name: a.name.clone(),
            })
            .collect()
    }

    /// Adds an application to the selected set, or says why it cannot be.
    pub fn add_app(&self, id: String, name: String) -> Option<String> {
        self.editor().add_app(App { id, name }).err()
    }

    /// The presets that `add_preset` takes the number of, by name.
    pub fn presets(&self) -> Vec<String> {
        preset::ALL.iter().map(|p| p.name.to_string()).collect()
    }

    /// Adds a set made from preset `index`, with the applications of
    /// `running` it knows.
    pub fn add_preset(&self, index: u32, running: Vec<AppEntry>) {
        let Some(p) = preset::ALL.get(index as usize) else {
            return;
        };
        let running: Vec<App> = running
            .into_iter()
            .map(|a| App {
                id: a.id,
                name: a.name,
            })
            .collect();
        self.editor().add_preset(p, &running);
    }

    pub fn remove_app(&self, line: u32) {
        self.editor().remove_app(index(line));
    }

    pub fn command_lines(&self) -> Vec<CommandEntry> {
        self.editor()
            .command_rows()
            .into_iter()
            .map(|r| CommandEntry {
                name: r.name,
                gesture: r.gesture,
            })
            .collect()
    }

    pub fn selected_command(&self) -> Option<u32> {
        self.editor().selected_command_row().map(|i| i as u32)
    }

    pub fn select_command(&self, line: Option<u32>) {
        self.editor().select_command(line.map(index));
    }

    pub fn add_command(&self) {
        self.editor().add_command();
    }

    pub fn delete_command(&self) {
        self.editor().delete_command();
    }

    pub fn can_move_command(&self, up: bool) -> bool {
        self.editor().can_move_command(up)
    }

    pub fn move_command(&self, up: bool) {
        self.editor().move_command(up);
    }

    /// The selected command's name.
    pub fn command_name(&self) -> Option<String> {
        self.editor().command().map(|c| c.name.clone())
    }

    pub fn rename_command(&self, name: String) {
        self.editor().rename_command(&name);
    }

    /// The selected command's gesture as arrows, empty when it has none.
    pub fn gesture(&self) -> String {
        self.editor()
            .command()
            .map(|c| c.gesture.to_string())
            .unwrap_or_default()
    }

    /// Adds a stroke: 0 left, 1 up, 2 right, 3 down, the order of the
    /// buttons.
    pub fn push_direction(&self, direction: u8) {
        if let Some(&d) = Direction::ALL.get(usize::from(direction)) {
            self.editor().push_direction(d);
        }
    }

    pub fn pop_direction(&self) {
        self.editor().pop_direction();
    }

    pub fn clear_gesture(&self) {
        self.editor().clear_gesture();
    }

    /// The other command with the selected one's gesture, as the window
    /// says it.
    pub fn gesture_conflict(&self) -> Option<String> {
        self.editor().gesture_conflict()
    }

    pub fn key_lines(&self) -> Vec<KeyEntry> {
        self.editor()
            .key_rows()
            .into_iter()
            .map(|r| KeyEntry {
                label: r.label,
                caps: r
                    .keys
                    .map(|k| k.caps(Look::Mac).into_iter().map(|c| c.text()).collect())
                    .unwrap_or_default(),
            })
            .collect()
    }

    /// Starts `recorder` out showing the keys of line `line`.
    pub fn start_recording(&self, line: u32, recorder: Arc<KeyRecorder>) {
        let keys = self
            .editor()
            .key_rows()
            .get(index(line))
            .and_then(|r| r.keys);
        let mut state = lock(&recorder.state);
        state.recorder = Recorder::showing(keys);
        state.flags = 0;
    }

    /// 決定: the keys `recorder` recorded go on line `line`.
    pub fn take_recording(&self, line: u32, recorder: Arc<KeyRecorder>) {
        let keys = lock(&recorder.state).recorder.result();
        if keys.is_some() {
            self.editor().set_keys(index(line), keys);
        }
    }

    pub fn clear_keys(&self, line: u32) {
        self.editor().set_keys(index(line), None);
    }

    pub fn is_changed(&self) -> bool {
        self.editor().is_changed()
    }

    /// Checks and writes the config to `path`; or why it cannot be saved,
    /// with the set or command the problem is in selected.
    pub fn save(&self, path: String) -> Option<String> {
        let mut editor = self.editor();
        let config = match editor.check() {
            Ok(config) => config,
            Err(p) => return Some(p.message),
        };
        if let Err(e) = config.save(Path::new(&path)) {
            return Some(e.to_string());
        }
        editor.saved();
        None
    }

    /// Writes what is being edited to `path`, checked as saving checks it.
    pub fn export_file(&self, path: String) -> Option<String> {
        let config = match self.editor().check() {
            Ok(config) => config,
            Err(p) => return Some(p.message),
        };
        config.save(Path::new(&path)).err().map(|e| e.to_string())
    }

    /// Puts the config at `path` in place of what is being edited, or says
    /// why it cannot be read. A file for Windows is refused.
    pub fn import_file(&self, path: String) -> Option<String> {
        match Config::read(Path::new(&path)) {
            Ok(config) => {
                self.editor().replace(config);
                None
            }
            Err(e) => Some(e.to_string()),
        }
    }
}

/// Why the settings could not be opened.
#[derive(Debug, uniffi::Error)]
pub enum SettingsError {
    Failed { message: String },
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingsError::Failed { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for SettingsError {}

/// Follows the keys pressed while the recording dialog is open, as macOS
/// reports them: key codes for keys, and for the modifiers the flags as
/// they change.
#[derive(Default, uniffi::Object)]
pub struct KeyRecorder {
    state: Mutex<RecorderState>,
}

#[derive(Default)]
struct RecorderState {
    recorder: Recorder,
    /// The flags of the last change, to tell which modifier changed.
    flags: u64,
}

#[uniffi::export]
impl KeyRecorder {
    #[uniffi::constructor]
    pub fn new() -> KeyRecorder {
        KeyRecorder::default()
    }

    pub fn flags_changed(&self, flags: u64) {
        let mut state = lock(&self.state);
        for (press, down) in mac::modifier_changes(state.flags, flags) {
            if down {
                state.recorder.key_down(press);
            } else {
                state.recorder.key_up(press);
            }
        }
        state.flags = flags;
    }

    /// A key went down, by its virtual key code; one meta-gesture has no
    /// name for is ignored.
    pub fn key_down(&self, code: u16) {
        if let Some(key) = mac::key(code) {
            lock(&self.state).recorder.key_down(Press::Key(key));
        }
    }

    pub fn key_up(&self, code: u16) {
        if let Some(key) = mac::key(code) {
            lock(&self.state).recorder.key_up(Press::Key(key));
        }
    }

    /// クリア: nothing recorded.
    pub fn clear(&self) {
        lock(&self.state).recorder = Recorder::default();
    }

    /// What has been pressed, as caps.
    pub fn caps(&self) -> Vec<String> {
        lock(&self.state)
            .recorder
            .caps(Look::Mac)
            .into_iter()
            .map(|c| c.text())
            .collect()
    }

    /// Whether a key that is not a modifier has been recorded, which 決定
    /// saves.
    pub fn is_complete(&self) -> bool {
        lock(&self.state).recorder.result().is_some()
    }
}

/// The app's icon, `size` pixels square, as RGBA bytes row by row from the
/// top, not premultiplied.
#[uniffi::export]
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    to_rgba(art::mac_pixels(size as usize))
}

/// The menu-bar glyph, `size` pixels square, as `app_icon_rgba` lays it out.
/// Black throughout: it is a template image, coloured by the menu bar.
#[uniffi::export]
pub fn menu_bar_icon_rgba(size: u32) -> Vec<u8> {
    to_rgba(art::menu_bar_pixels(size as usize))
}

fn to_rgba(pixels: Vec<u32>) -> Vec<u8> {
    pixels
        .into_iter()
        .flat_map(|argb| {
            let [a, r, g, b] = argb.to_be_bytes();
            [r, g, b, a]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Os;

    const CONTROL: u64 = 0x0004_0000;
    const COMMAND: u64 = 0x0010_0000;
    const LEFT_BRACKET: u16 = 0x21;
    const W: u16 = 0x0D;

    /// A config file of its own for each test, which may write it.
    fn config_file(text: &str) -> String {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("meta-gesture-ffi-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        let os = match Os::CURRENT {
            Os::Windows => "windows",
            Os::Macos => "macos",
        };
        std::fs::write(&path, format!("os = \"{os}\"\n{text}")).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn browsers() -> String {
        config_file(
            r#"
[[sets]]
name = "ブラウザ"
apps = [{ id = "com.apple.Safari", name = "Safari" }]

[[sets.commands]]
name = "戻る"
gesture = "←"
keys = "Cmd+Oem4"

[[sets.commands]]
name = "右側のタブを閉じる"
gesture = "↓→"
"#,
        )
    }

    /// Draws a path through `points` in small steps.
    fn draw(r: &Resident, app: &str, points: &[(f64, f64)]) -> (bool, GestureResult) {
        let followed = r.begin(app.into(), points[0].0, points[0].1);
        for w in points.windows(2) {
            for i in 1..=10 {
                let f = f64::from(i) / 10.0;
                r.moved(
                    w[0].0 + (w[1].0 - w[0].0) * f,
                    w[0].1 + (w[1].1 - w[0].1) * f,
                );
            }
        }
        (followed, r.ended())
    }

    #[test]
    fn a_gesture_comes_to_keys_for_the_application() {
        let r = Resident::new();
        assert_eq!(r.load(browsers()), None);
        let (followed, result) = draw(&r, "com.apple.Safari", &[(100.0, 100.0), (0.0, 100.0)]);
        assert!(followed);
        assert_eq!(
            result,
            GestureResult::Send {
                command: "戻る".into(),
                code: LEFT_BRACKET,
                flags: COMMAND,
            }
        );
        let (_, result) = draw(
            &r,
            "com.apple.Safari",
            &[(0.0, 0.0), (0.0, 80.0), (80.0, 80.0)],
        );
        assert_eq!(
            result,
            GestureResult::Unassigned {
                command: "右側のタブを閉じる".into()
            }
        );
        let (_, result) = draw(&r, "com.apple.Safari", &[(0.0, 0.0), (0.0, -80.0)]);
        assert_eq!(
            result,
            GestureResult::Unknown {
                gesture: "↑".into()
            }
        );
    }

    #[test]
    fn a_press_that_hardly_moves_is_a_click() {
        let r = Resident::new();
        r.load(browsers());
        let (followed, result) = draw(&r, "com.apple.Safari", &[(0.0, 0.0), (3.0, 2.0)]);
        assert!(followed);
        assert_eq!(result, GestureResult::Click);
        assert!(!r.is_following());
    }

    #[test]
    fn an_application_no_gesture_is_for_is_left_alone() {
        let r = Resident::new();
        r.load(browsers());
        assert!(!r.begin("com.apple.TextEdit".into(), 0.0, 0.0));
        assert!(!r.is_following());
        assert_eq!(r.ended(), GestureResult::Nothing);
    }

    #[test]
    fn edits_and_saves_through_the_settings_editor() {
        let path = browsers();
        let s = SettingsEditor::open(path.clone()).unwrap();
        assert_eq!(s.set_lines().len(), 2);
        s.select_set(1);
        assert_eq!(s.apps()[0].name, "Safari");
        s.select_command(Some(0));
        assert_eq!(s.gesture(), "←");
        assert_eq!(
            s.key_lines()[0].caps,
            ["⌘", "Oem4"].map(String::from).to_vec()
        );
        assert_eq!(s.key_lines()[1].caps, Vec::<String>::new());

        // Records ⌃W for Safari alone.
        let recorder = Arc::new(KeyRecorder::new());
        s.start_recording(1, recorder.clone());
        assert!(!recorder.is_complete());
        recorder.flags_changed(CONTROL);
        recorder.key_down(W);
        assert_eq!(recorder.caps(), ["⌃", "W"].map(String::from).to_vec());
        s.take_recording(1, recorder.clone());
        assert_eq!(s.key_lines()[1].caps, ["⌃", "W"].map(String::from).to_vec());
        assert!(s.is_changed());

        s.add_command();
        assert!(
            s.save(path.clone())
                .unwrap()
                .contains("ジェスチャーがありません")
        );
        s.push_direction(3);
        s.push_direction(0);
        assert_eq!(s.gesture(), "↓←");
        assert_eq!(s.save(path.clone()), None);
        assert!(!s.is_changed());

        let r = Resident::new();
        r.load(path);
        let (_, result) = draw(&r, "com.apple.Safari", &[(100.0, 100.0), (0.0, 100.0)]);
        assert_eq!(
            result,
            GestureResult::Send {
                command: "戻る".into(),
                code: W,
                flags: CONTROL,
            }
        );
    }

    #[test]
    fn refuses_to_import_a_file_for_the_other_platform() {
        let s = SettingsEditor::open(browsers()).unwrap();
        let other = config_file("").replace("config.toml", "other.toml");
        let os = match Os::CURRENT {
            Os::Windows => "macos",
            Os::Macos => "windows",
        };
        std::fs::write(&other, format!("os = \"{os}\"\n")).unwrap();
        assert!(s.import_file(other).unwrap().contains("用の設定です"));
        assert_eq!(s.set_lines().len(), 2);
    }

    #[test]
    fn the_icons_are_rgba() {
        let icon = app_icon_rgba(64);
        assert_eq!(icon.len(), 64 * 64 * 4);
        // The margin is transparent; the tile is opaque.
        assert_eq!(icon[(32 * 64 + 2) * 4 + 3], 0);
        assert_eq!(icon[(8 * 64 + 50) * 4 + 3], 0xFF);
        let glyph = menu_bar_icon_rgba(18);
        assert_eq!(glyph.len(), 18 * 18 * 4);
        assert!(glyph.chunks(4).all(|p| p[..3] == [0, 0, 0]));
    }

    #[test]
    fn adds_the_browsers_preset_with_safari_keys() {
        let s = SettingsEditor::open(config_file("")).unwrap();
        assert_eq!(s.presets(), vec!["ブラウザ".to_string()]);
        s.add_preset(
            0,
            vec![AppEntry {
                id: "com.apple.Safari".into(),
                name: "Safari".into(),
            }],
        );
        assert_eq!(s.set_name(), "ブラウザ");
        s.select_command(Some(0));
        assert_eq!(s.gesture(), "←");
        // Safari's keys are only known on macOS.
        let safari = &s.key_lines()[1];
        assert_eq!(safari.label, "Safari");
        match Os::CURRENT {
            Os::Macos => assert_eq!(safari.caps, ["⌘", "Oem4"].map(String::from).to_vec()),
            Os::Windows => assert!(safari.caps.is_empty()),
        }
    }
}
