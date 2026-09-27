//! Giving a window its own size back when it is dragged away from where a
//! shortcut put it, the way Windows does for a snapped window.
//!
//! winwin remembers the size a window had before the first shortcut moved
//! it, and forgets it as soon as the window is somewhere winwin did not put
//! it: resized by hand, snapped or maximized by Windows, moved by the
//! application itself. Pressing shortcuts one after another keeps the size
//! from before the first one.

use std::collections::HashMap;

use crate::layout::Rect;

struct Entry {
    /// The window's outer size before winwin first placed it.
    width: i32,
    height: i32,
    /// The DPI of the monitor it was on then, to scale the size on a
    /// monitor with another one.
    dpi: u32,
    /// Where winwin last put it.
    placed: Rect,
    /// A drag started while the window was still where winwin put it.
    dragging: bool,
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
                entry.dragging = false;
            }
            _ => {
                self.windows.insert(
                    window,
                    Entry {
                        width: before.width(),
                        height: before.height(),
                        dpi,
                        placed: after,
                        dragging: false,
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
    pub fn drag_started(&mut self, window: isize, current: Rect, resizing: bool) {
        let Some(entry) = self.windows.get_mut(&window) else {
            return;
        };
        if resizing || entry.placed != current {
            self.windows.remove(&window);
        } else {
            entry.dragging = true;
        }
    }

    /// The user let go of `window` at `current`, with the cursor at
    /// `cursor`, on a monitor of `dpi`. Returns where to put the window at
    /// its own size, unless Windows has just snapped or maximized it.
    /// Either way winwin is done with the window.
    pub fn drag_ended(
        &mut self,
        window: isize,
        current: Rect,
        dpi: u32,
        cursor: (i32, i32),
        arranged: bool,
    ) -> Option<Rect> {
        if !self.windows.get(&window)?.dragging {
            return None;
        }
        let entry = self.windows.remove(&window)?;
        if arranged {
            return None;
        }
        let scale = |v: i32| (f64::from(v) * f64::from(dpi) / f64::from(entry.dpi)).round() as i32;
        Some(restored(
            scale(entry.width),
            scale(entry.height),
            current,
            cursor,
        ))
    }

    /// Forgets the windows for which `alive` says no.
    pub fn retain(&mut self, mut alive: impl FnMut(isize) -> bool) {
        self.windows.retain(|&window, _| alive(window));
    }
}

/// A `width` × `height` window in place of `current`, holding on to the
/// cursor where it held the title bar: at the same share of the width, and
/// the same distance from the top.
fn restored(width: i32, height: i32, current: Rect, cursor: (i32, i32)) -> Rect {
    let share = if current.width() > 0 {
        (f64::from(cursor.0 - current.left) / f64::from(current.width())).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let left = cursor.0 - (share * f64::from(width)).round() as i32;
    Rect {
        left,
        top: current.top,
        right: left + width,
        bottom: current.top + height,
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

    #[test]
    fn dragging_a_placed_window_gives_its_size_back_under_the_cursor() {
        let mut sizes = Sizes::default();
        sizes.placed(W, ORIGINAL, 96, LEFT_HALF);
        sizes.drag_started(W, LEFT_HALF, false);
        // Held at a quarter of the width, 10px below the top; dropped
        // 500px to the right.
        let dropped = rect(500, 0, 1460, 1040);
        let back = sizes.drag_ended(W, dropped, 96, (740, 10), false);
        // 800 × 600, with the cursor still at a quarter of the width.
        assert_eq!(back, Some(rect(540, 0, 1340, 600)));
    }

    #[test]
    fn a_drag_gives_the_size_back_once() {
        let mut sizes = Sizes::default();
        sizes.placed(W, ORIGINAL, 96, LEFT_HALF);
        sizes.drag_started(W, LEFT_HALF, false);
        assert!(
            sizes
                .drag_ended(W, LEFT_HALF, 96, (10, 10), false)
                .is_some()
        );
        sizes.drag_started(W, LEFT_HALF, false);
        assert_eq!(sizes.drag_ended(W, LEFT_HALF, 96, (10, 10), false), None);
    }

    #[test]
    fn shortcuts_in_a_row_keep_the_size_from_before_the_first() {
        let mut sizes = Sizes::default();
        let quarter = rect(0, 0, 960, 520);
        sizes.placed(W, ORIGINAL, 96, LEFT_HALF);
        sizes.placed(W, LEFT_HALF, 96, quarter);
        sizes.drag_started(W, quarter, false);
        let back = sizes.drag_ended(W, quarter, 96, (0, 0), false).unwrap();
        assert_eq!((back.width(), back.height()), (800, 600));
    }

    #[test]
    fn a_window_moved_elsewhere_in_between_starts_over() {
        let mut sizes = Sizes::default();
        let elsewhere = rect(50, 50, 650, 450);
        sizes.placed(W, ORIGINAL, 96, LEFT_HALF);
        // Moved by hand or by the application, then placed again.
        sizes.placed(W, elsewhere, 96, LEFT_HALF);
        sizes.drag_started(W, LEFT_HALF, false);
        let back = sizes.drag_ended(W, LEFT_HALF, 96, (0, 0), false).unwrap();
        assert_eq!((back.width(), back.height()), (600, 400));
    }

    #[test]
    fn a_window_no_longer_where_it_was_put_is_left_alone() {
        let mut sizes = Sizes::default();
        sizes.placed(W, ORIGINAL, 96, LEFT_HALF);
        sizes.drag_started(W, rect(0, 0, 700, 1040), false);
        assert_eq!(sizes.drag_ended(W, LEFT_HALF, 96, (0, 0), false), None);
    }

    #[test]
    fn resizing_forgets_the_size() {
        let mut sizes = Sizes::default();
        sizes.placed(W, ORIGINAL, 96, LEFT_HALF);
        sizes.drag_started(W, LEFT_HALF, true);
        assert_eq!(sizes.drag_ended(W, LEFT_HALF, 96, (0, 0), false), None);
        sizes.drag_started(W, LEFT_HALF, false);
        assert_eq!(sizes.drag_ended(W, LEFT_HALF, 96, (0, 0), false), None);
    }

    #[test]
    fn a_window_snapped_or_maximized_on_drop_keeps_that() {
        let mut sizes = Sizes::default();
        sizes.placed(W, ORIGINAL, 96, LEFT_HALF);
        sizes.drag_started(W, LEFT_HALF, false);
        assert_eq!(sizes.drag_ended(W, LEFT_HALF, 96, (0, 0), true), None);
        // And winwin is done with it.
        sizes.drag_started(W, LEFT_HALF, false);
        assert_eq!(sizes.drag_ended(W, LEFT_HALF, 96, (0, 0), false), None);
    }

    #[test]
    fn a_drop_without_a_start_does_nothing() {
        let mut sizes = Sizes::default();
        sizes.placed(W, ORIGINAL, 96, LEFT_HALF);
        assert_eq!(sizes.drag_ended(W, LEFT_HALF, 96, (0, 0), false), None);
        assert_eq!(sizes.drag_ended(2, LEFT_HALF, 96, (0, 0), false), None);
    }

    #[test]
    fn another_placement_after_the_drag_started_needs_a_new_drag() {
        let mut sizes = Sizes::default();
        sizes.placed(W, ORIGINAL, 96, LEFT_HALF);
        sizes.drag_started(W, LEFT_HALF, false);
        sizes.placed(W, LEFT_HALF, 96, rect(0, 0, 960, 520));
        assert_eq!(sizes.drag_ended(W, LEFT_HALF, 96, (0, 0), false), None);
    }

    #[test]
    fn the_size_follows_the_dpi_of_the_monitor_it_lands_on() {
        let mut sizes = Sizes::default();
        sizes.placed(W, ORIGINAL, 96, LEFT_HALF);
        sizes.drag_started(W, LEFT_HALF, false);
        let back = sizes.drag_ended(W, LEFT_HALF, 144, (0, 0), false).unwrap();
        assert_eq!((back.width(), back.height()), (1200, 900));
    }

    #[test]
    fn the_cursor_stays_on_the_window_even_off_its_edge() {
        // A move started from the keyboard can leave the cursor anywhere.
        assert_eq!(
            restored(800, 600, LEFT_HALF, (2000, 10)),
            rect(1200, 0, 2000, 600)
        );
        assert_eq!(
            restored(800, 600, LEFT_HALF, (-50, 10)),
            rect(-50, 0, 750, 600)
        );
    }

    #[test]
    fn windows_that_are_gone_are_forgotten() {
        let mut sizes = Sizes::default();
        sizes.placed(1, ORIGINAL, 96, LEFT_HALF);
        sizes.placed(2, ORIGINAL, 96, LEFT_HALF);
        sizes.retain(|w| w == 2);
        sizes.drag_started(1, LEFT_HALF, false);
        sizes.drag_started(2, LEFT_HALF, false);
        assert_eq!(sizes.drag_ended(1, LEFT_HALF, 96, (0, 0), false), None);
        assert!(sizes.drag_ended(2, LEFT_HALF, 96, (0, 0), false).is_some());
    }
}
