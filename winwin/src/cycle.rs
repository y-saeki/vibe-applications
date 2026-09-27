//! What a press of a registered key combination does. Entries that share a
//! shortcut are grouped into one binding; pressing its last stroke again
//! while its modifiers stay held steps to the next entry, in the order of the
//! config, and wraps around. Letting go of the modifiers starts the next
//! press from the first entry.
//!
//! A shortcut of two strokes ("Ctrl+Left, Up") waits after the first
//! for the second for as long as the modifiers stay held. Only the first
//! strokes are registered all the time; the ones that may come next are
//! registered while they may.
//!
//! A shortcut may also be the start of a longer one (`Ctrl+Left` and
//! `Ctrl+Left, Up`). Its entry is applied at once, and should the
//! longer one's next stroke follow before the modifiers are let go, the
//! longer one's entry replaces it.
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

/// Where the presses stand. A shortcut that is also the start of a longer
/// one is applied at once and waits for the longer one's next stroke, so
/// both can hold at a time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Presses {
    /// The strokes so far, while they are the start of a longer shortcut.
    pending: Vec<Hotkey>,
    /// Binding `.0`, which has more than one entry, was pressed last and
    /// chose entry `.1`.
    cycling: Option<(usize, usize)>,
}

impl Presses {
    /// The next stroke of a longer shortcut comes first; then a press of
    /// the cycling binding's last stroke; then the stroke on its own.
    pub fn press(&mut self, bindings: &[Binding], stroke: Hotkey) -> Outcome {
        let pending = std::mem::take(&mut self.pending);
        let cycling = self.cycling.take();
        if !pending.is_empty() {
            let mut strokes = pending;
            strokes.push(stroke);
            let outcome = self.enter(bindings, strokes);
            if outcome != Outcome::Nothing {
                return outcome;
            }
        }
        if let Some((binding, entry)) = cycling
            && let Some(b) = bindings.get(binding)
            && b.keys.last() == stroke
        {
            let entry = (entry + 1) % b.placements.len();
            self.cycling = Some((binding, entry));
            // Still the start of the longer ones, whichever entry it is on.
            if extended(bindings, b.keys.strokes()) {
                self.pending = b.keys.strokes().to_vec();
            }
            return Outcome::Place { binding, entry };
        }
        self.enter(bindings, vec![stroke])
    }

    /// `strokes` have been pressed: applies the binding they make, and waits
    /// for more when they begin a longer one. Leaves the state alone when
    /// they do neither.
    fn enter(&mut self, bindings: &[Binding], strokes: Vec<Hotkey>) -> Outcome {
        let exact = bindings.iter().position(|b| b.keys.strokes() == strokes);
        let longer = extended(bindings, &strokes);
        if longer {
            self.pending = strokes;
        }
        match exact {
            Some(binding) => {
                if bindings[binding].cycles() {
                    self.cycling = Some((binding, 0));
                }
                Outcome::Place { binding, entry: 0 }
            }
            None if longer => Outcome::Pending,
            None => Outcome::Nothing,
        }
    }

    /// The binding whose entries are being stepped through, if any.
    #[cfg(test)]
    pub fn cycling(&self) -> Option<usize> {
        self.cycling.map(|(b, _)| b)
    }

    #[cfg(test)]
    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// The modifiers (MOD_*) that, once any of them is let go, end the wait
    /// for a next stroke and the cycle: `None` while there is neither. The
    /// strokes of a shortcut share their modifiers, so there is one set.
    pub fn watching(&self, bindings: &[Binding]) -> Option<u32> {
        self.pending.first().map(|h| h.modifiers).or_else(|| {
            let (b, _) = self.cycling?;
            bindings.get(b).map(|b| b.keys.first().modifiers)
        })
    }

    /// The modifiers were let go: the next press starts over.
    pub fn reset(&mut self) {
        self.pending.clear();
        self.cycling = None;
    }

    /// The strokes that must be registered for now on top of
    /// [`first_strokes`]: those that may come next.
    pub fn next_strokes(&self, bindings: &[Binding]) -> Vec<Hotkey> {
        let firsts = first_strokes(bindings);
        let n = self.pending.len();
        let following = bindings.iter().filter_map(|b| {
            let s = b.keys.strokes();
            (n > 0 && s.len() > n && s.starts_with(&self.pending)).then(|| s[n])
        });
        let repeated = self
            .cycling
            .and_then(|(b, _)| bindings.get(b))
            .map(|b| b.keys.last());
        let mut out = Vec::new();
        for h in following.chain(repeated) {
            if !firsts.contains(&h) && !out.contains(&h) {
                out.push(h);
            }
        }
        out
    }
}

/// Whether some binding is longer than `strokes` and begins with them.
fn extended(bindings: &[Binding], strokes: &[Hotkey]) -> bool {
    bindings.iter().any(|b| {
        let s = b.keys.strokes();
        s.len() > strokes.len() && s.starts_with(strokes)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::MOD_CONTROL;

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

    /// 左上 1/2 × 1/2 and 左上 1/2 × 2/3 on Ctrl+Left, Up; 上半分 on
    /// Ctrl+Up; 右半分 on Ctrl+Left, Right.
    fn config_with_sequences() -> Config {
        let text = r#"
            [[shortcut]]
            keys = "Ctrl+Left, Up"
            anchor = "top-left"
            width = "1/2"
            height = "1/2"

            [[shortcut]]
            keys = "Ctrl+Up"
            anchor = "top"
            width = "1"
            height = "1/2"

            [[shortcut]]
            keys = "Ctrl+Left, Up"
            anchor = "top-left"
            width = "1/2"
            height = "2/3"

            [[shortcut]]
            keys = "Ctrl+Left, Right"
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

    /// 左半分 on Ctrl+Left, 左上 1/2 × 1/2 on Ctrl+Left, Up.
    fn config_with_a_prefix() -> Config {
        let text = r#"
            [[shortcut]]
            keys = "Ctrl+Left"
            anchor = "left"
            width = "1/2"
            height = "1"

            [[shortcut]]
            keys = "Ctrl+Left, Up"
            anchor = "top-left"
            width = "1/2"
            height = "1/2"
        "#;
        Config::parse(text, std::path::Path::new("config.toml")).unwrap()
    }

    #[test]
    fn a_shortcut_that_starts_another_applies_at_once() {
        let bindings = bindings(&config_with_a_prefix());
        let mut presses = Presses::default();
        let (left, up) = (hk("Ctrl+Left"), hk("Ctrl+Up"));
        assert_eq!(presses.press(&bindings, left), place(0, 0));
        assert!(presses.is_pending());
        assert_eq!(presses.next_strokes(&bindings), [up]);
        assert_eq!(presses.press(&bindings, up), place(1, 0));
        assert!(!presses.is_pending());
        // Without the next stroke, it stays as it is.
        presses.press(&bindings, left);
        presses.reset();
        assert_eq!(presses.next_strokes(&bindings), []);
        assert_eq!(presses.press(&bindings, left), place(0, 0));
    }

    #[test]
    fn a_shorter_shortcut_cycles_and_still_leads_on() {
        let mut config = config_with_a_prefix();
        let mut two_thirds = config.shortcuts[0].clone();
        two_thirds.placement.width = "2/3".parse().unwrap();
        config.shortcuts.push(two_thirds);
        let bindings = bindings(&config);
        let mut presses = Presses::default();
        let (left, up) = (hk("Ctrl+Left"), hk("Ctrl+Up"));
        assert_eq!(presses.press(&bindings, left), place(0, 0));
        assert_eq!(presses.press(&bindings, left), place(0, 1));
        assert_eq!(presses.cycling(), Some(0));
        assert!(presses.is_pending());
        assert_eq!(presses.press(&bindings, up), place(1, 0));
        // The longer one took over; the cycle of the shorter one is over.
        assert_eq!(presses.cycling(), None);
        // Letting go ends both.
        presses.press(&bindings, left);
        presses.reset();
        assert_eq!(presses.press(&bindings, left), place(0, 0));
    }

    #[test]
    fn the_modifiers_of_what_is_under_way_are_watched() {
        let bindings = bindings(&config_with_sequences());
        let mut presses = Presses::default();
        assert_eq!(presses.watching(&bindings), None);
        presses.press(&bindings, hk("Ctrl+Left"));
        assert_eq!(presses.watching(&bindings), Some(MOD_CONTROL));
        presses.press(&bindings, hk("Ctrl+Right"));
        // 右半分 does not cycle: nothing is left to wait for.
        assert_eq!(presses.watching(&bindings), None);
        presses.press(&bindings, hk("Ctrl+Left"));
        presses.press(&bindings, hk("Ctrl+Up"));
        assert_eq!(presses.watching(&bindings), Some(MOD_CONTROL));
        presses.reset();
        assert_eq!(presses.watching(&bindings), None);
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
