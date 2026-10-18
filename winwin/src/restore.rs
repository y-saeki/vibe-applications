//! Giving a window its own size back when it is dragged away from where a
//! shortcut put it, the way Windows does for a snapped window.
//!
//! winwin remembers the size a window had before the first shortcut moved
//! it, and forgets it as soon as the window is something winwin did not make
//! it: resized by hand, snapped or maximized by Windows, resized by the
//! application itself. Pressing shortcuts one after another keeps the size
//! from before the first one.
//!
//! What it remembers also goes on the window itself (`Remembered::pack`), so
//! that a winwin started again picks it up, and goes away with the window.
//!
//! A reset shortcut puts a window back where it was before the first
//! shortcut moved it, at the size it had then, and winwin forgets it.
//!
//! The size comes back as soon as the window first moves in a drag, not
//! when the drag starts: a click on the title bar starts one too. The
//! system's move loop puts the size it started with back on every step, so
//! winwin ends that loop, gives the window its size under the cursor, and
//! has the system start a new one while the button is still down. Should
//! any of that not happen, the drop settles it.

use std::collections::HashMap;

use crate::layout::Rect;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    None,
    /// A move started while the window was as winwin made it.
    Started,
    /// It has moved, and winwin has asked its move loop to stop. The cursor
    /// held the title bar at `share` of the width, `dy` below the top.
    Stopping {
        share: f64,
        dy: i32,
    },
    /// It has its size back, and a new move loop carries on the drag.
    Resumed {
        share: f64,
    },
}

/// What to do with a window whose move loop has ended.
#[derive(Debug, PartialEq)]
pub enum Ended {
    Leave,
    /// Put it here.
    Put(Rect),
    /// Put it here and start a new move loop: the button is still down.
    Resume(Rect),
}

/// What winwin remembers of a window it has placed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Remembered {
    /// The window's outer rectangle before winwin first placed it; for a
    /// maximized window, the one it has when it is not.
    pub before: Rect,
    /// Whether it was maximized then.
    pub maximized: bool,
    /// The DPI of the monitor it was on then, to scale the size on a
    /// monitor with another one.
    pub dpi: u32,
    /// Where winwin last put it.
    pub placed: Rect,
}

/// Where a reset puts a window back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reset {
    pub rect: Rect,
    /// Maximize it there, as it was.
    pub maximized: bool,
}

/// Marks a packed value as winwin's, and as this layout of it.
const PACKED: u64 = 1 << 48;

impl Remembered {
    fn size_at(&self, dpi: u32) -> (i32, i32) {
        let scale = |v: i32| (f64::from(v) * f64::from(dpi) / f64::from(self.dpi)).round() as i32;
        (scale(self.before.width()), scale(self.before.height()))
    }

    /// Three values of 64 bits, to keep on the window: the size and DPI,
    /// where winwin put it, and where the window was before. None when
    /// something does not fit, which no screen of today comes near: a size
    /// or DPI over 65535, a coordinate outside ±32767.
    pub fn pack(&self) -> Option<[u64; 3]> {
        let unsigned = |v: i64| u16::try_from(v).ok().map(u64::from);
        let signed = |v: i32| i16::try_from(v).ok().map(|v| u64::from(v as u16));
        let size = unsigned(self.before.width().into())?
            | unsigned(self.before.height().into())? << 16
            | unsigned(self.dpi.into())? << 32
            | PACKED;
        let r = self.placed;
        let placed = signed(r.left)?
            | signed(r.top)? << 16
            | signed(r.right)? << 32
            | signed(r.bottom)? << 48;
        let origin = signed(self.before.left)?
            | signed(self.before.top)? << 16
            | u64::from(self.maximized) << 32
            | PACKED;
        Some([size, placed, origin])
    }

    /// The other way round. None for anything `pack` did not write,
    /// including what a winwin from before resets left, without the third.
    pub fn unpack([size, placed, origin]: [u64; 3]) -> Option<Self> {
        if size & !0xFFFF_FFFF_FFFF != PACKED || origin & !0x1_FFFF_FFFF != PACKED {
            return None;
        }
        let unsigned = |shift: u32| ((size >> shift) & 0xFFFF) as i32;
        let signed = |word: u64, shift: u32| i32::from((word >> shift) as u16 as i16);
        let (left, top) = (signed(origin, 0), signed(origin, 16));
        let remembered = Self {
            before: Rect {
                left,
                top,
                right: left + unsigned(0),
                bottom: top + unsigned(16),
            },
            maximized: origin & (1 << 32) != 0,
            dpi: unsigned(32) as u32,
            placed: Rect {
                left: signed(placed, 0),
                top: signed(placed, 16),
                right: signed(placed, 32),
                bottom: signed(placed, 48),
            },
        };
        let sane = remembered.before.width() > 0
            && remembered.before.height() > 0
            && remembered.dpi > 0
            && remembered.placed.width() > 0
            && remembered.placed.height() > 0;
        sane.then_some(remembered)
    }
}

struct Entry {
    kept: Remembered,
    drag: Drag,
}

fn same_size(a: Rect, b: Rect) -> bool {
    a.width() == b.width() && a.height() == b.height()
}

fn same_corner(a: Rect, b: Rect) -> bool {
    a.left == b.left && a.top == b.top
}

/// Where the cursor holds `rect` across its width, from 0 to 1.
fn share(rect: Rect, cursor: (i32, i32)) -> f64 {
    if rect.width() > 0 {
        (f64::from(cursor.0 - rect.left) / f64::from(rect.width())).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// The windows winwin has placed, by handle.
#[derive(Default)]
pub struct Sizes {
    windows: HashMap<isize, Entry>,
    /// Windows forgotten since `take_forgotten`, to take what winwin left on
    /// them away too.
    forgotten: Vec<isize>,
}

impl Sizes {
    /// A shortcut moved `window` from `before`, `maximized` or not, to
    /// `after`, on a monitor of `dpi`. When the window was still where an
    /// earlier shortcut put it, the size and place from before that one are
    /// kept. Returns what there now is to remember of the window.
    pub fn placed(
        &mut self,
        window: isize,
        before: Rect,
        maximized: bool,
        dpi: u32,
        after: Rect,
    ) -> Remembered {
        match self.windows.get_mut(&window) {
            Some(entry) if entry.kept.placed == before => {
                entry.kept.placed = after;
                entry.drag = Drag::None;
                entry.kept
            }
            _ => {
                let kept = Remembered {
                    before,
                    maximized,
                    dpi,
                    placed: after,
                };
                self.windows.insert(
                    window,
                    Entry {
                        kept,
                        drag: Drag::None,
                    },
                );
                kept
            }
        }
    }

    /// Picks up what an earlier winwin remembered of `window`, unless this
    /// one already knows it.
    pub fn adopt(&mut self, window: isize, kept: Remembered) {
        self.windows.entry(window).or_insert(Entry {
            kept,
            drag: Drag::None,
        });
    }

    /// The windows forgotten since the last call.
    pub fn take_forgotten(&mut self) -> Vec<isize> {
        std::mem::take(&mut self.forgotten)
    }

    fn forget(&mut self, window: isize) {
        if self.windows.remove(&window).is_some() {
            self.forgotten.push(window);
        }
    }

    /// A reset shortcut was pressed on `window`, which is at `current` on a
    /// monitor of `dpi`: where it was before winwin first placed it, and
    /// winwin forgets it. When `shown` says no monitor shows that place any
    /// more, the window gets its size back where it is instead. None for a
    /// window winwin does not remember.
    pub fn reset(
        &mut self,
        window: isize,
        current: Rect,
        dpi: u32,
        shown: impl FnOnce(Rect) -> bool,
    ) -> Option<Reset> {
        let kept = self.windows.get(&window)?.kept;
        self.forget(window);
        let rect = if shown(kept.before) {
            kept.before
        } else {
            let (width, height) = kept.size_at(dpi);
            Rect {
                left: current.left,
                top: current.top,
                right: current.left + width,
                bottom: current.top + height,
            }
        };
        Some(Reset {
            rect,
            maximized: kept.maximized,
        })
    }

    /// Whether winwin has placed `window` and still remembers its size.
    pub fn tracks(&self, window: isize) -> bool {
        self.windows.contains_key(&window)
    }

    /// The user started to move or resize `window`, which is at `current`.
    /// Returns whether to watch it move. The position is not compared: the
    /// news can arrive after the window has already moved a little.
    pub fn drag_started(&mut self, window: isize, current: Rect, resizing: bool) -> bool {
        let Some(entry) = self.windows.get_mut(&window) else {
            return false;
        };
        // The move loop winwin started itself, at the size it gave back:
        // nothing to watch, the drop settles it.
        if let Drag::Resumed { .. } = entry.drag {
            return false;
        }
        if resizing || !same_size(entry.kept.placed, current) {
            self.forget(window);
            return false;
        }
        entry.drag = Drag::Started;
        true
    }

    /// `window`, being dragged, is now at `current`, with the cursor at
    /// `cursor`. The first time it has left the place winwin put it, returns
    /// true: time to stop the move loop.
    pub fn moved(&mut self, window: isize, current: Rect, cursor: (i32, i32)) -> bool {
        let Some(entry) = self.windows.get_mut(&window) else {
            return false;
        };
        if entry.drag != Drag::Started || same_corner(entry.kept.placed, current) {
            return false;
        }
        if !same_size(entry.kept.placed, current) {
            self.forget(window);
            return false;
        }
        entry.drag = Drag::Stopping {
            share: share(current, cursor),
            dy: cursor.1 - current.top,
        };
        true
    }

    /// The move loop of `window` ended with the window at `current`, the
    /// cursor at `cursor` on a monitor of `dpi`, and the button `down` or
    /// not.
    ///
    /// The loop winwin stopped, with the button still down, resumes at the
    /// window's own size under the cursor. Otherwise the user let go: the
    /// window goes to its own size under the cursor, if it is not there
    /// yet, and winwin is done with it, unless Windows has just snapped or
    /// maximized it. A window back where winwin put it without having moved
    /// was only clicked, or the drag was cancelled: it keeps its place, and
    /// winwin keeps its size.
    pub fn drag_ended(
        &mut self,
        window: isize,
        current: Rect,
        dpi: u32,
        cursor: (i32, i32),
        arranged: bool,
        down: bool,
    ) -> Ended {
        let Some(entry) = self.windows.get_mut(&window) else {
            return Ended::Leave;
        };
        let drag = std::mem::replace(&mut entry.drag, Drag::None);
        let (width, height) = entry.kept.size_at(dpi);
        let under_cursor = |share: f64, top: i32| {
            let left = cursor.0 - (share * f64::from(width)).round() as i32;
            Rect {
                left,
                top,
                right: left + width,
                bottom: top + height,
            }
        };
        match drag {
            Drag::None => return Ended::Leave,
            Drag::Stopping { share, dy } if down && !arranged => {
                entry.drag = Drag::Resumed { share };
                return Ended::Resume(under_cursor(share, cursor.1 - dy));
            }
            Drag::Started if !arranged && same_corner(entry.kept.placed, current) => {
                return match current != entry.kept.placed {
                    true => Ended::Put(entry.kept.placed),
                    false => Ended::Leave,
                };
            }
            _ => {}
        }
        self.forget(window);
        if arranged {
            return Ended::Leave;
        }
        let there = current.width() == width
            && current.height() == height
            && (current.left..=current.right).contains(&cursor.0);
        if there {
            return Ended::Leave;
        }
        match drag {
            // Stopped, but let go before it could resume: the loop may have
            // put the window back where the drag started.
            Drag::Stopping { share, dy } => Ended::Put(under_cursor(share, cursor.1 - dy)),
            Drag::Resumed { share } => Ended::Put(under_cursor(share, current.top)),
            _ => Ended::Put(under_cursor(share(current, cursor), current.top)),
        }
    }

    /// Forgets the windows for which `alive` says no.
    pub fn retain(&mut self, mut alive: impl FnMut(isize) -> bool) {
        self.windows.retain(|&window, _| alive(window));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }

    fn shifted(r: Rect, dx: i32, dy: i32) -> Rect {
        rect(r.left + dx, r.top + dy, r.right + dx, r.bottom + dy)
    }

    const W: isize = 1;
    const ORIGINAL: Rect = Rect {
        left: 100,
        top: 100,
        right: 900,
        bottom: 700,
    };
    const LEFT_HALF: Rect = Rect {
        left: 0,
        top: 0,
        right: 960,
        bottom: 1040,
    };

    fn placed() -> Sizes {
        let mut sizes = Sizes::default();
        sizes.placed(W, ORIGINAL, false, 96, LEFT_HALF);
        sizes
    }

    /// Starts a drag and moves the window by 5px, the cursor holding the
    /// title bar a quarter of the way across, 10px down.
    fn dragged(sizes: &mut Sizes) {
        assert!(sizes.drag_started(W, LEFT_HALF, false));
        assert!(!sizes.moved(W, LEFT_HALF, (240, 10)));
        assert!(sizes.moved(W, shifted(LEFT_HALF, 5, 0), (245, 10)));
    }

    #[test]
    fn the_first_move_stops_the_loop_once() {
        let mut sizes = placed();
        dragged(&mut sizes);
        assert!(!sizes.moved(W, shifted(LEFT_HALF, 6, 0), (246, 10)));
    }

    #[test]
    fn a_stopped_loop_resumes_at_the_old_size_under_the_cursor() {
        let mut sizes = placed();
        dragged(&mut sizes);
        // The loop may have put the window back where the drag started.
        let ended = sizes.drag_ended(W, LEFT_HALF, 96, (250, 12), false, true);
        assert_eq!(ended, Ended::Resume(rect(50, 2, 850, 602)));
        // The new loop is winwin's own: not watched, settled on the drop.
        assert!(!sizes.drag_started(W, rect(50, 2, 850, 602), false));
        let dropped = rect(500, 300, 1300, 900);
        assert_eq!(
            sizes.drag_ended(W, dropped, 96, (700, 310), false, false),
            Ended::Leave
        );
        assert!(!sizes.tracks(W));
    }

    #[test]
    fn a_resumed_drag_dropped_at_the_wrong_size_is_settled() {
        let mut sizes = placed();
        dragged(&mut sizes);
        sizes.drag_ended(W, LEFT_HALF, 96, (250, 12), false, true);
        sizes.drag_started(W, rect(50, 2, 850, 602), false);
        let dropped = shifted(LEFT_HALF, 500, 0);
        assert_eq!(
            sizes.drag_ended(W, dropped, 96, (740, 10), false, false),
            Ended::Put(rect(540, 0, 1340, 600))
        );
    }

    #[test]
    fn a_stopped_loop_let_go_of_puts_the_window_under_the_cursor() {
        let mut sizes = placed();
        dragged(&mut sizes);
        assert_eq!(
            sizes.drag_ended(W, LEFT_HALF, 96, (740, 310), false, false),
            Ended::Put(rect(540, 300, 1340, 900))
        );
        assert!(!sizes.tracks(W));
    }

    #[test]
    fn a_drop_without_news_of_the_move_still_gives_the_size_back() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        let dropped = shifted(LEFT_HALF, 500, 0);
        assert_eq!(
            sizes.drag_ended(W, dropped, 96, (740, 10), false, false),
            Ended::Put(rect(540, 0, 1340, 600))
        );
    }

    #[test]
    fn a_click_on_the_title_bar_changes_nothing() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        assert!(!sizes.moved(W, LEFT_HALF, (240, 10)));
        assert_eq!(
            sizes.drag_ended(W, LEFT_HALF, 96, (240, 10), false, false),
            Ended::Leave
        );
        // Still remembered for a real drag later.
        sizes.drag_started(W, LEFT_HALF, false);
        assert!(sizes.moved(W, shifted(LEFT_HALF, 5, 0), (0, 0)));
    }

    #[test]
    fn a_window_moved_back_without_news_keeps_its_place() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        // Esc, at a size the application changed on the way.
        assert_eq!(
            sizes.drag_ended(W, rect(0, 0, 800, 600), 96, (245, 10), false, false),
            Ended::Put(LEFT_HALF)
        );
        assert!(sizes.tracks(W));
    }

    /// The size the window resumes at, after a drag stopped with the
    /// cursor at the window's top-left corner.
    fn resumed_size(sizes: &mut Sizes, from: Rect, dpi: u32) -> Option<(i32, i32)> {
        sizes.drag_started(W, from, false);
        let moved = shifted(from, 5, 0);
        if !sizes.moved(W, moved, (moved.left, moved.top)) {
            return None;
        }
        match sizes.drag_ended(W, from, dpi, (0, 0), false, true) {
            Ended::Resume(r) => Some((r.width(), r.height())),
            _ => None,
        }
    }

    #[test]
    fn shortcuts_in_a_row_keep_the_size_from_before_the_first() {
        let mut sizes = placed();
        let quarter = rect(0, 0, 960, 520);
        sizes.placed(W, LEFT_HALF, false, 96, quarter);
        assert_eq!(resumed_size(&mut sizes, quarter, 96), Some((800, 600)));
    }

    #[test]
    fn a_window_moved_elsewhere_in_between_starts_over() {
        let mut sizes = placed();
        let elsewhere = rect(50, 50, 650, 450);
        // Moved by hand or by the application, then placed again.
        sizes.placed(W, elsewhere, false, 96, LEFT_HALF);
        assert_eq!(resumed_size(&mut sizes, LEFT_HALF, 96), Some((600, 400)));
    }

    #[test]
    fn the_size_follows_the_dpi_of_the_monitor() {
        let mut sizes = placed();
        assert_eq!(resumed_size(&mut sizes, LEFT_HALF, 144), Some((1200, 900)));
    }

    #[test]
    fn the_start_may_come_after_the_window_has_moved_a_little() {
        let mut sizes = placed();
        assert!(sizes.drag_started(W, shifted(LEFT_HALF, 3, 1), false));
    }

    #[test]
    fn a_window_of_another_size_is_forgotten() {
        let mut sizes = placed();
        assert!(!sizes.drag_started(W, rect(0, 0, 700, 1040), false));
        assert!(!sizes.tracks(W));
    }

    #[test]
    fn resizing_forgets_the_size() {
        let mut sizes = placed();
        assert!(!sizes.drag_started(W, LEFT_HALF, true));
        assert!(!sizes.tracks(W));
    }

    #[test]
    fn a_window_snapped_or_maximized_on_drop_keeps_that() {
        let mut sizes = placed();
        dragged(&mut sizes);
        sizes.drag_ended(W, LEFT_HALF, 96, (250, 12), false, true);
        sizes.drag_started(W, rect(50, 2, 850, 602), false);
        assert_eq!(
            sizes.drag_ended(W, LEFT_HALF, 96, (0, 0), true, false),
            Ended::Leave
        );
        assert!(!sizes.tracks(W));
    }

    #[test]
    fn nothing_happens_outside_a_drag() {
        let mut sizes = placed();
        assert!(!sizes.moved(W, shifted(LEFT_HALF, 5, 0), (0, 0)));
        assert_eq!(
            sizes.drag_ended(W, LEFT_HALF, 96, (0, 0), false, true),
            Ended::Leave
        );
        assert_eq!(
            sizes.drag_ended(2, LEFT_HALF, 96, (0, 0), false, true),
            Ended::Leave
        );
        assert!(!sizes.drag_started(2, LEFT_HALF, false));
    }

    #[test]
    fn another_placement_during_the_drag_needs_a_new_drag() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        let quarter = rect(0, 0, 960, 520);
        sizes.placed(W, LEFT_HALF, false, 96, quarter);
        assert!(!sizes.moved(W, shifted(quarter, 5, 0), (0, 0)));
    }

    #[test]
    fn a_window_resized_by_itself_during_the_drag_is_forgotten() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        assert!(!sizes.moved(W, rect(5, 0, 900, 1040), (0, 0)));
        assert!(!sizes.tracks(W));
    }

    #[test]
    fn what_is_remembered_comes_back_from_the_window() {
        let kept = Remembered {
            before: rect(-1800, 100, -1000, 700),
            maximized: true,
            dpi: 144,
            placed: rect(-1928, -8, -952, 1040),
        };
        assert_eq!(Remembered::unpack(kept.pack().unwrap()), Some(kept));
        let kept = Remembered {
            maximized: false,
            ..kept
        };
        assert_eq!(Remembered::unpack(kept.pack().unwrap()), Some(kept));
    }

    #[test]
    fn what_does_not_fit_is_not_kept_on_the_window() {
        let kept = |width, left| Remembered {
            before: rect(0, 0, width, 600),
            maximized: false,
            dpi: 96,
            placed: rect(left, 0, left + 100, 100),
        };
        assert!(kept(70_000, 0).pack().is_none());
        assert!(kept(800, 40_000).pack().is_none());
        assert!(kept(800, -40_000).pack().is_none());
        let elsewhere = Remembered {
            before: rect(40_000, 0, 40_800, 600),
            ..kept(800, 0)
        };
        assert!(elsewhere.pack().is_none());
    }

    #[test]
    fn values_winwin_did_not_write_are_ignored() {
        assert_eq!(Remembered::unpack([0, 0, 0]), None);
        assert_eq!(Remembered::unpack([u64::MAX, 0, u64::MAX]), None);
        let [size, placed, origin] = Remembered {
            before: ORIGINAL,
            maximized: false,
            dpi: 96,
            placed: LEFT_HALF,
        }
        .pack()
        .unwrap();
        // Another layout of the same thing.
        assert_eq!(Remembered::unpack([size + (1 << 48), placed, origin]), None);
        assert_eq!(Remembered::unpack([size, placed, origin + (1 << 48)]), None);
        // A placed rectangle with no area.
        assert_eq!(Remembered::unpack([size, 0, origin]), None);
        // Left by a winwin from before resets, which wrote two.
        assert_eq!(Remembered::unpack([size, placed, 0]), None);
    }

    #[test]
    fn a_winwin_started_again_takes_over_what_was_remembered() {
        let mut first = placed();
        let kept = first.placed(W, LEFT_HALF, false, 96, rect(0, 0, 960, 520));
        let mut again = Sizes::default();
        again.adopt(W, kept);
        assert_eq!(
            resumed_size(&mut again, rect(0, 0, 960, 520), 96),
            Some((800, 600))
        );
    }

    #[test]
    fn adopting_does_not_replace_what_is_known() {
        let mut sizes = placed();
        sizes.adopt(
            W,
            Remembered {
                before: rect(0, 0, 100, 100),
                maximized: false,
                dpi: 96,
                placed: LEFT_HALF,
            },
        );
        assert_eq!(resumed_size(&mut sizes, LEFT_HALF, 96), Some((800, 600)));
    }

    #[test]
    fn a_reset_goes_back_to_before_the_first_shortcut_and_forgets() {
        let mut sizes = placed();
        let quarter = rect(0, 0, 960, 520);
        sizes.placed(W, LEFT_HALF, false, 96, quarter);
        let reset = sizes.reset(W, quarter, 96, |r| r == ORIGINAL);
        assert_eq!(
            reset,
            Some(Reset {
                rect: ORIGINAL,
                maximized: false
            })
        );
        assert!(!sizes.tracks(W));
        assert_eq!(sizes.take_forgotten(), vec![W]);
        // Nothing more to go back to.
        assert_eq!(sizes.reset(W, ORIGINAL, 96, |_| true), None);
    }

    #[test]
    fn a_reset_maximizes_a_window_that_was() {
        let mut sizes = Sizes::default();
        sizes.placed(W, ORIGINAL, true, 96, LEFT_HALF);
        let reset = sizes.reset(W, LEFT_HALF, 96, |_| true).unwrap();
        assert_eq!((reset.rect, reset.maximized), (ORIGINAL, true));
    }

    #[test]
    fn a_reset_where_no_monitor_shows_keeps_the_window_where_it_is() {
        let mut sizes = placed();
        let there = shifted(LEFT_HALF, 1920, 0);
        let reset = sizes.reset(W, there, 144, |_| false).unwrap();
        assert_eq!(reset.rect, rect(1920, 0, 3120, 900));
    }

    #[test]
    fn a_reset_on_a_window_winwin_has_not_placed_does_nothing() {
        let mut sizes = placed();
        assert_eq!(sizes.reset(2, LEFT_HALF, 96, |_| true), None);
        assert!(sizes.tracks(W));
    }

    #[test]
    fn a_winwin_started_again_resets_what_was_remembered() {
        let mut first = placed();
        let kept = first.placed(W, LEFT_HALF, false, 96, rect(0, 0, 960, 520));
        let mut again = Sizes::default();
        again.adopt(W, Remembered::unpack(kept.pack().unwrap()).unwrap());
        let reset = again.reset(W, rect(0, 0, 960, 520), 96, |_| true);
        assert_eq!(reset.map(|r| r.rect), Some(ORIGINAL));
    }

    #[test]
    fn forgotten_windows_are_reported_once() {
        let mut sizes = placed();
        sizes.placed(2, ORIGINAL, false, 96, LEFT_HALF);
        sizes.drag_started(W, LEFT_HALF, true);
        sizes.drag_started(2, rect(0, 0, 10, 10), false);
        assert_eq!(sizes.take_forgotten(), vec![W, 2]);
        assert_eq!(sizes.take_forgotten(), Vec::<isize>::new());
    }

    #[test]
    fn windows_that_are_gone_are_forgotten() {
        let mut sizes = Sizes::default();
        sizes.placed(1, ORIGINAL, false, 96, LEFT_HALF);
        sizes.placed(2, ORIGINAL, false, 96, LEFT_HALF);
        sizes.retain(|w| w == 2);
        assert!(!sizes.tracks(1));
        assert!(sizes.tracks(2));
    }
}
