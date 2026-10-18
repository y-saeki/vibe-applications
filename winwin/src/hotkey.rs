//! Global shortcuts as they are written in the config file ("Ctrl+Alt+Left")
//! and as winwin holds them: a set of modifiers and a key, in types of its
//! own rather than any operating system's. `vk.rs` turns them into what
//! Windows takes.
//!
//! Nothing here touches the operating system, so it is tested on every
//! platform.

use std::fmt;
use std::ops::{BitOr, BitOrAssign};
use std::str::FromStr;

use crate::vk;

/// The most strokes a shortcut can take.
pub const MAX_STROKES: usize = 2;

/// Modifier keys held with a shortcut's key, left and right alike.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Modifiers = Modifiers(0);
    pub const CTRL: Modifiers = Modifiers(1);
    pub const ALT: Modifiers = Modifiers(1 << 1);
    pub const SHIFT: Modifiers = Modifiers(1 << 2);
    /// The Windows key; Command on a Mac.
    pub const META: Modifiers = Modifiers(1 << 3);

    /// In the order they are written and drawn, with the names they are
    /// written as.
    const NAMED: [(Modifiers, &'static str); 4] = [
        (Modifiers::META, "Win"),
        (Modifiers::CTRL, "Ctrl"),
        (Modifiers::ALT, "Alt"),
        (Modifiers::SHIFT, "Shift"),
    ];

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether every modifier in `other` is in `self`.
    pub fn contains(self, other: Modifiers) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether any modifier in `other` is in `self`.
    pub fn intersects(self, other: Modifiers) -> bool {
        self.0 & other.0 != 0
    }

    pub fn remove(&mut self, other: Modifiers) {
        self.0 &= !other.0;
    }

    /// The names of the modifiers in `self`, in the order they are written.
    fn names(self) -> impl Iterator<Item = &'static str> {
        Modifiers::NAMED
            .into_iter()
            .filter(move |(m, _)| self.contains(*m))
            .map(|(_, name)| name)
    }
}

impl BitOr for Modifiers {
    type Output = Modifiers;

    fn bitor(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 | other.0)
    }
}

impl BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, other: Modifiers) {
        self.0 |= other.0;
    }
}

/// The key of a shortcut: any key but a modifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// `b'A'..=b'Z'`.
    Letter(u8),
    /// `0..=9`, above the letters.
    Digit(u8),
    /// `0..=9` on the numeric keypad.
    Num(u8),
    /// `1..=24`.
    F(u8),
    Backspace,
    Tab,
    Enter,
    Escape,
    Space,
    PageUp,
    PageDown,
    End,
    Home,
    Left,
    Up,
    Right,
    Down,
    Insert,
    Delete,
    NumMultiply,
    NumAdd,
    NumSubtract,
    NumDecimal,
    NumDivide,
    // The punctuation keys, named after where they sit rather than what
    // they print, which depends on the keyboard layout.
    Oem1,
    OemPlus,
    OemComma,
    OemMinus,
    OemPeriod,
    Oem2,
    Oem3,
    Oem4,
    Oem5,
    Oem6,
    Oem7,
    Oem102,
    /// A key with no name of its own, written as its Windows virtual-key
    /// code (`0xAD`). [`vk::press`] makes one only for a code that is none
    /// of the keys above, so that each key has one value.
    Code(u8),
}

/// Names for the keys a window shortcut is likely to use, besides letters,
/// digits, `Num0`-`Num9` and `F1`-`F24`. Anything else is written as its
/// virtual-key code in hex (`0xBA`), which round-trips too.
const KEY_NAMES: &[(&str, Key)] = &[
    ("Backspace", Key::Backspace),
    ("Tab", Key::Tab),
    ("Enter", Key::Enter),
    ("Escape", Key::Escape),
    ("Space", Key::Space),
    ("PageUp", Key::PageUp),
    ("PageDown", Key::PageDown),
    ("End", Key::End),
    ("Home", Key::Home),
    ("Left", Key::Left),
    ("Up", Key::Up),
    ("Right", Key::Right),
    ("Down", Key::Down),
    ("Insert", Key::Insert),
    ("Delete", Key::Delete),
    ("NumMultiply", Key::NumMultiply),
    ("NumAdd", Key::NumAdd),
    ("NumSubtract", Key::NumSubtract),
    ("NumDecimal", Key::NumDecimal),
    ("NumDivide", Key::NumDivide),
    ("Oem1", Key::Oem1),
    ("OemPlus", Key::OemPlus),
    ("OemComma", Key::OemComma),
    ("OemMinus", Key::OemMinus),
    ("OemPeriod", Key::OemPeriod),
    ("Oem2", Key::Oem2),
    ("Oem3", Key::Oem3),
    ("Oem4", Key::Oem4),
    ("Oem5", Key::Oem5),
    ("Oem6", Key::Oem6),
    ("Oem7", Key::Oem7),
    ("Oem102", Key::Oem102),
];

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Key::Letter(c) => write!(f, "{}", char::from(c)),
            Key::Digit(d) => write!(f, "{d}"),
            Key::Num(d) => write!(f, "Num{d}"),
            Key::F(n) => write!(f, "F{n}"),
            Key::Code(code) => write!(f, "0x{code:02X}"),
            key => f.write_str(
                KEY_NAMES
                    .iter()
                    .find(|(_, k)| *k == key)
                    .map_or("", |(name, _)| name),
            ),
        }
    }
}

/// A key going down or up, as the settings window hears it while it records
/// a shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Modifier(Modifiers),
    Key(Key),
}

/// One key as the settings window draws it: its name, or for the arrow keys
/// the direction, which reads better as a chevron than as a word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keycap {
    Name(&'static str),
    Key(Key),
    Arrow(Arrow),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrow {
    Left,
    Up,
    Right,
    Down,
}

impl Keycap {
    /// The text on the cap, for all but the arrows.
    pub fn label(self) -> Option<String> {
        match self {
            Keycap::Name(name) => Some(name.to_string()),
            Keycap::Key(key) => Some(key.to_string()),
            Keycap::Arrow(_) => None,
        }
    }
}

/// The keys of a combination as the settings window draws them, one cap
/// each: the modifiers in the order they are written, then the key, if
/// there is one yet.
pub fn keycaps(modifiers: Modifiers, key: Option<Key>) -> Vec<Keycap> {
    let mut caps: Vec<Keycap> = modifiers.names().map(Keycap::Name).collect();
    if let Some(key) = key {
        caps.push(match key {
            Key::Left => Keycap::Arrow(Arrow::Left),
            Key::Up => Keycap::Arrow(Arrow::Up),
            Key::Right => Keycap::Arrow(Arrow::Right),
            Key::Down => Keycap::Arrow(Arrow::Down),
            _ => Keycap::Key(key),
        });
    }
    caps
}

/// Follows the keys pressed while the settings window records a shortcut.
/// Pressing a key that is not a modifier records it with the modifiers held
/// at that moment. Another key pressed while those modifiers stay held is
/// the second stroke, as it would be when the shortcut is used; anything
/// else starts over. Pressing a modifier starts over with just the modifiers
/// held. Letting go changes what is held, not what was recorded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recorder {
    held: Modifiers,
    /// The key that is down and has been recorded, so that its repeats are
    /// not taken for another stroke.
    down: Option<Key>,
    /// What was recorded: modifiers and the keys pressed with them, one per
    /// stroke (none yet while only modifiers have been pressed).
    pub modifiers: Modifiers,
    keys: Vec<Key>,
    /// Whether the modifiers have stayed held since the last stroke, so
    /// that the next key follows it.
    open: bool,
}

impl Recorder {
    /// Starts out showing `strokes`, with nothing held.
    pub fn showing(strokes: &[Hotkey]) -> Recorder {
        Recorder {
            modifiers: strokes.first().map_or(Modifiers::NONE, |h| h.modifiers),
            keys: strokes.iter().map(|h| h.key).collect(),
            ..Recorder::default()
        }
    }

    /// The strokes recorded so far.
    pub fn strokes(&self) -> Vec<Hotkey> {
        self.keys
            .iter()
            .map(|&key| Hotkey {
                modifiers: self.modifiers,
                key,
            })
            .collect()
    }

    pub fn key_down(&mut self, press: Press) {
        match press {
            // Already held: a repeat, or the other of a left-right pair.
            Press::Modifier(m) if self.held.contains(m) => {}
            Press::Modifier(m) => {
                self.held |= m;
                self.modifiers = self.held;
                self.keys.clear();
                self.open = false;
            }
            Press::Key(key) if Some(key) == self.down => {}
            Press::Key(key) => {
                let follows =
                    self.open && self.keys.len() < MAX_STROKES && self.keys.last() != Some(&key);
                if !follows {
                    self.modifiers = self.held;
                    self.keys.clear();
                }
                self.keys.push(key);
                self.down = Some(key);
                self.open = true;
            }
        }
    }

    pub fn key_up(&mut self, press: Press) {
        match press {
            Press::Modifier(m) => {
                self.held.remove(m);
                self.open = false;
            }
            Press::Key(key) if Some(key) == self.down => self.down = None,
            Press::Key(_) => {}
        }
    }

    /// What was recorded as a shortcut, or why it cannot be one. `None`
    /// while no key other than modifiers has been pressed.
    pub fn result(&self) -> Option<Result<Keys, HotkeyError>> {
        (!self.keys.is_empty()).then(|| Keys::new(self.strokes()))
    }
}

/// What one part of a combination names that is not a modifier written by
/// name: a key, or a modifier written as its virtual-key code.
fn parse_key(name: &str) -> Option<Press> {
    let upper = name.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    if bytes.len() == 1 && bytes[0].is_ascii_uppercase() {
        return Some(Press::Key(Key::Letter(bytes[0])));
    }
    if bytes.len() == 1 && bytes[0].is_ascii_digit() {
        return Some(Press::Key(Key::Digit(bytes[0] - b'0')));
    }
    if let Some(hex) = upper.strip_prefix("0X") {
        return u32::from_str_radix(hex, 16).ok().and_then(vk::press);
    }
    if let Some(n) = upper.strip_prefix("NUM").and_then(|d| d.parse::<u8>().ok()) {
        return (n <= 9).then_some(Press::Key(Key::Num(n)));
    }
    if let Some(n) = upper.strip_prefix('F').and_then(|d| d.parse::<u8>().ok()) {
        return (1..=24).contains(&n).then_some(Press::Key(Key::F(n)));
    }
    KEY_NAMES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, key)| Press::Key(*key))
}

#[derive(Debug, PartialEq, Eq)]
pub enum HotkeyError {
    Empty,
    UnknownPart(String),
    NoKey,
    TwoKeys,
    ModifierAsKey,
    /// Shift alone, or no modifier at all, would take the key away from
    /// every application.
    NeedsModifier,
    TooManyStrokes,
    /// The strokes of one shortcut are pressed with the modifiers held
    /// throughout.
    ModifiersDiffer,
    SameKeyTwice,
}

impl fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HotkeyError::Empty => write!(f, "ショートカットが空です"),
            HotkeyError::UnknownPart(p) => write!(f, "「{p}」はキーとして認識できません"),
            HotkeyError::NoKey => write!(f, "修飾キー以外のキーがありません"),
            HotkeyError::TwoKeys => write!(f, "修飾キー以外のキーは 1 つだけ指定できます"),
            HotkeyError::ModifierAsKey => write!(f, "修飾キーだけのショートカットは使えません"),
            HotkeyError::NeedsModifier => {
                write!(f, "Ctrl・Alt・Win のいずれかを含めてください")
            }
            HotkeyError::TooManyStrokes => {
                write!(f, "続けて押せるのは {MAX_STROKES} つまでです")
            }
            HotkeyError::ModifiersDiffer => {
                write!(f, "続けて押すキーの修飾キーはそろえてください")
            }
            HotkeyError::SameKeyTwice => write!(f, "同じキーを続けて押すことはできません"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl Hotkey {
    pub fn new(modifiers: Modifiers, key: Key) -> Result<Self, HotkeyError> {
        if !modifiers.intersects(Modifiers::CTRL | Modifiers::ALT | Modifiers::META) {
            return Err(HotkeyError::NeedsModifier);
        }
        Ok(Hotkey { modifiers, key })
    }
}

/// The modifiers and the key written in `s`, not yet checked as a shortcut.
fn parse_combination(s: &str) -> Result<(Modifiers, Key), HotkeyError> {
    if s.trim().is_empty() {
        return Err(HotkeyError::Empty);
    }
    let mut modifiers = Modifiers::NONE;
    let mut key = None;
    for part in s.split('+').map(str::trim) {
        let modifier = match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Some(Modifiers::CTRL),
            // Option and Command are a Mac's names for the keys at the
            // places of Alt and Win.
            "alt" | "option" | "opt" => Some(Modifiers::ALT),
            "shift" => Some(Modifiers::SHIFT),
            "win" | "cmd" | "command" => Some(Modifiers::META),
            _ => None,
        };
        if let Some(m) = modifier {
            modifiers |= m;
            continue;
        }
        let press = parse_key(part).ok_or_else(|| HotkeyError::UnknownPart(part.to_string()))?;
        if key.replace(press).is_some() {
            return Err(HotkeyError::TwoKeys);
        }
    }
    match key.ok_or(HotkeyError::NoKey)? {
        Press::Key(key) => Ok((modifiers, key)),
        Press::Modifier(_) => Err(HotkeyError::ModifierAsKey),
    }
}

impl FromStr for Hotkey {
    type Err = HotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (modifiers, key) = parse_combination(s)?;
        Hotkey::new(modifiers, key)
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for name in self.modifiers.names() {
            write!(f, "{name}+")?;
        }
        write!(f, "{}", self.key)
    }
}

/// A shortcut: one combination, or two keys pressed one after the other
/// while the same modifiers stay held ("Ctrl+Left, Up"). Each stroke is
/// a valid [`Hotkey`] of its own.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Keys(Vec<Hotkey>);

impl Keys {
    pub fn new(strokes: Vec<Hotkey>) -> Result<Keys, HotkeyError> {
        if strokes.is_empty() {
            return Err(HotkeyError::Empty);
        }
        if strokes.len() > MAX_STROKES {
            return Err(HotkeyError::TooManyStrokes);
        }
        // A second stroke with other modifiers is told as such, before
        // what those modifiers would be wrong for on their own.
        Hotkey::new(strokes[0].modifiers, strokes[0].key)?;
        if strokes.iter().any(|h| h.modifiers != strokes[0].modifiers) {
            return Err(HotkeyError::ModifiersDiffer);
        }
        for h in &strokes {
            Hotkey::new(h.modifiers, h.key)?;
        }
        if strokes.windows(2).any(|w| w[0].key == w[1].key) {
            return Err(HotkeyError::SameKeyTwice);
        }
        Ok(Keys(strokes))
    }

    pub fn strokes(&self) -> &[Hotkey] {
        &self.0
    }

    pub fn first(&self) -> Hotkey {
        self.0[0]
    }

    pub fn last(&self) -> Hotkey {
        self.0[self.0.len() - 1]
    }
}

impl From<Hotkey> for Keys {
    fn from(h: Hotkey) -> Keys {
        Keys(vec![h])
    }
}

/// A second stroke is written as its key alone ("Ctrl+Left, Up"): it is
/// pressed with the first stroke's modifiers still held. Written with them
/// in full, it reads the same.
impl FromStr for Keys {
    type Err = HotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(',');
        let first: Hotkey = parts.next().unwrap_or_default().parse()?;
        let mut strokes = vec![first];
        for part in parts {
            let (modifiers, key) = parse_combination(part)?;
            strokes.push(Hotkey {
                modifiers: if modifiers.is_empty() {
                    first.modifiers
                } else {
                    modifiers
                },
                key,
            });
        }
        Keys::new(strokes)
    }
}

impl fmt::Display for Keys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&strokes_text(&self.0))
    }
}

/// Strokes as they are written and shown: the first in full, the ones after
/// it as their keys alone, since they share its modifiers ("Ctrl+Left, Up").
pub fn strokes_text(strokes: &[Hotkey]) -> String {
    let parts: Vec<String> = strokes
        .iter()
        .enumerate()
        .map(|(i, h)| {
            if i == 0 {
                h.to_string()
            } else {
                h.key.to_string()
            }
        })
        .collect();
    parts.join(", ")
}

/// Strokes as the settings window draws them, one group of caps each: the
/// first with its modifiers, the ones after it as their keys alone.
pub fn strokes_caps(strokes: &[Hotkey]) -> Vec<Vec<Keycap>> {
    strokes
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let modifiers = if i == 0 { h.modifiers } else { Modifiers::NONE };
            keycaps(modifiers, Some(h.key))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hk(s: &str) -> Hotkey {
        s.parse().unwrap()
    }

    const CTRL: Press = Press::Modifier(Modifiers::CTRL);
    const ALT: Press = Press::Modifier(Modifiers::ALT);
    const SHIFT: Press = Press::Modifier(Modifiers::SHIFT);
    const META: Press = Press::Modifier(Modifiers::META);

    fn key(k: Key) -> Press {
        Press::Key(k)
    }

    #[test]
    fn parses_modifiers_and_named_keys() {
        assert_eq!(
            hk("Ctrl+Alt+Left"),
            Hotkey {
                modifiers: Modifiers::CTRL | Modifiers::ALT,
                key: Key::Left
            }
        );
        assert_eq!(hk("win + shift + F12").key, Key::F(12));
        assert_eq!(
            hk("win+shift+F12").modifiers,
            Modifiers::META | Modifiers::SHIFT
        );
        assert_eq!(hk("Control+Alt+c").key, Key::Letter(b'C'));
        assert_eq!(hk("Ctrl+Alt+7").key, Key::Digit(7));
        assert_eq!(hk("Ctrl+Alt+Num5").key, Key::Num(5));
        assert_eq!(hk("Ctrl+Alt+enter").key, Key::Enter);
        assert_eq!(hk("Ctrl+Alt+0xAD").key, Key::Code(0xAD));
    }

    #[test]
    fn a_key_written_as_its_code_is_the_key_itself() {
        assert_eq!(hk("Ctrl+Alt+0xBA"), hk("Ctrl+Alt+Oem1"));
        assert_eq!(hk("Ctrl+Alt+0x25").to_string(), "Ctrl+Alt+Left");
        assert_eq!(hk("Ctrl+Alt+0x41"), hk("Ctrl+Alt+A"));
        assert_eq!(
            "Ctrl+0xA2".parse::<Hotkey>(),
            Err(HotkeyError::ModifierAsKey)
        );
        for code in ["0x00", "0xFF", "0x100"] {
            assert_eq!(
                format!("Ctrl+{code}").parse::<Hotkey>(),
                Err(HotkeyError::UnknownPart(code.into()))
            );
        }
    }

    #[test]
    fn reads_the_mac_names_of_alt_and_win() {
        assert_eq!(hk("Cmd+Option+Left"), hk("Win+Alt+Left"));
        assert_eq!(hk("command+opt+K"), hk("Win+Alt+K"));
        // They are written by the names the file has always used.
        assert_eq!(hk("Ctrl+Option+Left").to_string(), "Ctrl+Alt+Left");
    }

    #[test]
    fn writes_modifiers_in_a_fixed_order() {
        assert_eq!(
            hk("Shift+Alt+Win+Ctrl+Up").to_string(),
            "Win+Ctrl+Alt+Shift+Up"
        );
        assert_eq!(hk("ctrl+alt+c").to_string(), "Ctrl+Alt+C");
    }

    fn keys(s: &str) -> Keys {
        s.parse().unwrap()
    }

    #[test]
    fn records_the_modifiers_held_with_a_key() {
        let mut r = Recorder::default();
        assert_eq!(r.result(), None);
        r.key_down(CTRL);
        r.key_down(SHIFT);
        assert_eq!(
            (r.modifiers, r.strokes()),
            (Modifiers::CTRL | Modifiers::SHIFT, vec![])
        );
        assert_eq!(r.result(), None);
        r.key_down(key(Key::Left));
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Shift+Left"))));
        // Held keys repeat; that records nothing more.
        r.key_down(key(Key::Left));
        r.key_down(CTRL);
        r.key_up(key(Key::Left));
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Shift+Left"))));
        r.key_up(SHIFT);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Shift+Left"))));
        // Shift was let go: the next key starts over with Ctrl alone.
        r.key_down(key(Key::Letter(b'K')));
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+K"))));
    }

    #[test]
    fn a_second_key_with_the_modifiers_still_held_follows() {
        let mut r = Recorder::default();
        r.key_down(CTRL);
        r.key_down(key(Key::Left));
        r.key_up(key(Key::Left));
        r.key_down(key(Key::Up));
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Left, Up"))));
        // A third starts over, and so does the same key twice.
        r.key_up(key(Key::Up));
        r.key_down(key(Key::Down));
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Down"))));
        r.key_up(key(Key::Down));
        r.key_down(key(Key::Down));
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Down"))));
    }

    #[test]
    fn letting_go_of_a_modifier_ends_the_shortcut() {
        let mut r = Recorder::default();
        r.key_down(CTRL);
        r.key_down(key(Key::Left));
        r.key_up(key(Key::Left));
        r.key_up(CTRL);
        r.key_down(CTRL);
        assert_eq!((r.modifiers, r.strokes()), (Modifiers::CTRL, vec![]));
        r.key_down(key(Key::Up));
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Up"))));
        // What it starts out showing is not followed either.
        let mut r = Recorder::showing(keys("Ctrl+Alt+K").strokes());
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Alt+K"))));
        r.key_down(CTRL);
        r.key_down(ALT);
        r.key_down(key(Key::Letter(b'J')));
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Alt+J"))));
    }

    #[test]
    fn a_modifier_starts_the_recording_over() {
        let mut r = Recorder::default();
        r.key_down(META);
        r.key_down(key(Key::Letter(b'A')));
        r.key_up(key(Key::Letter(b'A')));
        r.key_up(META);
        r.key_down(ALT);
        assert_eq!((r.modifiers, r.strokes()), (Modifiers::ALT, vec![]));
        assert_eq!(r.result(), None);
    }

    #[test]
    fn says_why_a_recorded_combination_cannot_be_used() {
        let mut r = Recorder::default();
        r.key_down(SHIFT);
        r.key_down(key(Key::Letter(b'A')));
        assert_eq!(r.result(), Some(Err(HotkeyError::NeedsModifier)));
    }

    #[test]
    fn sequences_are_written_with_commas() {
        let k = keys("ctrl+left ,ctrl + up");
        assert_eq!(k.strokes(), [hk("Ctrl+Left"), hk("Ctrl+Up")]);
        // The second stroke shares the first one's modifiers, and is
        // written without them.
        assert_eq!(k.to_string(), "Ctrl+Left, Up");
        assert_eq!(k.to_string().parse::<Keys>(), Ok(k.clone()));
        assert_eq!(
            keys("Ctrl+Shift+Left, Up").to_string(),
            "Ctrl+Shift+Left, Up"
        );
        assert_eq!((k.first(), k.last()), (hk("Ctrl+Left"), hk("Ctrl+Up")));
        assert_eq!(keys("Ctrl+Alt+K"), Keys::from(hk("Ctrl+Alt+K")));
        assert_eq!("Ctrl+A,".parse::<Keys>(), Err(HotkeyError::Empty));
        assert_eq!(
            "Ctrl+A, Shift+B".parse::<Keys>(),
            Err(HotkeyError::ModifiersDiffer)
        );
        assert_eq!(
            "Ctrl+A, Ctrl+B, Ctrl+C".parse::<Keys>(),
            Err(HotkeyError::TooManyStrokes)
        );
        assert_eq!(
            "Ctrl+Left, Shift+Up".parse::<Keys>(),
            Err(HotkeyError::ModifiersDiffer)
        );
        assert_eq!("Left, Up".parse::<Keys>(), Err(HotkeyError::NeedsModifier));
        assert_eq!(
            "Ctrl+Left, Foo".parse::<Keys>(),
            Err(HotkeyError::UnknownPart("Foo".into()))
        );
        for other in ["Ctrl+Shift+Up", "Alt+Up"] {
            assert_eq!(
                format!("Ctrl+Left, {other}").parse::<Keys>(),
                Err(HotkeyError::ModifiersDiffer)
            );
        }
        assert_eq!(
            "Ctrl+Shift+Left, Ctrl+Alt+Up".parse::<Keys>(),
            Err(HotkeyError::ModifiersDiffer)
        );
        assert_eq!(
            "Ctrl+Left, Left".parse::<Keys>(),
            Err(HotkeyError::SameKeyTwice)
        );
    }

    #[test]
    fn a_second_stroke_is_shown_as_its_key_alone() {
        let k = keys("Ctrl+Shift+Left, Up");
        assert_eq!(
            strokes_caps(k.strokes()),
            [
                vec![
                    Keycap::Name("Ctrl"),
                    Keycap::Name("Shift"),
                    Keycap::Arrow(Arrow::Left)
                ],
                vec![Keycap::Arrow(Arrow::Up)],
            ]
        );
        assert_eq!(strokes_text(k.strokes()), "Ctrl+Shift+Left, Up");
        assert_eq!(strokes_caps(&[]), Vec::<Vec<Keycap>>::new());
    }

    #[test]
    fn keycaps_follow_the_written_order() {
        assert_eq!(
            keycaps(
                Modifiers::SHIFT | Modifiers::CTRL | Modifiers::META,
                Some(Key::Left)
            ),
            [
                Keycap::Name("Win"),
                Keycap::Name("Ctrl"),
                Keycap::Name("Shift"),
                Keycap::Arrow(Arrow::Left)
            ]
        );
        let labels: Vec<_> = keycaps(Modifiers::CTRL | Modifiers::ALT, Some(Key::Enter))
            .into_iter()
            .map(|c| c.label().unwrap())
            .collect();
        assert_eq!(labels, ["Ctrl", "Alt", "Enter"]);
        assert_eq!(keycaps(Modifiers::ALT, None), [Keycap::Name("Alt")]);
        assert_eq!(
            keycaps(Modifiers::NONE, Some(Key::Down)),
            [Keycap::Arrow(Arrow::Down)]
        );
        assert_eq!(Keycap::Arrow(Arrow::Up).label(), None);
    }

    #[test]
    fn rejects_what_cannot_be_a_shortcut() {
        assert_eq!("".parse::<Hotkey>(), Err(HotkeyError::Empty));
        assert_eq!("Ctrl+Alt".parse::<Hotkey>(), Err(HotkeyError::NoKey));
        assert_eq!("Ctrl+A+B".parse::<Hotkey>(), Err(HotkeyError::TwoKeys));
        assert_eq!("Left".parse::<Hotkey>(), Err(HotkeyError::NeedsModifier));
        assert_eq!(
            "Shift+Left".parse::<Hotkey>(),
            Err(HotkeyError::NeedsModifier)
        );
        assert_eq!(
            "Ctrl+Alt+Foo".parse::<Hotkey>(),
            Err(HotkeyError::UnknownPart("Foo".into()))
        );
        assert_eq!(
            "Ctrl+F25".parse::<Hotkey>(),
            Err(HotkeyError::UnknownPart("F25".into()))
        );
        assert_eq!(
            Hotkey::new(Modifiers::SHIFT, Key::Left),
            Err(HotkeyError::NeedsModifier)
        );
    }
}
