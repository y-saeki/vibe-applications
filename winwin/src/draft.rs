//! A shortcut as the settings window edits it: every field as the user left
//! it, including ones that do not make a valid shortcut yet. Turning a draft
//! into a [`Shortcut`] is where the fields are checked.
//!
//! Nothing here touches Win32, so it is tested on every platform.

use std::collections::BTreeSet;

use crate::config::{Config, Shortcut};
use crate::hotkey::{self, Hotkey, Keys};
use crate::layout::{self, Anchor, Placement, Ratio, Rect};

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    /// The strokes as recorded; empty until a shortcut is chosen.
    pub keys: Vec<Hotkey>,
    pub anchor: Anchor,
    pub width: String,
    pub height: String,
}

impl Draft {
    /// What 追加 starts from.
    pub fn new() -> Draft {
        Draft {
            keys: Vec::new(),
            anchor: Anchor::Center,
            width: "1/2".into(),
            height: "1/2".into(),
        }
    }

    pub fn from_shortcut(s: &Shortcut) -> Draft {
        let p = &s.placement;
        Draft {
            keys: s.keys.strokes().to_vec(),
            anchor: p.anchor,
            width: p.width.to_string(),
            height: p.height.to_string(),
        }
    }

    pub fn placement(&self) -> Result<Placement, String> {
        let field =
            |label: &str, text: &str| text.parse::<Ratio>().map_err(|e| format!("{label}: {e}"));
        Ok(Placement {
            anchor: self.anchor,
            width: field("幅", &self.width)?,
            height: field("高さ", &self.height)?,
        })
    }

    pub fn to_shortcut(&self) -> Result<Shortcut, String> {
        if self.keys.is_empty() {
            return Err("ショートカットを設定してください".into());
        }
        let keys = Keys::new(self.keys.clone()).map_err(|e| e.to_string())?;
        Ok(Shortcut {
            keys,
            placement: self.placement()?,
        })
    }

    /// A screen of `size` shrunk into `bounds`, and where on it this row puts
    /// a window; `None` for the window while the fields do not make a
    /// placement yet.
    pub fn picture(
        &self,
        size: (i32, i32),
        bounds: Rect,
        anchor: Anchor,
    ) -> Option<(Rect, Option<Rect>)> {
        let screen = layout::miniature(size, bounds, anchor)?;
        let window = self.placement().ok().map(|p| p.resolve(screen));
        Some((screen, window))
    }

    /// Where the row puts a window, in words: 左 1/2 × 1.
    pub fn placement_text(&self) -> String {
        format!(
            "{} {} × {}",
            self.anchor.label(),
            self.width.trim(),
            self.height.trim()
        )
    }

    /// The row in one line, as errors name it: the keys first, then where
    /// they put the window.
    pub fn list_text(&self) -> String {
        let keys = if self.keys.is_empty() {
            "(未設定)".to_string()
        } else {
            hotkey::strokes_text(&self.keys)
        };
        format!("{keys}    {}", self.placement_text())
    }
}

/// The keys held with a click on a row in the list, or with the arrow key
/// that moved to it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Click {
    pub shift: bool,
    pub ctrl: bool,
}

/// The list the settings window edits: the rows, which of them are
/// selected, and whether anything has changed since the window opened. Each
/// row carries an id, the key its line in the list is kept by: it goes with
/// the row when the row is dragged, and stays with the line when 上へ・下へ
/// swap two rows.
#[derive(Clone, Debug, Default)]
pub struct Editor {
    rows: Vec<(u64, Draft)>,
    next_id: u64,
    /// The ids of the selected rows.
    selected: BTreeSet<u64>,
    /// Where a Shift+click range starts: the row last clicked without Shift.
    anchor: Option<u64>,
    dirty: bool,
}

impl Editor {
    /// Starts on the first row, if there is one.
    pub fn new(rows: Vec<Draft>) -> Editor {
        let next_id = rows.len() as u64;
        let mut editor = Editor {
            rows: (0..).zip(rows).collect(),
            next_id,
            ..Editor::default()
        };
        editor.select(0);
        editor
    }

    pub fn rows(&self) -> impl Iterator<Item = (u64, &Draft)> {
        self.rows.iter().map(|(id, d)| (*id, d))
    }

    pub fn is_selected(&self, id: u64) -> bool {
        self.selected.contains(&id)
    }

    /// Where the selected rows are in the list, in list order.
    pub fn selection(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, (id, _))| self.selected.contains(id))
            .map(|(i, _)| i)
            .collect()
    }

    /// The selected row when it is the only one: the row whose fields the
    /// window shows and edits.
    pub fn current(&self) -> Option<usize> {
        match self.selection()[..] {
            [i] => Some(i),
            _ => None,
        }
    }

    pub fn selected(&self) -> Option<&Draft> {
        self.current().map(|i| &self.rows[i].1)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Selects row `index` alone; anything past the end is ignored.
    pub fn select(&mut self, index: usize) {
        self.click(index, Click::default());
    }

    /// Row `index` was clicked. On its own that selects the row alone. With
    /// Ctrl it adds the row to the selection, or takes it out. With Shift it
    /// selects the rows from the one last clicked without Shift to this one,
    /// in place of the selection, or with Ctrl as well, on top of it.
    /// Anything past the end is ignored.
    pub fn click(&mut self, index: usize, click: Click) {
        let Some(&(id, _)) = self.rows.get(index) else {
            return;
        };
        let anchor = self
            .anchor
            .and_then(|a| self.rows.iter().position(|(r, _)| *r == a));
        match anchor {
            Some(from) if click.shift => {
                if !click.ctrl {
                    self.selected.clear();
                }
                let range = &self.rows[from.min(index)..=from.max(index)];
                self.selected.extend(range.iter().map(|(r, _)| *r));
            }
            _ => {
                if !click.ctrl {
                    self.selected.clear();
                    self.selected.insert(id);
                } else if !self.selected.remove(&id) {
                    self.selected.insert(id);
                }
                self.anchor = Some(id);
            }
        }
    }

    /// Changes the selected row. Controls report the value they were just
    /// given as well as the user's edits, so a change that leaves the row
    /// as it was does not count as one.
    pub fn edit(&mut self, f: impl FnOnce(&mut Draft)) {
        let Some(i) = self.current() else {
            return;
        };
        let row = &mut self.rows[i].1;
        let before = row.clone();
        f(row);
        if *row != before {
            self.dirty = true;
        }
    }

    /// Puts `rows` in below the last selected row, or at the end, and
    /// selects them in place of what was.
    fn insert(&mut self, rows: Vec<Draft>) {
        let at = self.selection().last().map_or(self.rows.len(), |i| i + 1);
        let first = self.next_id;
        self.next_id += rows.len() as u64;
        self.rows.splice(at..at, (first..).zip(rows));
        self.selected = (first..self.next_id).collect();
        self.anchor = Some(first);
        self.dirty = true;
    }

    /// 追加: a new row below the selection.
    pub fn add(&mut self) {
        self.insert(vec![Draft::new()]);
    }

    /// 複製: copies of the selected rows, in their order, below the last of
    /// them. A copy shares its row's shortcut and so takes a turn after it.
    pub fn duplicate(&mut self) {
        let copies: Vec<Draft> = self
            .selection()
            .into_iter()
            .map(|i| self.rows[i].1.clone())
            .collect();
        if !copies.is_empty() {
            self.insert(copies);
        }
    }

    /// 削除: removes the selected rows and selects the one that took the
    /// place of the first of them, or the new last row.
    pub fn delete(&mut self) {
        let Some(&first) = self.selection().first() else {
            return;
        };
        self.rows.retain(|(id, _)| !self.selected.contains(id));
        self.selected.clear();
        self.anchor = None;
        self.dirty = true;
        if let Some(last) = self.rows.len().checked_sub(1) {
            self.select(first.min(last));
        }
    }

    pub fn can_move(&self, up: bool) -> bool {
        self.current()
            .is_some_and(|i| moved(i, self.rows.len(), up).is_some())
    }

    /// 上へ・下へ: moves the selected row, which is also its turn among the
    /// rows that share its shortcut. The ids stay where they are and the two
    /// rows trade contents, so the list redraws two lines in place instead of
    /// taking one out and putting it back.
    pub fn move_selected(&mut self, up: bool) {
        let Some(from) = self.current() else {
            return;
        };
        if let Some(to) = moved(from, self.rows.len(), up) {
            let (a, b) = self.rows.split_at_mut(from.max(to));
            std::mem::swap(&mut a[from.min(to)].1, &mut b[0].1);
            self.select(to);
            self.dirty = true;
        }
    }

    /// Puts the rows in the order of `ids`, as the list has been dragged
    /// into. Each row keeps its id, since the list has already moved its
    /// lines, and so stays selected or not. An order that is not the rows'
    /// ids rearranged is ignored.
    pub fn reorder(&mut self, ids: &[u64]) {
        let before: Vec<u64> = self.rows.iter().map(|(id, _)| *id).collect();
        if before == ids {
            return;
        }
        let (mut old, mut new) = (before, ids.to_vec());
        old.sort_unstable();
        new.sort_unstable();
        if old != new {
            return;
        }
        let mut rows = std::mem::take(&mut self.rows);
        self.rows = ids
            .iter()
            .map(|id| {
                let i = rows.iter().position(|(r, _)| r == id).unwrap();
                rows.swap_remove(i)
            })
            .collect();
        self.dirty = true;
    }

    /// Checks every row and makes the config to save. The first row that
    /// does not make a shortcut is selected alone, and the error names it.
    pub fn build_config(&mut self) -> Result<Config, String> {
        let shortcuts: Result<Vec<Shortcut>, (usize, String)> = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, (_, row))| {
                row.to_shortcut()
                    .map_err(|e| (i, format!("{}: {e}", row.list_text())))
            })
            .collect();
        match shortcuts {
            Ok(shortcuts) => Ok(Config {
                shortcuts,
                ..Config::default()
            }),
            Err((i, message)) => {
                self.select(i);
                Err(message)
            }
        }
    }

    /// The rows that make a shortcut as they stand, in list order, without
    /// saving anything: what the test window answers to. A row still being
    /// filled in is left out rather than holding up the rest.
    pub fn trial_config(&self) -> Config {
        Config {
            shortcuts: self
                .rows
                .iter()
                .filter_map(|(_, row)| row.to_shortcut().ok())
                .collect(),
            ..Config::default()
        }
    }
}

/// Where row `index` of `len` lands when moved one place up or down, or
/// `None` when it is already at that end. Rows that share a shortcut take
/// turns in list order, so this is how that order is changed.
pub fn moved(index: usize, len: usize, up: bool) -> Option<usize> {
    if index >= len {
        return None;
    }
    if up {
        index.checked_sub(1)
    } else {
        Some(index + 1).filter(|&i| i < len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::hotkey::{MOD_ALT, MOD_CONTROL, MOD_SHIFT};

    #[test]
    fn a_saved_shortcut_comes_back_unchanged() {
        for s in Config::defaults().shortcuts {
            assert_eq!(Draft::from_shortcut(&s).to_shortcut().unwrap().keys, s.keys);
            let placement = Draft::from_shortcut(&s).to_shortcut().unwrap().placement;
            assert_eq!(placement.anchor, s.placement.anchor);
            assert_eq!(placement.width, s.placement.width);
            assert_eq!(placement.height, s.placement.height);
        }
    }

    #[test]
    fn a_new_entry_needs_a_shortcut_before_it_saves() {
        let mut d = Draft::new();
        assert_eq!(
            d.to_shortcut().unwrap_err(),
            "ショートカットを設定してください"
        );
        d.keys = vec![Hotkey {
            modifiers: MOD_CONTROL | MOD_ALT,
            vk: 0x41,
        }];
        assert_eq!(d.to_shortcut().unwrap().keys.to_string(), "Ctrl+Alt+A");
        d.keys.push(Hotkey {
            modifiers: MOD_CONTROL | MOD_ALT,
            vk: 0x26,
        });
        assert_eq!(d.to_shortcut().unwrap().keys.to_string(), "Ctrl+Alt+A, Up");
        d.keys[1].modifiers = MOD_CONTROL;
        assert!(d.to_shortcut().unwrap_err().contains("修飾キー"));
    }

    #[test]
    fn says_which_field_is_wrong() {
        let mut d = Draft::from_shortcut(&Config::defaults().shortcuts[0]);
        d.height = "tall".into();
        let e = d.to_shortcut().unwrap_err();
        assert!(e.starts_with("高さ:"), "{e}");

        // The file still reads percent from earlier versions; the window
        // takes only ratios.
        d.height = "50%".into();
        assert!(d.to_shortcut().unwrap_err().starts_with("高さ:"));

        d = Draft::from_shortcut(&Config::defaults().shortcuts[0]);
        d.keys[0].modifiers = MOD_SHIFT;
        assert!(d.to_shortcut().unwrap_err().contains("Ctrl・Alt・Win"));
    }

    #[test]
    fn list_lines_show_the_shortcut_and_the_placement() {
        let d = Draft::from_shortcut(&Config::defaults().shortcuts[0]);
        assert_eq!(d.placement_text(), "左 1/2 × 1");
        assert_eq!(d.list_text(), "Ctrl+Alt+Left    左 1/2 × 1");
        assert_eq!(Draft::new().list_text(), "(未設定)    中央 1/2 × 1/2");
        let mut d = d;
        d.keys.push("Ctrl+Alt+Up".parse().unwrap());
        assert_eq!(d.list_text(), "Ctrl+Alt+Left, Up    左 1/2 × 1");
    }

    fn editor() -> Editor {
        Editor::new(
            Config::defaults()
                .shortcuts
                .iter()
                .take(3)
                .map(Draft::from_shortcut)
                .collect(),
        )
    }

    fn ids(e: &Editor) -> Vec<u64> {
        e.rows().map(|(id, _)| id).collect()
    }

    #[test]
    fn the_editor_starts_on_the_first_row() {
        let e = editor();
        assert_eq!(e.current(), Some(0));
        assert!(!e.is_dirty());
    }

    #[test]
    fn an_edit_that_changes_nothing_is_not_a_change() {
        let mut e = editor();
        let width = e.selected().unwrap().width.clone();
        e.edit(|d| d.width = width);
        assert!(!e.is_dirty());
        e.edit(|d| d.width = "2/3".into());
        assert!(e.is_dirty());
        assert_eq!(e.selected().unwrap().width, "2/3");
    }

    #[test]
    fn rows_are_added_duplicated_and_deleted_below_the_selection() {
        let mut e = editor();
        e.select(1);
        e.duplicate();
        assert_eq!(ids(&e), [0, 1, 3, 2]);
        assert_eq!(e.current(), Some(2));
        assert_eq!(e.rows().nth(2).unwrap().1, e.rows().nth(1).unwrap().1);
        e.add();
        assert_eq!(ids(&e), [0, 1, 3, 4, 2]);
        assert_eq!(e.selected(), Some(&Draft::new()));
        e.delete();
        assert_eq!(ids(&e), [0, 1, 3, 2]);
        assert_eq!(e.current(), Some(3));
        e.delete();
        assert_eq!(e.current(), Some(2));
        assert!(e.is_dirty());
    }

    fn shift() -> Click {
        Click {
            shift: true,
            ctrl: false,
        }
    }

    fn ctrl() -> Click {
        Click {
            shift: false,
            ctrl: true,
        }
    }

    #[test]
    fn clicks_with_shift_and_ctrl_select_several_rows() {
        let mut e = Editor::new(vec![Draft::new(); 5]);
        e.select(1);
        e.click(3, shift());
        assert_eq!(e.selection(), [1, 2, 3]);
        // The range starts from the row last clicked without Shift.
        e.click(0, shift());
        assert_eq!(e.selection(), [0, 1]);
        e.click(4, ctrl());
        assert_eq!(e.selection(), [0, 1, 4]);
        e.click(
            2,
            Click {
                shift: true,
                ctrl: true,
            },
        );
        assert_eq!(e.selection(), [0, 1, 2, 3, 4]);
        e.click(3, ctrl());
        assert_eq!(e.selection(), [0, 1, 2, 4]);
        e.click(9, Click::default());
        assert_eq!(e.selection(), [0, 1, 2, 4]);
        e.click(2, Click::default());
        assert_eq!(e.selection(), [2]);
    }

    #[test]
    fn several_selected_rows_have_no_fields_to_edit_and_do_not_move() {
        let mut e = editor();
        e.click(1, shift());
        assert_eq!((e.current(), e.selected()), (None, None));
        e.edit(|d| d.width = "1".into());
        assert!(!e.can_move(true) && !e.can_move(false));
        e.move_selected(false);
        assert!(!e.is_dirty());
    }

    #[test]
    fn several_rows_are_duplicated_together_below_the_last_of_them() {
        let mut e = editor();
        e.click(2, ctrl());
        e.duplicate();
        assert_eq!(ids(&e), [0, 1, 2, 3, 4]);
        assert_eq!(
            keys(&e),
            [
                "Ctrl+Alt+Left",
                "Ctrl+Alt+Right",
                "Ctrl+Alt+Up",
                "Ctrl+Alt+Left",
                "Ctrl+Alt+Up"
            ]
        );
        // The copies are selected, and a Shift+click ranges from the first.
        assert_eq!(e.selection(), [3, 4]);
        e.click(1, shift());
        assert_eq!(e.selection(), [1, 2, 3]);
    }

    #[test]
    fn several_rows_are_deleted_together() {
        let mut e = editor();
        e.add();
        e.select(1);
        e.click(2, shift());
        assert_eq!(ids(&e), [0, 3, 1, 2]);
        e.delete();
        assert_eq!(ids(&e), [0, 2]);
        assert_eq!(e.current(), Some(1));
    }

    #[test]
    fn deleting_the_last_row_leaves_nothing_selected() {
        let mut e = Editor::new(vec![Draft::new()]);
        e.delete();
        assert_eq!(e.current(), None);
        e.edit(|d| d.width = "1".into());
        e.add();
        assert_eq!(e.current(), Some(0));
    }

    fn keys(e: &Editor) -> Vec<String> {
        e.rows()
            .map(|(_, d)| d.to_shortcut().unwrap().keys.to_string())
            .collect()
    }

    #[test]
    fn the_selected_row_moves_and_stays_selected() {
        let mut e = editor();
        assert!(!e.can_move(true));
        e.move_selected(false);
        assert_eq!(keys(&e), ["Ctrl+Alt+Right", "Ctrl+Alt+Left", "Ctrl+Alt+Up"]);
        assert_eq!(e.current(), Some(1));
        assert!(e.is_dirty());
        // The lines stay put and trade what they show.
        assert_eq!(ids(&e), [0, 1, 2]);
        e.move_selected(true);
        assert_eq!(keys(&e), ["Ctrl+Alt+Left", "Ctrl+Alt+Right", "Ctrl+Alt+Up"]);
        assert_eq!(e.current(), Some(0));
        e.select(2);
        assert!(!e.can_move(false));
        e.select(7);
        assert_eq!(e.current(), Some(2));
    }

    #[test]
    fn dragged_rows_take_the_new_order_with_their_ids() {
        let mut e = editor();
        e.select(1);
        e.reorder(&[2, 0, 1]);
        assert_eq!(ids(&e), [2, 0, 1]);
        assert_eq!(keys(&e), ["Ctrl+Alt+Up", "Ctrl+Alt+Left", "Ctrl+Alt+Right"]);
        // Still on Ctrl+Alt+Right, which is now last.
        assert_eq!(e.current(), Some(2));
        assert!(e.is_dirty());
        e.click(0, ctrl());
        assert_eq!(e.selection(), [0, 2]);
        e.reorder(&[0, 1, 2]);
        assert_eq!(e.selection(), [1, 2]);
    }

    #[test]
    fn a_drag_that_changes_nothing_is_not_a_change() {
        let mut e = editor();
        e.reorder(&[0, 1, 2]);
        assert!(!e.is_dirty());
        // Not the rows' ids: missing one, an unknown one, one twice.
        for order in [&[0, 1][..], &[0, 1, 5], &[0, 1, 1]] {
            e.reorder(order);
            assert_eq!(ids(&e), [0, 1, 2]);
        }
        assert!(!e.is_dirty());
    }

    #[test]
    fn saving_stops_at_the_first_row_that_is_not_a_shortcut() {
        let mut e = editor();
        assert_eq!(e.build_config().unwrap().shortcuts.len(), 3);
        e.add();
        e.select(0);
        let error = e.build_config().unwrap_err();
        assert_eq!(e.current(), Some(1));
        assert_eq!(
            error,
            "(未設定)    中央 1/2 × 1/2: ショートカットを設定してください"
        );
    }

    #[test]
    fn the_test_window_takes_the_rows_that_make_a_shortcut() {
        let mut e = editor();
        e.add();
        e.select(0);
        e.edit(|d| d.width = "5".into());
        let keys: Vec<String> = e
            .trial_config()
            .shortcuts
            .iter()
            .map(|s| s.keys.to_string())
            .collect();
        assert_eq!(keys, ["Ctrl+Alt+Right", "Ctrl+Alt+Up"]);
        // Nothing is selected or saved on the way.
        assert_eq!(e.current(), Some(0));
    }

    #[test]
    fn rows_move_within_the_list() {
        assert_eq!(moved(1, 3, true), Some(0));
        assert_eq!(moved(1, 3, false), Some(2));
        assert_eq!(moved(0, 3, true), None);
        assert_eq!(moved(2, 3, false), None);
        assert_eq!(moved(3, 3, true), None);
    }

    #[test]
    fn pictures_show_the_screen_and_the_window_on_it() {
        let bounds = Rect {
            left: 0,
            top: 0,
            right: 160,
            bottom: 100,
        };
        let d = Draft::from_shortcut(&Config::defaults().shortcuts[0]);
        let (screen, window) = d.picture((1920, 1080), bounds, Anchor::Center).unwrap();
        assert_eq!((screen.width(), screen.height()), (160, 90));
        assert_eq!(screen.top, 5);
        let window = window.unwrap();
        assert_eq!((window.left, window.top), (screen.left, screen.top));
        assert_eq!((window.width(), window.height()), (80, 90));

        let mut d = Draft::new();
        d.width = "5".into();
        let (_, window) = d.picture((1920, 1080), bounds, Anchor::Center).unwrap();
        assert_eq!(window, None);
    }
}
