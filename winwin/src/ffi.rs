//! The library as the macOS app (`macos/`) calls it, through the Swift
//! bindings UniFFI writes from this file. The app passes on what the system
//! tells it, such as a key event or the screens' frames, and does what comes
//! back; what that is is decided here, as the rest of the library decides it
//! for `src/win/`.
//!
//! Built with the `ffi` feature only.

mod settings;

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use settings::SettingsEditor;

use crate::config::{Config, Theme};
use crate::cycle::{self, Binding, Outcome, Presses};
use crate::layout::{self, Action, Placement, Rect, Sizing};
use crate::restore::Sizes;
use crate::{art, mac};

/// What `restore::Sizes` takes as the DPI of a monitor. Points are the same
/// size on every screen, so one value does for all of them.
const POINTS_DPI: u32 = 96;

/// A rectangle in the coordinates windows are moved in: points, counting
/// down from the top left of the main screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct Frame {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl From<Frame> for Rect {
    fn from(f: Frame) -> Rect {
        Rect {
            left: f.left,
            top: f.top,
            right: f.right,
            bottom: f.bottom,
        }
    }
}

impl From<Rect> for Frame {
    fn from(r: Rect) -> Frame {
        Frame {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

/// What to do with a key event.
#[derive(uniffi::Enum)]
pub enum KeyResult {
    /// It is no shortcut of winwin's: let it through to the application.
    Pass,
    /// It is one; keep it from the application, and do nothing else.
    Take,
    /// It is one; keep it from the application, and place the frontmost
    /// window.
    Place { placement: Arc<Placing> },
    /// It is one; keep it from the application, and give the frontmost
    /// window back where it was before winwin first placed it (see
    /// [`Resident::reset`]).
    Reset,
}

/// Where a shortcut puts the window.
#[derive(uniffi::Object)]
pub struct Placing(Placement);

#[uniffi::export]
impl Placing {
    /// Whether it fits between the other windows, so that `resolve` needs
    /// their frames.
    pub fn fits(&self) -> bool {
        self.0.sizing == Sizing::Fit
    }

    /// The window's frame on the screen whose visible frame is `work`, for
    /// a window at `current`. `others` are the frames of the other windows,
    /// front to back, which only a placement that fits looks at. `None` when
    /// it fits and finds no room.
    pub fn resolve(&self, work: Frame, current: Frame, others: Vec<Frame>) -> Option<Frame> {
        let current = Rect::from(current);
        self.0
            .resolve(work.into(), (current.width(), current.height()), || {
                others.into_iter().map(Rect::from).collect()
            })
            .map(Frame::from)
    }
}

/// The resident part's state: the shortcuts of the config, how far the
/// presses of them have got, and where the windows winwin placed were
/// before. The app gives each window it places a number of its own, which
/// stands for the window here.
#[derive(Default, uniffi::Object)]
pub struct Resident {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    bindings: Vec<Binding>,
    presses: Presses,
    sizes: Sizes,
    theme: Theme,
}

impl Resident {
    fn state(&self) -> MutexGuard<'_, State> {
        // A panic aborts the process, so the lock is never poisoned.
        self.state.lock().expect("the state is never poisoned")
    }

    fn set(&self, config: &Config) {
        let mut state = self.state();
        state.bindings = cycle::bindings(config);
        state.presses = Presses::default();
        state.theme = config.theme;
    }
}

#[uniffi::export]
impl Resident {
    /// No shortcuts until a config is loaded.
    #[uniffi::constructor]
    pub fn new() -> Resident {
        Resident::default()
    }

    /// Takes the shortcuts of the config file at `path`, which is written
    /// with the defaults first when there is none. When it cannot be read,
    /// the shortcuts stay as they were, and this says why.
    pub fn load(&self, path: String) -> Option<String> {
        match Config::load_or_create(Path::new(&path)) {
            Ok(config) => {
                self.set(&config);
                None
            }
            Err(e) => Some(e.to_string()),
        }
    }

    /// Takes the shortcuts the settings window's rows make as they stand,
    /// for the test window to answer to.
    pub fn use_trial(&self, editor: Arc<SettingsEditor>) {
        self.set(&editor.trial_config());
    }

    /// Which of the settings window's themes the config chose, for the
    /// menu bar's menu.
    pub fn theme(&self) -> u8 {
        let theme = self.state().theme;
        Theme::ALL.iter().position(|t| *t == theme).unwrap_or(0) as u8
    }

    /// A key went down: its virtual key code, the event's modifier flags,
    /// and whether it is held down, repeating. Only the strokes that
    /// would be registered on Windows are taken, and a repeat does nothing,
    /// as a hotkey registered without repeats does not.
    pub fn key_down(&self, code: u16, flags: u64, held: bool) -> KeyResult {
        let Some(stroke) = mac::hotkey(code, flags) else {
            return KeyResult::Pass;
        };
        let state = &mut *self.state();
        let registered = cycle::first_strokes(&state.bindings).contains(&stroke)
            || state
                .presses
                .next_strokes(&state.bindings)
                .contains(&stroke);
        if !registered {
            return KeyResult::Pass;
        }
        if held {
            return KeyResult::Take;
        }
        match state.presses.press(&state.bindings, stroke) {
            Outcome::Place { binding, entry } => match state.bindings[binding].actions[entry] {
                Action::Place(p) => KeyResult::Place {
                    placement: Arc::new(Placing(p)),
                },
                Action::Reset => KeyResult::Reset,
            },
            Outcome::Pending | Outcome::Nothing => KeyResult::Take,
        }
    }

    /// The modifiers held changed to `flags`. Letting go of any of the
    /// modifiers of a shortcut under way ends it, as on Windows.
    pub fn flags_changed(&self, flags: u64) {
        let state = &mut *self.state();
        if let Some(m) = state.presses.watching(&state.bindings)
            && !mac::modifiers(flags).contains(m)
        {
            state.presses.reset();
        }
    }

    /// A shortcut moved `window` from `before` to `after`. When the window
    /// was still where an earlier shortcut put it, where it was before that
    /// one is kept.
    pub fn placed(&self, window: u64, before: Frame, after: Frame) {
        self.state().sizes.placed(
            window as isize,
            before.into(),
            false,
            POINTS_DPI,
            after.into(),
        );
    }

    /// A reset shortcut was pressed on `window`, which is at `current`:
    /// where it was before winwin first placed it, and winwin forgets it.
    /// When none of the `screens` shows that place any more, the window gets
    /// its size back where it is. `None` for a window winwin does not
    /// remember.
    pub fn reset(&self, window: u64, current: Frame, screens: Vec<Frame>) -> Option<Frame> {
        let screens: Vec<Rect> = screens.into_iter().map(Rect::from).collect();
        let shown = |r: Rect| screens.iter().any(|s| r.intersection(*s).is_some());
        let reset = self
            .state()
            .sizes
            .reset(window as isize, current.into(), POINTS_DPI, shown)?;
        Some(reset.rect.into())
    }

    /// Forgets the windows that are not among `alive`, which have closed.
    pub fn retain(&self, alive: Vec<u64>) {
        let mut state = self.state();
        state.sizes.retain(|w| alive.contains(&(w as u64)));
        // Nothing is left on a window on macOS to take away.
        state.sizes.take_forgotten();
    }
}

/// Which of the screens' frames `areas` the window at `window` is on.
#[uniffi::export]
pub fn area_of(window: Frame, areas: Vec<Frame>) -> Option<u32> {
    let areas: Vec<Rect> = areas.into_iter().map(Rect::from).collect();
    layout::area_of(window.into(), &areas).map(|i| i as u32)
}

/// A frame AppKit gives, such as a screen's visible frame, as a `Frame`.
/// `main_height` is the height of the main screen.
#[uniffi::export]
pub fn from_appkit(x: f64, y: f64, width: f64, height: f64, main_height: f64) -> Frame {
    mac::from_appkit(x, y, width, height, main_height).into()
}

/// A `Frame` back in AppKit's coordinates, to set a window of winwin's own
/// to it.
#[derive(Clone, Copy, Debug, PartialEq, uniffi::Record)]
pub struct AppKitRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[uniffi::export]
pub fn to_appkit(frame: Frame, main_height: f64) -> AppKitRect {
    let (x, y, width, height) = mac::to_appkit(frame.into(), main_height);
    AppKitRect {
        x,
        y,
        width,
        height,
    }
}

/// winwin's icon, `size` pixels square, as rows of RGBA bytes from the top,
/// not premultiplied.
#[uniffi::export]
pub fn icon_rgba(size: u32) -> Vec<u8> {
    art::pixels(size as usize)
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

    const CONTROL: u64 = 0x0004_0000;
    const OPTION: u64 = 0x0008_0000;
    const LEFT: u16 = 0x7B;
    const UP: u16 = 0x7E;
    const K: u16 = 0x28;

    fn resident(toml: &str) -> Resident {
        let r = Resident::new();
        r.set(&Config::parse(toml, Path::new("config.toml")).unwrap());
        r
    }

    fn frame(left: i32, top: i32, right: i32, bottom: i32) -> Frame {
        Frame {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Where a press puts a window on a 1440 by 900 screen.
    fn placed(result: KeyResult) -> Option<Frame> {
        match result {
            KeyResult::Place { placement } => {
                let work = frame(0, 0, 1440, 900);
                placement.resolve(work, frame(0, 0, 100, 100), vec![])
            }
            KeyResult::Pass | KeyResult::Take | KeyResult::Reset => None,
        }
    }

    #[test]
    fn takes_only_the_shortcuts_of_the_config() {
        let r = resident(
            r#"
            [[shortcut]]
            keys = "Ctrl+Alt+Left"
            anchor = "left"
            width = "1/2"
            height = "1"
            "#,
        );
        assert_eq!(
            placed(r.key_down(LEFT, CONTROL | OPTION, false)),
            Some(frame(0, 0, 720, 900))
        );
        // Held down, it does nothing more, but is still kept from the
        // application.
        assert!(matches!(
            r.key_down(LEFT, CONTROL | OPTION, true),
            KeyResult::Take
        ));
        for (code, flags) in [(LEFT, CONTROL), (K, CONTROL | OPTION), (0x37, CONTROL)] {
            assert!(matches!(r.key_down(code, flags, false), KeyResult::Pass));
        }
    }

    #[test]
    fn a_second_stroke_is_taken_only_while_it_may_follow() {
        let r = resident(
            r#"
            [[shortcut]]
            keys = "Ctrl+Left, Up"
            anchor = "top-left"
            width = "1/2"
            height = "1/2"
            "#,
        );
        assert!(matches!(r.key_down(UP, CONTROL, false), KeyResult::Pass));
        assert!(matches!(r.key_down(LEFT, CONTROL, false), KeyResult::Take));
        assert_eq!(
            placed(r.key_down(UP, CONTROL, false)),
            Some(frame(0, 0, 720, 450))
        );
        // Letting go of Ctrl in between ends the wait.
        r.key_down(LEFT, CONTROL, false);
        r.flags_changed(CONTROL);
        r.flags_changed(0);
        assert!(matches!(r.key_down(UP, CONTROL, false), KeyResult::Pass));
    }

    #[test]
    fn a_reset_gives_a_window_back_where_it_was() {
        let r = resident(
            r#"
            [[shortcut]]
            keys = "Ctrl+Alt+K"
            reset = true
            "#,
        );
        assert!(matches!(
            r.key_down(K, CONTROL | OPTION, false),
            KeyResult::Reset
        ));
        let screen = frame(0, 0, 1440, 900);
        let before = frame(100, 100, 600, 500);
        let half = frame(0, 0, 720, 900);
        let quarter = frame(0, 0, 720, 450);
        r.placed(7, before, half);
        // Placed again from where the first placement left it: still the
        // place before the first one.
        r.placed(7, half, quarter);
        assert_eq!(r.reset(7, quarter, vec![screen]), Some(before));
        // Forgotten once given back, and never known for another window.
        assert_eq!(r.reset(7, before, vec![screen]), None);
        assert_eq!(r.reset(8, before, vec![screen]), None);
        // Where no screen shows it any more, only the size comes back.
        r.placed(7, frame(2000, 100, 2500, 500), half);
        assert_eq!(r.reset(7, half, vec![screen]), Some(frame(0, 0, 500, 400)));
        // A closed window is forgotten.
        r.placed(9, before, half);
        r.retain(vec![]);
        assert_eq!(r.reset(9, half, vec![screen]), None);
    }

    #[test]
    fn the_test_window_answers_to_the_rows_as_they_stand() {
        let r = Resident::new();
        let editor = Arc::new(SettingsEditor::new());
        assert!(matches!(
            r.key_down(LEFT, CONTROL | OPTION, false),
            KeyResult::Pass
        ));
        let path = std::env::temp_dir().join(format!("winwin-trial-{}.toml", std::process::id()));
        assert_eq!(editor.load(path.to_string_lossy().into()), None);
        r.use_trial(editor.clone());
        assert!(matches!(
            r.key_down(LEFT, CONTROL | OPTION, false),
            KeyResult::Place { .. }
        ));
        assert_eq!(r.theme(), 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn the_icon_is_rgba() {
        let icon = icon_rgba(16);
        assert_eq!(icon.len(), 16 * 16 * 4);
        // The corner is transparent; the middle of the left half is white.
        assert_eq!(icon[3], 0);
        let at = |x: usize, y: usize| &icon[(y * 16 + x) * 4..][..4];
        assert_eq!(at(5, 8), [0xFF, 0xFF, 0xFF, 0xFF]);
    }
}
