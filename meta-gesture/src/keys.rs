//! The key combinations a gesture sends to an application ("Ctrl+Shift+T"),
//! as the config file writes them and as meta-gesture holds them: a set of
//! modifiers and a key, in types of its own rather than any operating
//! system's. `vk.rs` turns them into what Windows takes, `mac.rs` into what
//! macOS takes.
//!
//! Nothing here touches the operating system, so it is tested on every
//! platform.

use std::fmt;
use std::ops::{BitOr, BitOrAssign};
use std::str::FromStr;

use crate::vk;

/// Modifier keys held with a combination's key, left and right alike.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Modifiers = Modifiers(0);
    pub const CTRL: Modifiers = Modifiers(1);
    /// Alt; Option on a Mac.
    pub const ALT: Modifiers = Modifiers(1 << 1);
    pub const SHIFT: Modifiers = Modifiers(1 << 2);
    /// The Windows key; Command on a Mac.
    pub const META: Modifiers = Modifiers(1 << 3);

    pub const EACH: [Modifiers; 4] = [
        Modifiers::CTRL,
        Modifiers::ALT,
        Modifiers::SHIFT,
        Modifiers::META,
    ];

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether every modifier in `other` is in `self`.
    pub fn contains(self, other: Modifiers) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn remove(&mut self, other: Modifiers) {
        self.0 &= !other.0;
    }

    /// The modifiers in `self` as `look` names them, in its order.
    fn labels(self, look: Look) -> impl Iterator<Item = &'static str> {
        let table: [(Modifiers, &str); 4] = match look {
            Look::Windows => [
                (Modifiers::META, "Win"),
                (Modifiers::CTRL, "Ctrl"),
                (Modifiers::ALT, "Alt"),
                (Modifiers::SHIFT, "Shift"),
            ],
            Look::Mac => [
                (Modifiers::CTRL, "⌃"),
                (Modifiers::ALT, "⌥"),
                (Modifiers::SHIFT, "⇧"),
                (Modifiers::META, "⌘"),
            ],
        };
        table
            .into_iter()
            .filter(move |(m, _)| self.contains(*m))
            .map(|(_, name)| name)
    }

    /// The names the config file writes, which are the key's names on the
    /// keyboard of the platform the file is for.
    fn words(self, look: Look) -> impl Iterator<Item = &'static str> {
        let table: [(Modifiers, &str); 4] = match look {
            Look::Windows => [
                (Modifiers::META, "Win"),
                (Modifiers::CTRL, "Ctrl"),
                (Modifiers::ALT, "Alt"),
                (Modifiers::SHIFT, "Shift"),
            ],
            Look::Mac => [
                (Modifiers::CTRL, "Ctrl"),
                (Modifiers::ALT, "Option"),
                (Modifiers::SHIFT, "Shift"),
                (Modifiers::META, "Cmd"),
            ],
        };
        table
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

/// The key of a combination: any key but a modifier.
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

/// Names for the keys besides letters, digits, `Num0`-`Num9` and
/// `F1`-`F24`. Anything else is written as its virtual-key code in hex
/// (`0xBA`), which round-trips too.
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
/// a combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Modifier(Modifiers),
    Key(Key),
}

/// How a platform names the modifiers: Windows by words, a Mac by its
/// symbols and in its order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Look {
    #[default]
    Windows,
    Mac,
}

/// One key as the settings window draws it. `Plus` is no key but the `+`
/// drawn between the modifiers and the key on Windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keycap {
    Name(&'static str),
    Key(Key),
    Arrow(Arrow),
    Plus,
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
            Keycap::Plus => Some("+".to_string()),
        }
    }

    /// The text on the cap with the arrows as arrow characters, for a Mac,
    /// which draws caps as text.
    pub fn text(self) -> String {
        match self {
            Keycap::Arrow(Arrow::Left) => "←".into(),
            Keycap::Arrow(Arrow::Up) => "↑".into(),
            Keycap::Arrow(Arrow::Right) => "→".into(),
            Keycap::Arrow(Arrow::Down) => "↓".into(),
            cap => cap.label().unwrap_or_default(),
        }
    }
}

/// The keys of a combination as the settings window draws them, one cap
/// each: the modifiers in `look`'s order, then the key, if there is one yet.
/// On Windows a [`Keycap::Plus`] sets the key off from the modifiers.
pub fn keycaps(look: Look, modifiers: Modifiers, key: Option<Key>) -> Vec<Keycap> {
    let mut caps: Vec<Keycap> = modifiers.labels(look).map(Keycap::Name).collect();
    if let Some(key) = key {
        if !caps.is_empty() && look == Look::Windows {
            caps.push(Keycap::Plus);
        }
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

/// A key with the modifiers held with it: what a gesture sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Combo {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl Combo {
    pub fn new(modifiers: Modifiers, key: Key) -> Combo {
        Combo { modifiers, key }
    }

    /// As the config file for `look`'s platform writes it.
    pub fn text(&self, look: Look) -> String {
        let mut s = String::new();
        for name in self.modifiers.words(look) {
            s.push_str(name);
            s.push('+');
        }
        s.push_str(&self.key.to_string());
        s
    }

    pub fn caps(&self, look: Look) -> Vec<Keycap> {
        keycaps(look, self.modifiers, Some(self.key))
    }
}

/// Written as Windows writes it; [`Combo::text`] picks the platform.
impl fmt::Display for Combo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text(Look::Windows))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComboError {
    Empty,
    UnknownPart(String),
    NoKey,
    TwoKeys,
    ModifierAsKey,
}

impl fmt::Display for ComboError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComboError::Empty => write!(f, "キーが空です"),
            ComboError::UnknownPart(p) => write!(f, "「{p}」はキーとして認識できません"),
            ComboError::NoKey => write!(f, "修飾キー以外のキーがありません"),
            ComboError::TwoKeys => write!(f, "修飾キー以外のキーは 1 つだけ指定できます"),
            ComboError::ModifierAsKey => write!(f, "修飾キーだけの組み合わせは使えません"),
        }
    }
}

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

/// Reads either platform's names: the modifier words of both are accepted,
/// so that a file stays readable whichever names it was written with.
impl FromStr for Combo {
    type Err = ComboError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.trim().is_empty() {
            return Err(ComboError::Empty);
        }
        let mut modifiers = Modifiers::NONE;
        let mut key = None;
        for part in s.split('+').map(str::trim) {
            let modifier = match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => Some(Modifiers::CTRL),
                "alt" | "option" | "opt" => Some(Modifiers::ALT),
                "shift" => Some(Modifiers::SHIFT),
                "win" | "cmd" | "command" => Some(Modifiers::META),
                _ => None,
            };
            if let Some(m) = modifier {
                modifiers |= m;
                continue;
            }
            let press = parse_key(part).ok_or_else(|| ComboError::UnknownPart(part.to_string()))?;
            if key.replace(press).is_some() {
                return Err(ComboError::TwoKeys);
            }
        }
        match key.ok_or(ComboError::NoKey)? {
            Press::Key(key) => Ok(Combo { modifiers, key }),
            Press::Modifier(_) => Err(ComboError::ModifierAsKey),
        }
    }
}

/// Follows the keys pressed while the settings window records a
/// combination. A key that is not a modifier is recorded with the modifiers
/// held at that moment; pressing a modifier starts over with the modifiers
/// held. Letting go changes what is held, not what was recorded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recorder {
    held: Modifiers,
    /// The modifiers to show: those held, or those recorded with the key.
    pub modifiers: Modifiers,
    pub key: Option<Key>,
}

impl Recorder {
    /// Starts out showing `combo`, with nothing held.
    pub fn showing(combo: Option<Combo>) -> Recorder {
        Recorder {
            held: Modifiers::NONE,
            modifiers: combo.map_or(Modifiers::NONE, |c| c.modifiers),
            key: combo.map(|c| c.key),
        }
    }

    pub fn key_down(&mut self, press: Press) {
        match press {
            Press::Modifier(m) if self.held.contains(m) => {}
            Press::Modifier(m) => {
                self.held |= m;
                self.modifiers = self.held;
                self.key = None;
            }
            Press::Key(key) => {
                self.modifiers = self.held;
                self.key = Some(key);
            }
        }
    }

    pub fn key_up(&mut self, press: Press) {
        if let Press::Modifier(m) = press {
            self.held.remove(m);
        }
    }

    /// What was recorded, once a key that is not a modifier has been.
    pub fn result(&self) -> Option<Combo> {
        self.key.map(|key| Combo::new(self.modifiers, key))
    }

    pub fn caps(&self, look: Look) -> Vec<Keycap> {
        keycaps(look, self.modifiers, self.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo(s: &str) -> Combo {
        s.parse().unwrap()
    }

    #[test]
    fn reads_and_writes_combinations() {
        let c = combo("ctrl+shift+t");
        assert_eq!(c.modifiers, Modifiers::CTRL | Modifiers::SHIFT);
        assert_eq!(c.key, Key::Letter(b'T'));
        assert_eq!(c.text(Look::Windows), "Ctrl+Shift+T");
        assert_eq!(c.text(Look::Mac), "Ctrl+Shift+T");
        assert_eq!(combo("Alt+Left").text(Look::Windows), "Alt+Left");
        assert_eq!(combo("Cmd+Option+Left").text(Look::Mac), "Option+Cmd+Left");
        assert_eq!(combo("Win+Alt+Left").text(Look::Windows), "Win+Alt+Left");
        // A key alone is a combination too: gestures send F5 or Escape.
        assert_eq!(combo("F5"), Combo::new(Modifiers::NONE, Key::F(5)));
        assert_eq!(combo("0xAD").key, Key::Code(0xAD));
    }

    #[test]
    fn what_it_writes_reads_back() {
        for look in [Look::Windows, Look::Mac] {
            for text in [
                "Ctrl+W",
                "Alt+Shift+Tab",
                "Win+Ctrl+Alt+Shift+F24",
                "Num5",
                "Oem4",
            ] {
                let c = combo(text);
                assert_eq!(c.text(look).parse::<Combo>(), Ok(c), "{text} {look:?}");
            }
        }
    }

    #[test]
    fn refuses_what_is_no_combination() {
        assert_eq!("".parse::<Combo>(), Err(ComboError::Empty));
        assert_eq!("Ctrl".parse::<Combo>(), Err(ComboError::NoKey));
        assert_eq!("Ctrl+A+B".parse::<Combo>(), Err(ComboError::TwoKeys));
        assert_eq!(
            "Ctrl+Foo".parse::<Combo>(),
            Err(ComboError::UnknownPart("Foo".into()))
        );
        assert_eq!("0xA0".parse::<Combo>(), Err(ComboError::ModifierAsKey));
    }

    #[test]
    fn draws_caps_in_each_platforms_order() {
        let c = combo("Ctrl+Shift+Win+Left");
        assert_eq!(
            c.caps(Look::Windows),
            vec![
                Keycap::Name("Win"),
                Keycap::Name("Ctrl"),
                Keycap::Name("Shift"),
                Keycap::Plus,
                Keycap::Arrow(Arrow::Left),
            ]
        );
        assert_eq!(
            c.caps(Look::Mac),
            vec![
                Keycap::Name("⌃"),
                Keycap::Name("⇧"),
                Keycap::Name("⌘"),
                Keycap::Arrow(Arrow::Left),
            ]
        );
        assert_eq!(Keycap::Arrow(Arrow::Left).text(), "←");
    }

    #[test]
    fn records_the_key_with_the_modifiers_held() {
        let ctrl = Press::Modifier(Modifiers::CTRL);
        let shift = Press::Modifier(Modifiers::SHIFT);
        let mut r = Recorder::default();
        r.key_down(ctrl);
        r.key_down(shift);
        assert_eq!(r.result(), None);
        assert_eq!(r.modifiers, Modifiers::CTRL | Modifiers::SHIFT);
        r.key_down(Press::Key(Key::Letter(b'T')));
        assert_eq!(r.result(), Some(combo("Ctrl+Shift+T")));
        // Letting go keeps what was recorded.
        r.key_up(shift);
        r.key_up(ctrl);
        assert_eq!(r.result(), Some(combo("Ctrl+Shift+T")));
        // A key alone after that is recorded alone.
        r.key_down(Press::Key(Key::F(5)));
        assert_eq!(r.result(), Some(combo("F5")));
        // A modifier starts over.
        r.key_down(Press::Modifier(Modifiers::ALT));
        assert_eq!(r.result(), None);
        assert_eq!(r.modifiers, Modifiers::ALT);
    }

    #[test]
    fn starts_out_showing_the_combination_it_replaces() {
        let r = Recorder::showing(Some(combo("Alt+Left")));
        assert_eq!(r.result(), Some(combo("Alt+Left")));
        assert_eq!(Recorder::showing(None).result(), None);
    }
}
