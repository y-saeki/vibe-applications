//! Giving a window its own size back when it is dragged away from where a
//! shortcut put it, the way Windows does for a snapped window.
//!
//! winwin remembers the size a window had before the first shortcut moved
//! it, and forgets it as soon as the window is something winwin did not make
//! it: resized by hand, snapped or maximized by Windows, resized by the
//! application itself. Pressing shortcuts one after another keeps the size
//! from before the first one.
//!
//! The size comes back as soon as the window first moves in a drag, not
//! when the drag starts: a click on the title bar starts one too. Only the
//! size changes then, never the position, which belongs to the system's move
//! loop until the drop. The drop settles whatever the loop made of it.

use std::collections::HashMap;

use crate::layout::Rect;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    None,
    /// A move started while the window was as winwin made it.
    Started,
    /// It has moved and got its size back; the cursor held the title bar at
    /// this share of the width.
    Moved {
        share: f64,
    },
}

struct Entry {
    /// The window's outer size before winwin first placed it.
    width: i32,
    height: i32,
    /// The DPI of the monitor it was on then, to scale the size on a
    /// monitor with another one.
    dpi: u32,
    /// Where winwin last put it.
    placed: Rect,
    drag: Drag,
}

impl Entry {
    fn size_at(&self, dpi: u32) -> (i32, i32) {
        let scale = |v: i32| (f64::from(v) * f64::from(dpi) / f64::from(self.dpi)).round() as i32;
        (scale(self.width), scale(self.height))
    }
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
}

impl Sizes {
    /// A shortcut moved `window` from `before` to `after`, on a monitor of
    /// `dpi`. When the window was still where an earlier shortcut put it,
    /// the size from before that one is kept.
    pub fn placed(&mut self, window: isize, before: Rect, dpi: u32, after: Rect) {
        match self.windows.get_mut(&window) {
            Some(entry) if entry.placed == before => {
                entry.placed = after;
                entry.drag = Drag::None;
            }
            _ => {
                self.windows.insert(
                    window,
                    Entry {
                        width: before.width(),
                        height: before.height(),
                        dpi,
                        placed: after,
                        drag: Drag::None,
                    },
                );
            }
        }
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
        if resizing || !same_size(entry.placed, current) {
            self.windows.remove(&window);
            return false;
        }
        entry.drag = Drag::Started;
        true
    }

    /// `window`, being dragged, is now at `current`, with the cursor at
    /// `cursor`, on a monitor of `dpi`. The first time it has left the place
    /// winwin put it, returns the size to give it right away.
    pub fn moved(
        &mut self,
        window: isize,
        current: Rect,
        dpi: u32,
        cursor: (i32, i32),
    ) -> Option<(i32, i32)> {
        let entry = self.windows.get_mut(&window)?;
        if entry.drag != Drag::Started || same_corner(entry.placed, current) {
            return None;
        }
        if !same_size(entry.placed, current) {
            self.windows.remove(&window);
            return None;
        }
        entry.drag = Drag::Moved {
            share: share(current, cursor),
        };
        Some(entry.size_at(dpi))
    }

    /// The user let go of `window` at `current`, with the cursor at
    /// `cursor`, on a monitor of `dpi`. Returns where the window has to go
    /// to end up at its own size under the cursor, if it is not there yet.
    ///
    /// A window back where winwin put it was only clicked, or the drag was
    /// cancelled: it keeps its place, and winwin keeps its size. Otherwise
    /// winwin is done with the window, and leaves it alone if Windows has
    /// just snapped or maximized it.
    pub fn drag_ended(
        &mut self,
        window: isize,
        current: Rect,
        dpi: u32,
        cursor: (i32, i32),
        arranged: bool,
    ) -> Option<Rect> {
        let entry = self.windows.get_mut(&window)?;
        let drag = std::mem::replace(&mut entry.drag, Drag::None);
        if drag == Drag::None {
            return None;
        }
        if !arranged && same_corner(entry.placed, current) {
            return (current != entry.placed).then_some(entry.placed);
        }
        let entry = self.windows.remove(&window)?;
        if arranged {
            return None;
        }
        let (width, height) = entry.size_at(dpi);
        let under_cursor = current.width() == width
            && current.height() == height
            && (current.left..=current.right).contains(&cursor.0);
        if under_cursor {
            return None;
        }
        let share = match drag {
            Drag::Moved { share } => share,
            _ => share(current, cursor),
        };
        let left = cursor.0 - (share * f64::from(width)).round() as i32;
        Some(Rect {
            left,
            top: current.top,
            right: left + width,
            bottom: current.top + height,
        })
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
        sizes.placed(W, ORIGINAL, 96, LEFT_HALF);
        sizes
    }

    #[test]
    fn the_size_comes_back_as_soon_as_the_window_moves() {
        let mut sizes = placed();
        assert!(sizes.drag_started(W, LEFT_HALF, false));
        assert_eq!(sizes.moved(W, LEFT_HALF, 96, (240, 10)), None);
        let first_step = shifted(LEFT_HALF, 5, 0);
        assert_eq!(sizes.moved(W, first_step, 96, (245, 10)), Some((800, 600)));
        // Once is enough.
        assert_eq!(sizes.moved(W, first_step, 96, (245, 10)), None);
    }

    #[test]
    fn a_drop_where_the_loop_kept_the_size_leaves_the_window_alone() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        sizes.moved(W, shifted(LEFT_HALF, 5, 0), 96, (245, 10));
        // The loop moved the 800 × 600 window along; the cursor is on it.
        let dropped = rect(505, 200, 1305, 800);
        assert_eq!(sizes.drag_ended(W, dropped, 96, (745, 210), false), None);
        assert!(!sizes.tracks(W));
    }

    #[test]
    fn a_drop_where_the_loop_put_the_old_size_back_is_settled() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        // Held at a quarter of the width.
        sizes.moved(W, shifted(LEFT_HALF, 5, 0), 96, (245, 10));
        let dropped = shifted(LEFT_HALF, 500, 0);
        let back = sizes.drag_ended(W, dropped, 96, (740, 10), false);
        assert_eq!(back, Some(rect(540, 0, 1340, 600)));
    }

    #[test]
    fn a_drop_with_the_cursor_off_the_window_puts_it_under_the_cursor() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        // Held near the right end, further right than the old width.
        sizes.moved(W, shifted(LEFT_HALF, 5, 0), 96, (869, 10));
        let dropped = rect(505, 0, 1305, 600);
        let back = sizes.drag_ended(W, dropped, 96, (1369, 10), false);
        assert_eq!(back, Some(rect(649, 0, 1449, 600)));
    }

    #[test]
    fn a_drop_without_news_of_the_move_still_gives_the_size_back() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        let dropped = shifted(LEFT_HALF, 500, 0);
        let back = sizes.drag_ended(W, dropped, 96, (740, 10), false);
        assert_eq!(back, Some(rect(540, 0, 1340, 600)));
    }

    #[test]
    fn a_click_on_the_title_bar_changes_nothing() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        assert_eq!(sizes.moved(W, LEFT_HALF, 96, (240, 10)), None);
        assert_eq!(sizes.drag_ended(W, LEFT_HALF, 96, (240, 10), false), None);
        // Still remembered for a real drag later.
        sizes.drag_started(W, LEFT_HALF, false);
        assert!(
            sizes
                .moved(W, shifted(LEFT_HALF, 5, 0), 96, (0, 0))
                .is_some()
        );
    }

    #[test]
    fn a_cancelled_drag_puts_the_window_back_as_placed() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        sizes.moved(W, shifted(LEFT_HALF, 5, 0), 96, (245, 10));
        // Esc: the loop moved it back, at the size winwin gave it.
        let cancelled = rect(0, 0, 800, 600);
        assert_eq!(
            sizes.drag_ended(W, cancelled, 96, (245, 10), false),
            Some(LEFT_HALF)
        );
        assert!(sizes.tracks(W));
    }

    #[test]
    fn shortcuts_in_a_row_keep_the_size_from_before_the_first() {
        let mut sizes = placed();
        let quarter = rect(0, 0, 960, 520);
        sizes.placed(W, LEFT_HALF, 96, quarter);
        sizes.drag_started(W, quarter, false);
        assert_eq!(
            sizes.moved(W, shifted(quarter, 5, 0), 96, (0, 0)),
            Some((800, 600))
        );
    }

    #[test]
    fn a_window_moved_elsewhere_in_between_starts_over() {
        let mut sizes = placed();
        let elsewhere = rect(50, 50, 650, 450);
        // Moved by hand or by the application, then placed again.
        sizes.placed(W, elsewhere, 96, LEFT_HALF);
        sizes.drag_started(W, LEFT_HALF, false);
        assert_eq!(
            sizes.moved(W, shifted(LEFT_HALF, 5, 0), 96, (0, 0)),
            Some((600, 400))
        );
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
        sizes.drag_started(W, LEFT_HALF, false);
        sizes.moved(W, shifted(LEFT_HALF, 5, 0), 96, (245, 10));
        assert_eq!(sizes.drag_ended(W, LEFT_HALF, 96, (0, 0), true), None);
        assert!(!sizes.tracks(W));
    }

    #[test]
    fn nothing_happens_outside_a_drag() {
        let mut sizes = placed();
        assert_eq!(sizes.moved(W, shifted(LEFT_HALF, 5, 0), 96, (0, 0)), None);
        assert_eq!(sizes.drag_ended(W, LEFT_HALF, 96, (0, 0), false), None);
        assert_eq!(sizes.drag_ended(2, LEFT_HALF, 96, (0, 0), false), None);
        assert!(!sizes.drag_started(2, LEFT_HALF, false));
    }

    #[test]
    fn another_placement_during_the_drag_needs_a_new_drag() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        let quarter = rect(0, 0, 960, 520);
        sizes.placed(W, LEFT_HALF, 96, quarter);
        assert_eq!(sizes.moved(W, shifted(quarter, 5, 0), 96, (0, 0)), None);
    }

    #[test]
    fn a_window_resized_by_itself_during_the_drag_is_forgotten() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        assert_eq!(sizes.moved(W, rect(5, 0, 900, 1040), 96, (0, 0)), None);
        assert!(!sizes.tracks(W));
    }

    #[test]
    fn the_size_follows_the_dpi_of_the_monitor() {
        let mut sizes = placed();
        sizes.drag_started(W, LEFT_HALF, false);
        assert_eq!(
            sizes.moved(W, shifted(LEFT_HALF, 5, 0), 144, (0, 0)),
            Some((1200, 900))
        );
        // Dropped on a 96 DPI monitor after the application scaled itself
        // there: 800 × 600 again, under the cursor.
        let dropped = rect(2000, 0, 2800, 600);
        assert_eq!(sizes.drag_ended(W, dropped, 96, (2100, 10), false), None);
    }

    #[test]
    fn windows_that_are_gone_are_forgotten() {
        let mut sizes = Sizes::default();
        sizes.placed(1, ORIGINAL, 96, LEFT_HALF);
        sizes.placed(2, ORIGINAL, 96, LEFT_HALF);
        sizes.retain(|w| w == 2);
        assert!(!sizes.tracks(1));
        assert!(sizes.tracks(2));
    }
}
