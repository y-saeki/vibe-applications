//! The settings window as the macOS app draws it: the list being edited
//! (`draft::Editor`), the dialog that records a shortcut
//! (`hotkey::Recorder`), and what each of them shows, ready to draw. The
//! window itself only lays these out and reports what was clicked or typed.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use super::Frame;
use crate::config::{Config, Theme};
use crate::draft::{self, Click, Editor};
use crate::hotkey::{self, Arrow, Hotkey, HotkeyError, Keycap, Keys, Look, Press, Recorder};
use crate::layout::{Anchor, Rect};
use crate::mac;

/// One key as a keycap: its label, or for an arrow key the direction.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Cap {
    Text { label: String },
    Arrow { direction: Direction },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Direction {
    Left,
    Up,
    Right,
    Down,
}

/// Strokes as keycaps, one group each, with a Mac's modifier symbols.
fn caps(strokes: &[Hotkey]) -> Vec<Vec<Cap>> {
    hotkey::strokes_caps_in(Look::Mac, strokes)
        .into_iter()
        .map(|group| group.into_iter().map(cap).collect())
        .collect()
}

fn cap(keycap: Keycap) -> Cap {
    match keycap {
        Keycap::Arrow(arrow) => Cap::Arrow {
            direction: match arrow {
                Arrow::Left => Direction::Left,
                Arrow::Up => Direction::Up,
                Arrow::Right => Direction::Right,
                Arrow::Down => Direction::Down,
            },
        },
        other => Cap::Text {
            label: other.label().unwrap_or_default(),
        },
    }
}

/// Why a recorded combination cannot be a shortcut, in a Mac's names for
/// the modifiers that it needs.
fn problem(e: &HotkeyError) -> String {
    match e {
        HotkeyError::NeedsModifier => "Control・Option・Command のいずれかを含めてください".into(),
        e => e.to_string(),
    }
}

/// A line of the list.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct Row {
    /// What the line is kept by: it goes with the row when the row is
    /// dragged, as `reorder` takes them.
    pub id: u64,
    pub selected: bool,
    pub keys: Vec<Vec<Cap>>,
    pub anchor: String,
    pub width: String,
    pub height: String,
    /// The anchor's column spans the sizes' too.
    pub reset: bool,
    /// The width's column spans the height's too.
    pub fit: bool,
}

/// The fields of the row being edited.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct Fields {
    pub keys: Vec<Vec<Cap>>,
    /// Which of [`anchors`] it is.
    pub anchor: u8,
    pub width: String,
    pub height: String,
    pub fit: bool,
    pub reset: bool,
    /// Why the width or the height does not make a placement, shown under
    /// them while they are typed.
    pub problem: Option<String>,
}

/// One of the nine anchors, in the order they are laid out, three by three.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct AnchorChoice {
    pub label: String,
    pub can_fit: bool,
}

#[uniffi::export]
pub fn anchors() -> Vec<AnchorChoice> {
    Anchor::ALL
        .into_iter()
        .map(|a| AnchorChoice {
            label: a.label().to_string(),
            can_fit: a.can_fit(),
        })
        .collect()
}

/// The themes the settings window offers, in order, by name.
#[uniffi::export]
pub fn themes() -> Vec<String> {
    Theme::ALL.iter().map(|t| t.label().to_string()).collect()
}

/// A row's placement drawn on a shrunk screen: the screen, and the window
/// unless the fields do not make a placement yet. Edges marked loose are
/// drawn dashed.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct PlacementPreview {
    pub screen: Frame,
    pub window: Option<PreviewedWindow>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct PreviewedWindow {
    pub frame: Frame,
    pub loose_left: bool,
    pub loose_top: bool,
    pub loose_right: bool,
    pub loose_bottom: bool,
}

/// A piece of a dashed line.
#[derive(Clone, Copy, Debug, PartialEq, uniffi::Record)]
pub struct Dash {
    pub from: f64,
    pub to: f64,
}

/// The dashes of a line from `from` to `to`, as `draft::dashes` lays them.
#[uniffi::export]
pub fn dashes(from: f64, to: f64, dash: f64, gap: f64) -> Vec<Dash> {
    draft::dashes(from, to, dash, gap)
        .into_iter()
        .map(|(from, to)| Dash { from, to })
        .collect()
}

/// The list the settings window edits, and the theme chosen there.
#[derive(Default, uniffi::Object)]
pub struct SettingsEditor {
    state: Mutex<EditorState>,
}

#[derive(Default)]
struct EditorState {
    editor: Editor,
    theme: Theme,
    theme_was: Theme,
}

impl SettingsEditor {
    fn state(&self) -> MutexGuard<'_, EditorState> {
        // A panic aborts the process, so the lock is never poisoned.
        self.state.lock().expect("the state is never poisoned")
    }

    fn edit(&self, f: impl FnOnce(&mut draft::Draft)) {
        self.state().editor.edit(f);
    }

    /// The rows that make a shortcut as they stand: what the test window
    /// answers to.
    pub(super) fn trial_config(&self) -> Config {
        self.state().editor.trial_config()
    }

    #[cfg(test)]
    fn with(config: &Config) -> SettingsEditor {
        let editor = SettingsEditor::new();
        editor.open(config);
        editor
    }

    fn open(&self, config: &Config) {
        let mut state = self.state();
        state.editor = Editor::new(
            config
                .shortcuts
                .iter()
                .map(draft::Draft::from_shortcut)
                .collect(),
        );
        state.theme = config.theme;
        state.theme_was = config.theme;
    }
}

#[uniffi::export]
impl SettingsEditor {
    #[uniffi::constructor]
    pub fn new() -> SettingsEditor {
        SettingsEditor::default()
    }

    /// Starts over from the config file at `path`, or says why it cannot
    /// be read.
    pub fn load(&self, path: String) -> Option<String> {
        match Config::load_or_create(Path::new(&path)) {
            Ok(config) => {
                self.open(&config);
                None
            }
            Err(e) => Some(e.to_string()),
        }
    }

    pub fn rows(&self) -> Vec<Row> {
        let state = self.state();
        state
            .editor
            .rows()
            .map(|(id, d)| Row {
                id,
                selected: state.editor.is_selected(id),
                keys: caps(&d.keys),
                anchor: d.anchor_text(),
                width: d.width_text(),
                height: d.height_text(),
                reset: d.reset,
                fit: d.fit,
            })
            .collect()
    }

    /// The fields of the row being edited: the selected row, when it is
    /// the only one.
    pub fn fields(&self) -> Option<Fields> {
        let state = self.state();
        let d = state.editor.selected()?;
        Some(Fields {
            keys: caps(&d.keys),
            anchor: Anchor::ALL.iter().position(|a| *a == d.anchor).unwrap_or(0) as u8,
            width: d.width.clone(),
            height: d.height.clone(),
            fit: d.fit,
            reset: d.reset,
            problem: d.size_problem(),
        })
    }

    pub fn has_selection(&self) -> bool {
        !self.state().editor.selection().is_empty()
    }

    pub fn can_move(&self, up: bool) -> bool {
        self.state().editor.can_move(up)
    }

    /// Row `index` was clicked, with Shift held or not, and with Command,
    /// which adds to the selection as Ctrl does on Windows.
    pub fn click(&self, index: u32, shift: bool, command: bool) {
        self.state().editor.click(
            index as usize,
            Click {
                shift,
                ctrl: command,
            },
        );
    }

    pub fn add(&self) {
        self.state().editor.add();
    }

    pub fn duplicate(&self) {
        self.state().editor.duplicate();
    }

    pub fn delete(&self) {
        self.state().editor.delete();
    }

    pub fn move_selected(&self, up: bool) {
        self.state().editor.move_selected(up);
    }

    /// The list was dragged into the order of `ids`.
    pub fn reorder(&self, ids: Vec<u64>) {
        self.state().editor.reorder(&ids);
    }

    pub fn set_anchor(&self, index: u8) {
        if let Some(&a) = Anchor::ALL.get(usize::from(index)) {
            self.edit(|d| d.anchor = a);
        }
    }

    pub fn set_width(&self, text: String) {
        self.edit(|d| d.width = text);
    }

    pub fn set_height(&self, text: String) {
        self.edit(|d| d.height = text);
    }

    pub fn set_fit(&self, on: bool) {
        self.edit(|d| d.fit = on);
    }

    pub fn set_reset(&self, on: bool) {
        self.edit(|d| d.reset = on);
    }

    /// Readies `recorder` for the row being edited: it starts out showing
    /// the row's shortcut.
    pub fn start_recording(&self, recorder: Arc<KeyRecorder>) {
        let keys = self
            .state()
            .editor
            .selected()
            .map(|d| d.keys.clone())
            .unwrap_or_default();
        recorder.show(keys);
    }

    /// Gives the row being edited what `recorder` recorded, when that is a
    /// shortcut.
    pub fn take_recording(&self, recorder: Arc<KeyRecorder>) {
        if let Some(keys) = recorder.keys() {
            self.edit(|d| d.keys = keys.strokes().to_vec());
        }
    }

    /// Which of [`themes`] is chosen.
    pub fn theme(&self) -> u8 {
        let theme = self.state().theme;
        Theme::ALL.iter().position(|t| *t == theme).unwrap_or(0) as u8
    }

    pub fn set_theme(&self, index: u8) {
        if let Some(&theme) = Theme::ALL.get(usize::from(index)) {
            self.state().theme = theme;
        }
    }

    /// Whether anything would be lost by closing without saving.
    pub fn is_changed(&self) -> bool {
        let state = self.state();
        state.editor.is_dirty() || state.theme != state.theme_was
    }

    /// Checks every row and writes the config file at `path`. When a row
    /// does not make a shortcut, that row is selected alone and this says
    /// why; it also says why the file could not be written.
    pub fn save(&self, path: String) -> Option<String> {
        let mut state = self.state();
        let mut config = match state.editor.build_config() {
            Ok(config) => config,
            Err(e) => return Some(e),
        };
        config.theme = state.theme;
        config.save(Path::new(&path)).err().map(|e| e.to_string())
    }

    /// Where the row being edited puts a window on a screen of `work_width`
    /// by `work_height`, shrunk into `width` by `height` and drawn at its
    /// top left. `None` with no row being edited.
    pub fn preview(
        &self,
        work_width: i32,
        work_height: i32,
        width: i32,
        height: i32,
    ) -> Option<PlacementPreview> {
        let state = self.state();
        let bounds = Rect {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        let picture =
            state
                .editor
                .selected()?
                .picture((work_width, work_height), bounds, Anchor::TopLeft)?;
        Some(PlacementPreview {
            screen: picture.screen.into(),
            window: picture.window.map(|w| PreviewedWindow {
                frame: w.rect.into(),
                loose_left: w.loose.left,
                loose_top: w.loose.top,
                loose_right: w.loose.right,
                loose_bottom: w.loose.bottom,
            }),
        })
    }
}

/// What the recording dialog shows.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct Recording {
    /// The strokes recorded, one group of keycaps each; while only
    /// modifiers have been pressed, those alone. Empty before anything is.
    pub keys: Vec<Vec<Cap>>,
    /// Why what was recorded cannot be a shortcut.
    pub problem: Option<String>,
    /// Whether what was recorded is a shortcut, which 決定 saves.
    pub complete: bool,
}

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
    /// The shortcut the dialog opened with, which リセット goes back to.
    showing: Vec<Hotkey>,
}

impl KeyRecorder {
    fn state(&self) -> MutexGuard<'_, RecorderState> {
        // A panic aborts the process, so the lock is never poisoned.
        self.state.lock().expect("the state is never poisoned")
    }

    fn show(&self, keys: Vec<Hotkey>) {
        let mut state = self.state();
        state.recorder = Recorder::showing(&keys);
        state.showing = keys;
    }

    fn keys(&self) -> Option<Keys> {
        self.state().recorder.result()?.ok()
    }
}

#[uniffi::export]
impl KeyRecorder {
    #[uniffi::constructor]
    pub fn new() -> KeyRecorder {
        KeyRecorder::default()
    }

    pub fn flags_changed(&self, flags: u64) {
        let mut state = self.state();
        for (press, down) in mac::modifier_changes(state.flags, flags) {
            if down {
                state.recorder.key_down(press);
            } else {
                state.recorder.key_up(press);
            }
        }
        state.flags = flags;
    }

    /// A key went down, by its virtual key code; one winwin has no name for
    /// is ignored.
    pub fn key_down(&self, code: u16) {
        if let Some(key) = mac::key(code) {
            self.state().recorder.key_down(Press::Key(key));
        }
    }

    pub fn key_up(&self, code: u16) {
        if let Some(key) = mac::key(code) {
            self.state().recorder.key_up(Press::Key(key));
        }
    }

    /// リセット: back to the shortcut the dialog opened with.
    pub fn reset(&self) {
        let mut state = self.state();
        state.recorder = Recorder::showing(&state.showing);
    }

    /// クリア: nothing recorded.
    pub fn clear(&self) {
        self.state().recorder = Recorder::default();
    }

    pub fn recording(&self) -> Recording {
        let state = self.state();
        let r = &state.recorder;
        let strokes = r.strokes();
        let keys = if !strokes.is_empty() {
            caps(&strokes)
        } else if !r.modifiers.is_empty() {
            let group = hotkey::keycaps_in(Look::Mac, r.modifiers, None);
            vec![group.into_iter().map(cap).collect()]
        } else {
            Vec::new()
        };
        let result = r.result();
        Recording {
            keys,
            problem: match &result {
                Some(Err(e)) => Some(problem(e)),
                _ => None,
            },
            complete: matches!(result, Some(Ok(_))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTROL: u64 = 0x0004_0000;
    const SHIFT: u64 = 0x0002_0000;
    const LEFT: u16 = 0x7B;
    const K: u16 = 0x28;

    fn text(label: &str) -> Cap {
        Cap::Text {
            label: label.into(),
        }
    }

    fn editor() -> SettingsEditor {
        SettingsEditor::with(&Config::defaults())
    }

    #[test]
    fn shows_the_rows_with_a_macs_keycaps() {
        let e = editor();
        let rows = e.rows();
        assert_eq!(rows.len(), Config::defaults().shortcuts.len());
        assert!(rows[0].selected && !rows[1].selected);
        assert_eq!(
            rows[0].keys,
            [vec![
                text("⌃"),
                text("⌥"),
                Cap::Arrow {
                    direction: Direction::Left
                }
            ]]
        );
        assert_eq!(
            (rows[0].anchor.as_str(), rows[0].width.as_str()),
            ("左", "w1/2")
        );
        let fields = e.fields().unwrap();
        assert_eq!(anchors()[usize::from(fields.anchor)].label, "左");
        assert_eq!((fields.width.as_str(), fields.problem), ("1/2", None));
    }

    #[test]
    fn edits_reach_the_row_and_count_as_changes() {
        let e = editor();
        assert!(!e.is_changed());
        e.set_width("2".into());
        assert!(e.fields().unwrap().problem.unwrap().starts_with("幅:"));
        e.set_width("1/3".into());
        e.set_anchor(4);
        assert_eq!(e.rows()[0].anchor, "中央");
        assert!(e.is_changed());
        // Out of range, nothing changes.
        e.set_anchor(9);
        assert_eq!(e.rows()[0].anchor, "中央");
    }

    #[test]
    fn clicks_with_command_and_shift_select_several() {
        let e = editor();
        e.click(2, false, true);
        assert_eq!(e.fields(), None);
        assert!(e.has_selection() && !e.can_move(true));
        e.click(4, true, false);
        let selected: Vec<bool> = e.rows().iter().map(|r| r.selected).take(6).collect();
        assert_eq!(selected, [false, false, true, true, true, false]);
        let ids: Vec<u64> = e.rows().iter().rev().map(|r| r.id).collect();
        e.reorder(ids.clone());
        assert_eq!(e.rows().iter().map(|r| r.id).collect::<Vec<_>>(), ids);
    }

    #[test]
    fn the_theme_is_a_change_of_its_own() {
        let e = editor();
        assert_eq!(themes()[usize::from(e.theme())], Theme::System.label());
        e.set_theme(2);
        assert!(e.is_changed());
        e.set_theme(9);
        assert_eq!(e.theme(), 2);
    }

    #[test]
    fn saving_names_the_row_that_cannot_be_saved() {
        let e = editor();
        e.click(3, false, false);
        e.set_height("tall".into());
        e.click(0, false, false);
        let dir = std::env::temp_dir().join(format!("winwin-ffi-{}", std::process::id()));
        let path = dir.join("config.toml");
        let problem = e.save(path.to_string_lossy().into()).unwrap();
        assert!(problem.contains("高さ"), "{problem}");
        assert!(e.rows()[3].selected);
        e.set_height("1/2".into());
        e.set_theme(1);
        assert_eq!(e.save(path.to_string_lossy().into()), None);
        let saved = Config::load_or_create(&path).unwrap();
        assert_eq!(saved.theme, Theme::ALL[1]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn records_from_flags_and_key_codes() {
        let e = editor();
        let r = Arc::new(KeyRecorder::new());
        e.start_recording(r.clone());
        // It opens on the row's shortcut.
        assert!(r.recording().complete);
        r.flags_changed(CONTROL);
        r.flags_changed(CONTROL | SHIFT);
        assert_eq!(r.recording().keys, [vec![text("⌃"), text("⇧")]]);
        assert!(!r.recording().complete);
        r.key_down(K);
        assert_eq!(r.recording().keys, [vec![text("⌃"), text("⇧"), text("K")]]);
        e.take_recording(r.clone());
        assert_eq!(e.rows()[0].keys, r.recording().keys);
        // Shift alone cannot be a shortcut, and says so in a Mac's names.
        r.key_up(K);
        r.flags_changed(SHIFT);
        r.flags_changed(0);
        r.flags_changed(SHIFT);
        r.key_down(LEFT);
        assert!(r.recording().problem.unwrap().contains("Command"));
        r.reset();
        assert_eq!(
            r.recording().keys,
            [vec![
                text("⌃"),
                text("⌥"),
                Cap::Arrow {
                    direction: Direction::Left
                }
            ]]
        );
        r.clear();
        assert_eq!(r.recording().keys, Vec::<Vec<Cap>>::new());
    }

    #[test]
    fn previews_the_row_being_edited() {
        let e = editor();
        let preview = e.preview(1440, 900, 320, 200).unwrap();
        assert_eq!(
            preview.screen,
            Frame {
                left: 0,
                top: 0,
                right: 320,
                bottom: 200
            }
        );
        assert_eq!(preview.window.unwrap().frame.right, 160);
        e.click(1, false, true);
        assert_eq!(e.preview(1440, 900, 320, 200), None);
        assert_eq!(
            dashes(0.0, 10.0, 10.0, 4.0),
            [Dash {
                from: 0.0,
                to: 10.0
            }]
        );
    }
}
