//! An application's menu bar as the settings window lists it: each item
//! with the titles that lead to it and the shortcut it shows, and the search
//! that finds items by name. The macOS app reads the items through the
//! accessibility API; what is listed and found is decided here.
//!
//! Nothing here touches the operating system, so it is tested on every
//! platform.

use crate::keys::Combo;

/// What goes between the titles of a menu item's path as the window shows
/// it.
pub const SEPARATOR: &str = " › ";

/// An item of a menu bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    /// The titles from the menu bar's item down to this one.
    pub path: Vec<String>,
    /// The shortcut it shows, when meta-gesture can name its key.
    pub keys: Option<Combo>,
}

/// `path` as the window shows it: `ファイル › 閉じる`.
pub fn path_text(path: &[String]) -> String {
    path.join(SEPARATOR)
}

/// How well an item matches a search, best first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Match {
    /// The item's own title is the search.
    Title,
    /// Every word is in the item's own title.
    InTitle,
    /// Every word is somewhere in the path.
    InPath,
}

/// The items of `items` that `query` finds, by their places in `items`:
/// those whose path holds every word of it, ignoring case. Items whose own
/// title is the query come first, then those whose own title holds every
/// word, each in the menu's order. An empty query finds every item.
pub fn search(items: &[MenuItem], query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut found: Vec<(Match, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let title = item
                .path
                .last()
                .map(|t| t.to_lowercase())
                .unwrap_or_default();
            let path = path_text(&item.path).to_lowercase();
            let m = if !query.is_empty() && title.trim() == query {
                Match::Title
            } else if words.iter().all(|w| title.contains(w)) {
                Match::InTitle
            } else if words.iter().all(|w| path.contains(w)) {
                Match::InPath
            } else {
                return None;
            };
            Some((m, i))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &[&str]) -> MenuItem {
        MenuItem {
            path: path.iter().map(|t| t.to_string()).collect(),
            keys: None,
        }
    }

    fn items() -> Vec<MenuItem> {
        vec![
            item(&["ファイル", "新規タブ"]),
            item(&["ファイル", "タブを閉じる"]),
            item(&["タブ", "右側のタブを閉じる"]),
            item(&["タブ", "固定"]),
            item(&["履歴", "戻る"]),
            item(&["表示", "開発 / 管理", "JavaScript Console"]),
        ]
    }

    #[test]
    fn shows_the_path_with_its_titles() {
        assert_eq!(path_text(&items()[2].path), "タブ › 右側のタブを閉じる");
    }

    #[test]
    fn finds_items_by_their_titles_and_menus() {
        let items = items();
        // Its own title first, then those in a menu of that name.
        assert_eq!(search(&items, "タブ"), [0, 1, 2, 3]);
        assert_eq!(search(&items, "閉じる"), [1, 2]);
        // Every word, in any order, and case does not matter.
        assert_eq!(search(&items, "閉じる 右側"), [2]);
        assert_eq!(search(&items, "javascript 表示"), [5]);
        assert_eq!(search(&items, "戻る"), [4]);
        assert_eq!(search(&items, "進む"), Vec::<usize>::new());
        assert_eq!(search(&items, "  "), [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn an_item_named_as_the_search_comes_first() {
        let items = vec![
            item(&["表示", "タブを閉じる前に確認"]),
            item(&["ファイル", "タブを閉じる"]),
            item(&["タブを閉じる", "すべて"]),
        ];
        assert_eq!(search(&items, "タブを閉じる"), [1, 0, 2]);
    }
}
