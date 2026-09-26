//! A shortcut as the settings window edits it: every field as the user left
//! it, including ones that do not make a valid shortcut yet. Turning a draft
//! into a [`Shortcut`] is where the fields are checked.
//!
//! Nothing here touches Win32, so it is tested on every platform.

use crate::config::{Config, Shortcut};
use crate::hotkey::Hotkey;
use crate::layout::{self, Anchor, Placement, Ratio, Rect};

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    /// MOD_* flags and a virtual-key code; `vk` is 0 until a key is chosen.
    pub modifiers: u32,
    pub vk: u32,
    pub anchor: Anchor,
    pub width: String,
    pub height: String,
}

impl Draft {
    /// What 追加 starts from.
    pub fn new() -> Draft {
        Draft {
            modifiers: 0,
            vk: 0,
            anchor: Anchor::Center,
            width: "1/2".into(),
            height: "1/2".into(),
        }
    }

    pub fn from_shortcut(s: &Shortcut) -> Draft {
        let p = &s.placement;
        Draft {
            modifiers: s.keys.modifiers,
            vk: s.keys.vk,
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
        if self.vk == 0 {
            return Err("ショートカットを設定してください".into());
        }
        let keys = Hotkey::new(self.modifiers, self.vk).map_err(|e| e.to_string())?;
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

    /// The line in the list of shortcuts: the keys first, so that entries
    /// sharing them read alike, then where they put the window.
    pub fn list_text(&self) -> String {
        let keys = if self.vk == 0 {
            "(未設定)".to_string()
        } else {
            Hotkey {
                modifiers: self.modifiers,
                vk: self.vk,
            }
            .to_string()
        };
        format!(
            "{keys}    {} {} × {}",
            self.anchor.label(),
            self.width.trim(),
            self.height.trim()
        )
    }
}

/// The list the settings window edits: the rows, which one is selected, and
/// whether anything has changed since the window opened. Each row carries an
/// id that stays with it when it moves, for the list to follow it by.
#[derive(Clone, Debug, Default)]
pub struct Editor {
    rows: Vec<(u64, Draft)>,
    next_id: u64,
    current: Option<usize>,
    dirty: bool,
}

impl Editor {
    /// Starts on the first row, if there is one.
    pub fn new(rows: Vec<Draft>) -> Editor {
        let current = (!rows.is_empty()).then_some(0);
        let next_id = rows.len() as u64;
        Editor {
            rows: (0..).zip(rows).collect(),
            next_id,
            current,
            dirty: false,
        }
    }

    pub fn rows(&self) -> impl Iterator<Item = (u64, &Draft)> {
        self.rows.iter().map(|(id, d)| (*id, d))
    }

    pub fn current(&self) -> Option<usize> {
        self.current
    }

    pub fn selected(&self) -> Option<&Draft> {
        self.current.map(|i| &self.rows[i].1)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Selects row `index`; anything past the end is ignored.
    pub fn select(&mut self, index: usize) {
        if index < self.rows.len() {
            self.current = Some(index);
        }
    }

    /// Changes the selected row. Controls report the value they were just
    /// given as well as the user's edits, so a change that leaves the row
    /// as it was does not count as one.
    pub fn edit(&mut self, f: impl FnOnce(&mut Draft)) {
        let Some(i) = self.current else {
            return;
        };
        let row = &mut self.rows[i].1;
        let before = row.clone();
        f(row);
        if *row != before {
            self.dirty = true;
        }
    }

    fn insert(&mut self, row: Draft) {
        let i = self.current.map_or(self.rows.len(), |i| i + 1);
        self.rows.insert(i, (self.next_id, row));
        self.next_id += 1;
        self.current = Some(i);
        self.dirty = true;
    }

    /// 追加: a new row below the selected one.
    pub fn add(&mut self) {
        self.insert(Draft::new());
    }

    /// 複製: a copy of the selected row below it, which shares its shortcut
    /// and so takes the next turn.
    pub fn duplicate(&mut self) {
        if let Some(row) = self.selected().cloned() {
            self.insert(row);
        }
    }

    /// 削除: removes the selected row and selects the one that took its
    /// place, or the new last row.
    pub fn delete(&mut self) {
        let Some(i) = self.current else {
            return;
        };
        self.rows.remove(i);
        self.dirty = true;
        self.current = if self.rows.is_empty() {
            None
        } else {
            Some(i.min(self.rows.len() - 1))
        };
    }

    pub fn can_move(&self, up: bool) -> bool {
        self.current
            .is_some_and(|i| moved(i, self.rows.len(), up).is_some())
    }

    /// 上へ・下へ: moves the selected row, which is also its turn among the
    /// rows that share its shortcut.
    pub fn move_selected(&mut self, up: bool) {
        let Some(from) = self.current else {
            return;
        };
        if let Some(to) = moved(from, self.rows.len(), up) {
            self.rows.swap(from, to);
            self.current = Some(to);
            self.dirty = true;
        }
    }

    /// Checks every row and makes the config to save. The first row that
    /// does not make a shortcut is selected, and the error names it.
    pub fn build_config(&mut self) -> Result<Config, String> {
        let mut config = Config::default();
        for (i, (_, row)) in self.rows.iter().enumerate() {
            match row.to_shortcut() {
                Ok(s) => config.shortcuts.push(s),
                Err(e) => {
                    let message = format!("{}: {e}", row.list_text());
                    self.current = Some(i);
                    return Err(message);
                }
            }
        }
        Ok(config)
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
        d.modifiers = MOD_CONTROL | MOD_ALT;
        d.vk = 0x41;
        assert_eq!(d.to_shortcut().unwrap().keys.to_string(), "Ctrl+Alt+A");
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
        d.modifiers = MOD_SHIFT;
        assert!(d.to_shortcut().unwrap_err().contains("Ctrl・Alt・Win"));
    }

    #[test]
    fn list_lines_show_the_shortcut_and_the_placement() {
        let d = Draft::from_shortcut(&Config::defaults().shortcuts[0]);
        assert_eq!(d.list_text(), "Ctrl+Alt+Left    左 1/2 × 1");
        assert_eq!(Draft::new().list_text(), "(未設定)    中央 1/2 × 1/2");
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
        assert_eq!(Editor::new(Vec::new()).current(), None);
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

    #[test]
    fn deleting_the_last_row_leaves_nothing_selected() {
        let mut e = Editor::new(vec![Draft::new()]);
        e.delete();
        assert_eq!(e.current(), None);
        e.edit(|d| d.width = "1".into());
        e.add();
        assert_eq!(e.current(), Some(0));
    }

    #[test]
    fn the_selected_row_moves_and_stays_selected() {
        let mut e = editor();
        assert!(!e.can_move(true));
        e.move_selected(false);
        assert_eq!(ids(&e), [1, 0, 2]);
        assert_eq!(e.current(), Some(1));
        e.select(2);
        assert!(!e.can_move(false));
        e.select(7);
        assert_eq!(e.current(), Some(2));
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
