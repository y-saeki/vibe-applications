//! The library as the macOS app (`macos/`) calls it, through the Swift
//! bindings UniFFI writes from this file. The app passes on what the system
//! tells it, such as a key event or the screens' frames, and does what comes
//! back; what that is is decided here, as the rest of the library decides it
//! for `src/win/`.
//!
//! Built with the `ffi` feature only.

mod settings;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use settings::SettingsEditor;

use crate::config::{Config, Theme};
use crate::cycle::{self, Binding, Outcome, Presses};
use crate::layout::{self, Action, Placement, Rect, Sizing};
use crate::restore::{Ended, Remembered, Sizes};
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

/// A window that is open, as the app finds it when winwin starts: its
/// number in the window server, and the application it belongs to.
#[derive(uniffi::Record)]
pub struct OpenWindow {
    pub window: u64,
    pub owner: String,
}

/// The resident part's state: the shortcuts of the config, how far the
/// presses of them have got, and where the windows winwin placed were
/// before. A window is its number in the window server, which stays the
/// same while it is open, so what winwin remembers of it is kept in a file
/// for a winwin started again.
#[derive(Default, uniffi::Object)]
pub struct Resident {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    bindings: Vec<Binding>,
    presses: Presses,
    sizes: Sizes,
    /// The application each window winwin remembers belongs to, as the app
    /// tells it apart, so that a window number met again after a restart
    /// is known to be the same window.
    owners: HashMap<u64, String>,
    /// Where what winwin remembers is kept; nowhere until `remember_in`.
    marks: Option<PathBuf>,
    drag: Option<MouseDrag>,
    theme: Theme,
}

/// A drag of a window winwin remembers, from the button going down to it
/// coming up.
struct MouseDrag {
    window: u64,
    /// Where the window was when the button went down.
    start: Rect,
    /// It has its own size back, and the drag carries it on.
    restored: bool,
}

impl State {
    /// Writes what winwin remembers to the file, for a winwin started
    /// again. Nothing depends on it, so a file that cannot be written is
    /// left at that.
    fn save(&mut self) {
        let sizes = &self.sizes;
        self.owners
            .retain(|&window, _| sizes.tracks(window as isize));
        let Some(path) = &self.marks else { return };
        let _ = std::fs::write(path, marks::write(sizes, &self.owners));
    }

    /// The window has been resized by hand, or by its application: winwin
    /// is done with it.
    fn forget(&mut self, window: u64) {
        self.sizes.forget(window as isize);
        self.save();
    }
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

    /// Keeps what winwin remembers in the file at `path` from now on, and
    /// takes back what a winwin before this one left there of the
    /// `windows` still open in the same application.
    pub fn remember_in(&self, path: String, windows: Vec<OpenWindow>) {
        let state = &mut *self.state();
        let open: HashMap<u64, String> = windows.into_iter().map(|w| (w.window, w.owner)).collect();
        for (window, owner, kept) in
            marks::read(&std::fs::read_to_string(&path).unwrap_or_default())
        {
            if open.get(&window) == Some(&owner) {
                state.sizes.adopt(window as isize, kept);
                state.owners.entry(window).or_insert(owner);
            }
        }
        state.marks = Some(path.into());
        state.save();
    }

    /// A shortcut moved `window`, of the application `owner`, from `before`
    /// to `after`. When the window was still where an earlier shortcut put
    /// it, where it was before that one is kept.
    pub fn placed(&self, window: u64, owner: String, before: Frame, after: Frame) {
        let state = &mut *self.state();
        state.sizes.placed(
            window as isize,
            before.into(),
            false,
            POINTS_DPI,
            after.into(),
        );
        state.owners.insert(window, owner);
        state.save();
    }

    /// Whether winwin remembers `window`, which is when a click on it may
    /// start a drag worth following.
    pub fn tracks(&self, window: u64) -> bool {
        self.state().sizes.tracks(window as isize)
    }

    /// The left button went down on `window`, which is at `current`.
    /// Returns whether to follow the drag: winwin placed the window, and it
    /// is still the size winwin made it.
    pub fn mouse_down(&self, window: u64, current: Frame) -> bool {
        let state = &mut *self.state();
        state.drag = None;
        let current = Rect::from(current);
        let known = state.sizes.tracks(window as isize);
        let follow = state.sizes.drag_started(window as isize, current, false);
        if follow {
            state.drag = Some(MouseDrag {
                window,
                start: current,
                restored: false,
            });
        } else if known {
            // A window of another size than winwin made it is forgotten.
            state.save();
        }
        follow
    }

    /// The drag goes on, with `window` at `current` and the cursor at
    /// `cursor_x`, `cursor_y`. The first time the window has moved from
    /// where winwin put it, returns where to put it at its own size, the
    /// cursor holding it where it did. A window that changes size instead
    /// is being resized, and winwin forgets it.
    pub fn mouse_dragged(
        &self,
        window: u64,
        current: Frame,
        cursor_x: i32,
        cursor_y: i32,
    ) -> Option<Frame> {
        let state = &mut *self.state();
        let drag = state
            .drag
            .as_mut()
            .filter(|d| d.window == window && !d.restored)?;
        let current = Rect::from(current);
        if !same_size(drag.start, current) {
            state.drag = None;
            state.forget(window);
            return None;
        }
        let w = window as isize;
        if !state.sizes.moved(w, current, (cursor_x, cursor_y)) {
            return None;
        }
        match state
            .sizes
            .drag_ended(w, current, POINTS_DPI, (cursor_x, cursor_y), false, true)
        {
            Ended::Resume(r) | Ended::Put(r) => {
                drag.restored = true;
                Some(r.into())
            }
            Ended::Leave => None,
        }
    }

    /// The button came up, with `window` at `current` and the cursor at
    /// `cursor_x`, `cursor_y`. Where to put the window, when the drag did not
    /// leave it at its own size under the cursor.
    pub fn mouse_up(
        &self,
        window: u64,
        current: Frame,
        cursor_x: i32,
        cursor_y: i32,
    ) -> Option<Frame> {
        let state = &mut *self.state();
        let drag = state.drag.take().filter(|d| d.window == window)?;
        let current = Rect::from(current);
        if !drag.restored && !same_size(drag.start, current) {
            state.forget(window);
            return None;
        }
        let ended = state.sizes.drag_ended(
            window as isize,
            current,
            POINTS_DPI,
            (cursor_x, cursor_y),
            false,
            false,
        );
        state.save();
        match ended {
            Ended::Put(r) | Ended::Resume(r) => Some(r.into()),
            Ended::Leave => None,
        }
    }

    /// A reset shortcut was pressed on `window`, which is at `current`:
    /// where it was before winwin first placed it, and winwin forgets it.
    /// When none of the `screens` shows that place any more, the window gets
    /// its size back where it is. `None` for a window winwin does not
    /// remember.
    pub fn reset(&self, window: u64, current: Frame, screens: Vec<Frame>) -> Option<Frame> {
        let screens: Vec<Rect> = screens.into_iter().map(Rect::from).collect();
        let shown = |r: Rect| screens.iter().any(|s| r.intersection(*s).is_some());
        let state = &mut *self.state();
        let reset = state
            .sizes
            .reset(window as isize, current.into(), POINTS_DPI, shown)?;
        state.save();
        Some(reset.rect.into())
    }

    /// Forgets the windows that are not among `alive`, which have closed.
    pub fn retain(&self, alive: Vec<u64>) {
        let state = &mut *self.state();
        state.sizes.retain(|w| alive.contains(&(w as u64)));
        // The file is all there is to take away, and `save` rewrites it.
        state.sizes.take_forgotten();
        state.save();
    }
}

fn same_size(a: Rect, b: Rect) -> bool {
    a.width() == b.width() && a.height() == b.height()
}

/// The file that keeps what winwin remembers: a line for each window, with
/// its number, the three values `Remembered::pack` makes of it, and its
/// application.
mod marks {
    use std::collections::HashMap;

    use super::{Remembered, Sizes};

    pub fn write(sizes: &Sizes, owners: &HashMap<u64, String>) -> String {
        let mut lines: Vec<String> = sizes
            .remembered()
            .filter_map(|(window, kept)| {
                let owner = owners.get(&(window as u64))?;
                let [a, b, c] = kept.pack()?;
                Some(format!("{window} {a:016x} {b:016x} {c:016x} {owner}\n"))
            })
            .collect();
        lines.sort();
        lines.concat()
    }

    /// The windows in `text`, skipping any line `write` did not make.
    pub fn read(text: &str) -> Vec<(u64, String, Remembered)> {
        text.lines()
            .filter_map(|line| {
                let mut parts = line.splitn(5, ' ');
                let window = parts.next()?.parse().ok()?;
                let mut value = || u64::from_str_radix(parts.next()?, 16).ok();
                let packed = [value()?, value()?, value()?];
                let owner = parts.next().filter(|o| !o.is_empty())?;
                Some((window, owner.to_owned(), Remembered::unpack(packed)?))
            })
            .collect()
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

/// winwin's app icon, `size` pixels square with the margin macOS app icons
/// keep, as rows of RGBA bytes from the top, not premultiplied.
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
        r.placed(7, "a".into(), before, half);
        // Placed again from where the first placement left it: still the
        // place before the first one.
        r.placed(7, "a".into(), half, quarter);
        assert_eq!(r.reset(7, quarter, vec![screen]), Some(before));
        // Forgotten once given back, and never known for another window.
        assert_eq!(r.reset(7, before, vec![screen]), None);
        assert_eq!(r.reset(8, before, vec![screen]), None);
        // Where no screen shows it any more, only the size comes back.
        r.placed(7, "a".into(), frame(2000, 100, 2500, 500), half);
        assert_eq!(r.reset(7, half, vec![screen]), Some(frame(0, 0, 500, 400)));
        // A closed window is forgotten.
        r.placed(9, "a".into(), before, half);
        r.retain(vec![]);
        assert_eq!(r.reset(9, half, vec![screen]), None);
    }

    const BEFORE: Frame = Frame {
        left: 100,
        top: 100,
        right: 900,
        bottom: 700,
    };
    const HALF: Frame = Frame {
        left: 0,
        top: 25,
        right: 720,
        bottom: 900,
    };

    fn shifted(f: Frame, dx: i32, dy: i32) -> Frame {
        frame(f.left + dx, f.top + dy, f.right + dx, f.bottom + dy)
    }

    /// A window placed on the left half.
    fn placed_half() -> Resident {
        let r = Resident::new();
        assert!(!r.tracks(7));
        r.placed(7, "a".into(), BEFORE, HALF);
        assert!(r.tracks(7));
        r
    }

    #[test]
    fn a_dragged_window_gets_its_size_back_under_the_cursor() {
        let r = placed_half();
        assert!(r.mouse_down(7, HALF));
        // The button down on the title bar, a quarter of the way across.
        assert_eq!(r.mouse_dragged(7, HALF, 180, 35), None);
        assert_eq!(
            r.mouse_dragged(7, shifted(HALF, 4, 0), 184, 35),
            Some(frame(-16, 25, 784, 625))
        );
        // Once only: the drag carries the window on.
        assert_eq!(r.mouse_dragged(7, frame(30, 25, 830, 625), 230, 35), None);
        // Dropped at its own size under the cursor: nothing more to do.
        assert_eq!(r.mouse_up(7, frame(300, 200, 1100, 800), 500, 210), None);
        assert!(!r.tracks(7));
    }

    #[test]
    fn a_drag_that_did_not_take_the_size_is_settled_on_the_drop() {
        let r = placed_half();
        r.mouse_down(7, HALF);
        r.mouse_dragged(7, shifted(HALF, 4, 0), 184, 35);
        // The application kept its size through the drag.
        assert_eq!(
            r.mouse_up(7, shifted(HALF, 400, 0), 580, 35),
            Some(frame(380, 25, 1180, 625))
        );
        assert!(!r.tracks(7));
    }

    #[test]
    fn a_click_keeps_the_window_remembered() {
        let r = placed_half();
        r.mouse_down(7, HALF);
        assert_eq!(r.mouse_up(7, HALF, 180, 35), None);
        assert!(r.tracks(7));
        // Windows winwin does not remember, and a button that came up on
        // another window, are left alone.
        assert!(!r.mouse_down(8, HALF));
        assert_eq!(r.mouse_dragged(8, shifted(HALF, 4, 0), 184, 35), None);
        assert_eq!(r.mouse_up(8, shifted(HALF, 4, 0), 184, 35), None);
        assert!(r.tracks(7));
    }

    #[test]
    fn resizing_a_window_by_hand_forgets_it() {
        let screen = frame(0, 0, 1440, 900);
        // By its right edge, which leaves the corner where it was.
        let r = placed_half();
        r.mouse_down(7, HALF);
        assert_eq!(r.mouse_dragged(7, frame(0, 25, 760, 900), 760, 400), None);
        assert_eq!(r.mouse_up(7, frame(0, 25, 800, 900), 800, 400), None);
        assert_eq!(r.reset(7, HALF, vec![screen]), None);
        // By its left edge, which moves it too.
        let r = placed_half();
        r.mouse_down(7, HALF);
        assert_eq!(r.mouse_dragged(7, frame(40, 25, 720, 900), 40, 400), None);
        assert!(!r.tracks(7));
        // Without news of the drag on the way.
        let r = placed_half();
        r.mouse_down(7, HALF);
        assert_eq!(r.mouse_up(7, frame(0, 25, 800, 900), 800, 400), None);
        assert!(!r.tracks(7));
        // Or with no drag at all: the window is not the size winwin made it
        // when the button goes down on it.
        let r = placed_half();
        assert!(!r.mouse_down(7, frame(0, 25, 800, 900)));
        assert!(!r.tracks(7));
    }

    #[test]
    fn a_winwin_started_again_takes_back_what_was_remembered() {
        let path = std::env::temp_dir().join(format!("winwin-marks-{}", std::process::id()));
        let path_text: String = path.to_string_lossy().into();
        let _ = std::fs::remove_file(&path);
        let r = Resident::new();
        r.remember_in(path_text.clone(), vec![]);
        r.placed(7, "100-5".into(), BEFORE, HALF);
        r.placed(8, "100-5".into(), BEFORE, HALF);
        r.placed(9, "200-6".into(), BEFORE, HALF);
        let screen = frame(0, 0, 1440, 900);

        let again = Resident::new();
        again.remember_in(
            path_text.clone(),
            vec![
                OpenWindow {
                    window: 7,
                    owner: "100-5".into(),
                },
                // The same number, now another application's window.
                OpenWindow {
                    window: 9,
                    owner: "300-7".into(),
                },
            ],
        );
        assert_eq!(again.reset(7, HALF, vec![screen]), Some(BEFORE));
        assert_eq!(again.reset(8, HALF, vec![screen]), None);
        assert_eq!(again.reset(9, HALF, vec![screen]), None);
        // What was given back or has closed is gone from the file too.
        let third = Resident::new();
        third.remember_in(
            path_text,
            vec![OpenWindow {
                window: 7,
                owner: "100-5".into(),
            }],
        );
        assert!(!third.tracks(7));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn the_file_of_what_is_remembered_skips_what_it_cannot_read() {
        let mut sizes = Sizes::default();
        let kept = sizes.placed(7, BEFORE.into(), false, POINTS_DPI, HALF.into());
        sizes.placed(8, BEFORE.into(), false, POINTS_DPI, HALF.into());
        let owners = HashMap::from([(7, "100-5".to_owned())]);
        let text = marks::write(&sizes, &owners);
        // A window without an application is not written.
        assert_eq!(text.lines().count(), 1);
        // Nor read back: a line without one, one whose values are not
        // winwin's, and one that is not a window's at all.
        let values = text.trim_end().strip_suffix(" 100-5").unwrap();
        let without_owner = values.replacen('7', "9", 1);
        let text = format!("{text}{without_owner}\n8 1 2 3 x\ngarbage\n");
        assert_eq!(marks::read(&text), vec![(7, "100-5".to_owned(), kept)]);
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
    fn the_icons_are_rgba() {
        let icon = app_icon_rgba(64);
        assert_eq!(icon.len(), 64 * 64 * 4);
        let at = |x: usize, y: usize| &icon[(y * 64 + x) * 4..][..4];
        // The margin is transparent; the tile's top middle is opaque and
        // off-white, red before alpha.
        assert_eq!(at(3, 32)[3], 0);
        let top = at(32, 8);
        assert_eq!(top[3], 0xFF);
        assert!(top[0] > 0xE0 && top[0] > top[2]);

        let glyph = menu_bar_icon_rgba(18);
        assert_eq!(glyph.len(), 18 * 18 * 4);
        // Alpha last: the opaque left half is [0, 0, 0, 255].
        let at = |x: usize, y: usize| &glyph[(y * 18 + x) * 4..][..4];
        assert_eq!(at(5, 13), [0, 0, 0, 0xFF]);
    }
}
