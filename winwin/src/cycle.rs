//! Shortcuts that several entries share. Each distinct key combination is
//! registered once; pressing it again while its modifiers stay held steps to
//! the next entry that uses it, in the order of the config, and wraps around.
//! Letting go of the modifiers starts the next press from the first entry.
//!
//! Nothing here touches Win32, so it is tested on every platform.

use crate::config::Config;
use crate::hotkey::Hotkey;
use crate::layout::Placement;

/// One registered key combination and every entry that uses it.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub keys: Hotkey,
    pub placements: Vec<Placement>,
}

impl Binding {
    /// Whether pressing it again can lead somewhere else.
    pub fn cycles(&self) -> bool {
        self.placements.len() > 1
    }
}

/// The config's entries grouped by key combination, in the order each
/// combination first appears.
pub fn bindings(config: &Config) -> Vec<Binding> {
    let mut out: Vec<Binding> = Vec::new();
    for s in &config.shortcuts {
        match out.iter_mut().find(|b| b.keys == s.keys) {
            Some(b) => {
                b.placements.push(s.placement);
            }
            None => out.push(Binding {
                keys: s.keys,
                placements: vec![s.placement],
            }),
        }
    }
    out
}

/// Where the cycle stands: which binding was pressed last and which of its
/// entries that press chose.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cycle {
    last: Option<(usize, usize)>,
}

impl Cycle {
    /// Binding `binding`, which has `len` entries, was pressed. Returns the
    /// entry to apply.
    pub fn press(&mut self, binding: usize, len: usize) -> usize {
        let next = match self.last {
            Some((b, i)) if b == binding && len > 0 => (i + 1) % len,
            _ => 0,
        };
        self.last = Some((binding, next));
        next
    }

    /// The modifiers were let go: the next press starts over.
    pub fn reset(&mut self) {
        self.last = None;
    }

    /// The binding whose modifiers are being watched, if any.
    pub fn active(&self) -> Option<usize> {
        self.last.map(|(b, _)| b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_shared_keys() -> Config {
        let mut config = Config::defaults();
        // 左 2/3 (Ctrl+Alt+E) joins 左半分 on Ctrl+Alt+Left.
        let two_thirds = config
            .shortcuts
            .iter()
            .position(|s| s.keys.to_string() == "Ctrl+Alt+E")
            .unwrap();
        config.shortcuts[two_thirds].keys = config.shortcuts[0].keys;
        config
    }

    #[test]
    fn distinct_keys_are_one_binding_each() {
        let config = Config::defaults();
        let bindings = bindings(&config);
        assert_eq!(bindings.len(), config.shortcuts.len());
        assert!(bindings.iter().all(|b| !b.cycles()));
    }

    #[test]
    fn shared_keys_are_grouped_in_config_order() {
        let config = config_with_shared_keys();
        let bindings = bindings(&config);
        assert_eq!(bindings.len(), config.shortcuts.len() - 1);
        let left = &bindings[0];
        assert_eq!(left.keys.to_string(), "Ctrl+Alt+Left");
        assert!(left.cycles());
        assert_eq!(left.placements[0].width.value(), 50.0);
        assert_eq!(left.placements[1].width.value(), 200.0 / 3.0);
        // The others keep their order behind it.
        assert_eq!(bindings[1].keys.to_string(), "Ctrl+Alt+Right");
    }

    #[test]
    fn repeated_presses_step_through_and_wrap() {
        let mut cycle = Cycle::default();
        let picks: Vec<usize> = (0..4).map(|_| cycle.press(0, 3)).collect();
        assert_eq!(picks, [0, 1, 2, 0]);
    }

    #[test]
    fn letting_go_starts_over() {
        let mut cycle = Cycle::default();
        cycle.press(0, 3);
        assert_eq!(cycle.press(0, 3), 1);
        cycle.reset();
        assert_eq!(cycle.active(), None);
        assert_eq!(cycle.press(0, 3), 0);
    }

    #[test]
    fn another_shortcut_starts_its_own_cycle() {
        let mut cycle = Cycle::default();
        cycle.press(0, 3);
        cycle.press(0, 3);
        assert_eq!(cycle.press(1, 2), 0);
        assert_eq!(cycle.active(), Some(1));
        assert_eq!(cycle.press(0, 3), 0);
    }

    #[test]
    fn a_single_entry_stays_put() {
        let mut cycle = Cycle::default();
        assert_eq!(cycle.press(0, 1), 0);
        assert_eq!(cycle.press(0, 1), 0);
    }
}
