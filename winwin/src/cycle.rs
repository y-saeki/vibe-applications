//! What a press of a registered key combination does. Entries that share a
//! shortcut are grouped into one binding; pressing its last stroke again
//! while its modifiers stay held steps to the next entry, in the order of the
//! config, and wraps around. Letting go of the modifiers starts the next
//! press from the first entry.
//!
//! A shortcut of several strokes ("Ctrl+Left, Ctrl+Up") waits after each
//! stroke but the last for the next one, for up to
//! [`SEQUENCE_TIMEOUT_MS`](crate::hotkey::SEQUENCE_TIMEOUT_MS). Only the first
//! strokes are registered all the time; the ones that may come next are
//! registered while they may.
//!
//! Nothing here touches Win32, so it is tested on every platform.

use crate::config::Config;
use crate::hotkey::{Hotkey, Keys};
use crate::layout::Placement;

/// One shortcut and every entry that uses it.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub keys: Keys,
    pub placements: Vec<Placement>,
}

impl Binding {
    /// Whether pressing it again can lead somewhere else.
    pub fn cycles(&self) -> bool {
        self.placements.len() > 1
    }
}

/// The config's entries grouped by shortcut, in the order each shortcut
/// first appears.
pub fn bindings(config: &Config) -> Vec<Binding> {
    let mut out: Vec<Binding> = Vec::new();
    for s in &config.shortcuts {
        match out.iter_mut().find(|b| b.keys == s.keys) {
            Some(b) => {
                b.placements.push(s.placement);
            }
            None => out.push(Binding {
                keys: s.keys.clone(),
                placements: vec![s.placement],
            }),
        }
    }
    out
}

/// The first stroke of every binding, each once: what is registered for as
/// long as the bindings are.
pub fn first_strokes(bindings: &[Binding]) -> Vec<Hotkey> {
    let mut out = Vec::new();
    for b in bindings {
        if !out.contains(&b.keys.first()) {
            out.push(b.keys.first());
        }
    }
    out
}

/// What a press did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Entry `entry` of binding `binding` is to be applied.
    Place { binding: usize, entry: usize },
    /// The strokes so far begin a shortcut; the next one is awaited.
    Pending,
    /// The press leads nowhere.
    Nothing,
}

#[derive(Clone, Debug, Default, PartialEq)]
enum State {
    #[default]
    Idle,
    /// The strokes of an unfinished shortcut.
    Pending(Vec<Hotkey>),
    /// Binding `binding`, which has more than one entry, was pressed last
    /// and chose `entry`.
    Cycling { binding: usize, entry: usize },
}

/// Where the presses stand: a shortcut under way, or one whose entries are
/// being stepped through.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Presses {
    state: State,
}

impl Presses {
    pub fn press(&mut self, bindings: &[Binding], stroke: Hotkey) -> Outcome {
        let pending = match std::mem::take(&mut self.state) {
            State::Cycling { binding, entry } => {
                if let Some(b) = bindings.get(binding)
                    && b.keys.last() == stroke
                {
                    let entry = (entry + 1) % b.placements.len();
                    self.state = State::Cycling { binding, entry };
                    return Outcome::Place { binding, entry };
                }
                Vec::new()
            }
            State::Pending(strokes) => strokes,
            State::Idle => Vec::new(),
        };
        let outcome = self.follow(bindings, pending.clone(), stroke);
        // A stroke that does not go on from the ones before may still begin
        // something of its own.
        if outcome == Outcome::Nothing && !pending.is_empty() {
            return self.follow(bindings, Vec::new(), stroke);
        }
        outcome
    }

    fn follow(
        &mut self,
        bindings: &[Binding],
        mut strokes: Vec<Hotkey>,
        stroke: Hotkey,
    ) -> Outcome {
        strokes.push(stroke);
        if let Some(binding) = bindings.iter().position(|b| b.keys.strokes() == strokes) {
            if bindings[binding].cycles() {
                self.state = State::Cycling { binding, entry: 0 };
            }
            return Outcome::Place { binding, entry: 0 };
        }
        if bindings
            .iter()
            .any(|b| b.keys.strokes().starts_with(&strokes))
        {
            self.state = State::Pending(strokes);
            return Outcome::Pending;
        }
        Outcome::Nothing
    }

    /// The binding whose modifiers are being watched, if any.
    pub fn cycling(&self) -> Option<usize> {
        match self.state {
            State::Cycling { binding, .. } => Some(binding),
            _ => None,
        }
    }

    pub fn is_pending(&self) -> bool {
        matches!(self.state, State::Pending(_))
    }

    /// The modifiers were let go, or the next stroke did not come in time:
    /// the next press starts over.
    pub fn reset(&mut self) {
        self.state = State::Idle;
    }

    /// The strokes that must be registered for now on top of
    /// [`first_strokes`]: those that may come next.
    pub fn next_strokes(&self, bindings: &[Binding]) -> Vec<Hotkey> {
        let firsts = first_strokes(bindings);
        let candidates: Vec<Hotkey> = match &self.state {
            State::Idle => Vec::new(),
            State::Pending(strokes) => bindings
                .iter()
                .filter_map(|b| {
                    let s = b.keys.strokes();
                    (s.len() > strokes.len() && s.starts_with(strokes)).then(|| s[strokes.len()])
                })
                .collect(),
            State::Cycling { binding, .. } => bindings
                .get(*binding)
                .map(|b| b.keys.last())
                .into_iter()
                .collect(),
        };
        let mut out = Vec::new();
        for h in candidates {
            if !firsts.contains(&h) && !out.contains(&h) {
                out.push(h);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hk(s: &str) -> Hotkey {
        s.parse().unwrap()
    }

    fn config_with_shared_keys() -> Config {
        let mut config = Config::defaults();
        // 左 2/3 (Ctrl+Alt+E) joins 左半分 on Ctrl+Alt+Left.
        let two_thirds = config
            .shortcuts
            .iter()
            .position(|s| s.keys.to_string() == "Ctrl+Alt+E")
            .unwrap();
        config.shortcuts[two_thirds].keys = config.shortcuts[0].keys.clone();
        config
    }

    /// 左上 1/2 × 1/2 and 左上 1/2 × 2/3 on Ctrl+Left, Ctrl+Up; 上半分 on
    /// Ctrl+Up; 右半分 on Ctrl+Left, Ctrl+Right.
    fn config_with_sequences() -> Config {
        let text = r#"
            [[shortcut]]
            keys = "Ctrl+Left, Ctrl+Up"
            anchor = "top-left"
            width = "1/2"
            height = "1/2"

            [[shortcut]]
            keys = "Ctrl+Up"
            anchor = "top"
            width = "1"
            height = "1/2"

            [[shortcut]]
            keys = "Ctrl+Left, Ctrl+Up"
            anchor = "top-left"
            width = "1/2"
            height = "2/3"

            [[shortcut]]
            keys = "Ctrl+Left, Ctrl+Right"
            anchor = "right"
            width = "1/2"
            height = "1"
        "#;
        Config::parse(text, std::path::Path::new("config.toml")).unwrap()
    }

    fn place(binding: usize, entry: usize) -> Outcome {
        Outcome::Place { binding, entry }
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
        assert_eq!(left.placements[0].width.to_string(), "1/2");
        assert_eq!(left.placements[1].width.to_string(), "2/3");
        // The others keep their order behind it.
        assert_eq!(bindings[1].keys.to_string(), "Ctrl+Alt+Right");
    }

    #[test]
    fn repeated_presses_step_through_and_wrap() {
        let bindings = bindings(&config_with_shared_keys());
        let mut presses = Presses::default();
        let left = hk("Ctrl+Alt+Left");
        let picks: Vec<Outcome> = (0..3).map(|_| presses.press(&bindings, left)).collect();
        assert_eq!(picks, [place(0, 0), place(0, 1), place(0, 0)]);
        assert_eq!(presses.cycling(), Some(0));
    }

    #[test]
    fn letting_go_starts_over() {
        let bindings = bindings(&config_with_shared_keys());
        let mut presses = Presses::default();
        let left = hk("Ctrl+Alt+Left");
        presses.press(&bindings, left);
        assert_eq!(presses.press(&bindings, left), place(0, 1));
        presses.reset();
        assert_eq!(presses.cycling(), None);
        assert_eq!(presses.press(&bindings, left), place(0, 0));
    }

    #[test]
    fn another_shortcut_starts_its_own_cycle() {
        let bindings = bindings(&config_with_shared_keys());
        let mut presses = Presses::default();
        let left = hk("Ctrl+Alt+Left");
        presses.press(&bindings, left);
        presses.press(&bindings, left);
        assert_eq!(presses.press(&bindings, hk("Ctrl+Alt+Right")), place(1, 0));
        assert_eq!(presses.cycling(), None);
        assert_eq!(presses.press(&bindings, left), place(0, 0));
    }

    #[test]
    fn a_single_entry_stays_put() {
        let bindings = bindings(&Config::defaults());
        let mut presses = Presses::default();
        let left = hk("Ctrl+Alt+Left");
        assert_eq!(presses.press(&bindings, left), place(0, 0));
        assert_eq!(presses.press(&bindings, left), place(0, 0));
        assert_eq!(presses.cycling(), None);
    }

    #[test]
    fn a_sequence_waits_for_its_next_stroke() {
        let bindings = bindings(&config_with_sequences());
        assert_eq!(bindings.len(), 3);
        let mut presses = Presses::default();
        assert_eq!(presses.press(&bindings, hk("Ctrl+Left")), Outcome::Pending);
        assert!(presses.is_pending());
        // Ctrl+Up is registered anyway, as a shortcut of its own.
        assert_eq!(presses.next_strokes(&bindings), [hk("Ctrl+Right")]);
        assert_eq!(presses.press(&bindings, hk("Ctrl+Right")), place(2, 0));
        assert!(!presses.is_pending());
        assert_eq!(presses.next_strokes(&bindings), []);
    }

    #[test]
    fn a_sequence_cycles_on_its_last_stroke() {
        let bindings = bindings(&config_with_sequences());
        let mut presses = Presses::default();
        let (left, up) = (hk("Ctrl+Left"), hk("Ctrl+Up"));
        presses.press(&bindings, left);
        assert_eq!(presses.press(&bindings, up), place(0, 0));
        assert_eq!(presses.cycling(), Some(0));
        assert_eq!(presses.press(&bindings, up), place(0, 1));
        assert_eq!(presses.press(&bindings, up), place(0, 0));
        // Once let go, Ctrl+Up is its own shortcut again.
        presses.reset();
        assert_eq!(presses.press(&bindings, up), place(1, 0));
    }

    #[test]
    fn a_stroke_that_does_not_follow_starts_afresh() {
        let bindings = bindings(&config_with_sequences());
        let mut presses = Presses::default();
        presses.press(&bindings, hk("Ctrl+Left"));
        // Ctrl+Left again does not go on from Ctrl+Left, but begins anew.
        assert_eq!(presses.press(&bindings, hk("Ctrl+Left")), Outcome::Pending);
        presses.reset();
        assert!(!presses.is_pending());
        assert_eq!(presses.press(&bindings, hk("Ctrl+Up")), place(1, 0));
        // A different stroke ends a cycle and begins what it begins.
        presses.press(&bindings, hk("Ctrl+Left"));
        presses.press(&bindings, hk("Ctrl+Up"));
        assert_eq!(presses.press(&bindings, hk("Ctrl+Left")), Outcome::Pending);
        assert_eq!(presses.cycling(), None);
        assert_eq!(presses.press(&bindings, hk("Ctrl+Alt+Z")), Outcome::Nothing);
        assert!(!presses.is_pending());
    }

    #[test]
    fn only_first_strokes_are_always_registered() {
        let bindings = bindings(&config_with_sequences());
        assert_eq!(first_strokes(&bindings), [hk("Ctrl+Left"), hk("Ctrl+Up")]);
        let mut presses = Presses::default();
        assert_eq!(presses.next_strokes(&bindings), []);
        presses.press(&bindings, hk("Ctrl+Left"));
        presses.press(&bindings, hk("Ctrl+Up"));
        // Cycling on Ctrl+Up, which is registered already.
        assert_eq!(presses.cycling(), Some(0));
        assert_eq!(presses.next_strokes(&bindings), []);

        let mut config = config_with_sequences();
        config.shortcuts.remove(1);
        let bindings = super::bindings(&config);
        assert_eq!(first_strokes(&bindings), [hk("Ctrl+Left")]);
        presses.reset();
        presses.press(&bindings, hk("Ctrl+Left"));
        assert_eq!(
            presses.next_strokes(&bindings),
            [hk("Ctrl+Up"), hk("Ctrl+Right")]
        );
        presses.press(&bindings, hk("Ctrl+Up"));
        assert_eq!(presses.next_strokes(&bindings), [hk("Ctrl+Up")]);
    }
}
