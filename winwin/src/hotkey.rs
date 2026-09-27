//! Global shortcuts as they are written in the config file ("Ctrl+Alt+Left")
//! and as `RegisterHotKey` takes them (a modifier mask and a virtual-key code).
//!
//! Nothing here touches Win32, so it is tested on every platform.

use std::fmt;
use std::str::FromStr;

// The MOD_* values of RegisterHotKey.
pub const MOD_ALT: u32 = 0x0001;
pub const MOD_CONTROL: u32 = 0x0002;
pub const MOD_SHIFT: u32 = 0x0004;
pub const MOD_WIN: u32 = 0x0008;

/// The most strokes a shortcut can take.
pub const MAX_STROKES: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hotkey {
    /// A combination of the MOD_* constants above.
    pub modifiers: u32,
    /// A Win32 virtual-key code.
    pub vk: u32,
}

/// Names for the keys a window shortcut is likely to use. Anything else is
/// written as its virtual-key code in hex (`0xBA`), which round-trips too.
const KEY_NAMES: &[(&str, u32)] = &[
    ("Backspace", 0x08),
    ("Tab", 0x09),
    ("Enter", 0x0D),
    ("Escape", 0x1B),
    ("Space", 0x20),
    ("PageUp", 0x21),
    ("PageDown", 0x22),
    ("End", 0x23),
    ("Home", 0x24),
    ("Left", 0x25),
    ("Up", 0x26),
    ("Right", 0x27),
    ("Down", 0x28),
    ("Insert", 0x2D),
    ("Delete", 0x2E),
    ("NumMultiply", 0x6A),
    ("NumAdd", 0x6B),
    ("NumSubtract", 0x6D),
    ("NumDecimal", 0x6E),
    ("NumDivide", 0x6F),
    ("Oem1", 0xBA),
    ("OemPlus", 0xBB),
    ("OemComma", 0xBC),
    ("OemMinus", 0xBD),
    ("OemPeriod", 0xBE),
    ("Oem2", 0xBF),
    ("Oem3", 0xC0),
    ("Oem4", 0xDB),
    ("Oem5", 0xDC),
    ("Oem6", 0xDD),
    ("Oem7", 0xDE),
    ("Oem102", 0xE2),
];

/// The virtual-key codes of the modifier keys themselves, which cannot be the
/// key of a shortcut.
const MODIFIER_VKS: &[u32] = &[
    0x10, 0x11, 0x12, // Shift, Control, Menu
    0x5B, 0x5C, // LWin, RWin
    0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, // L/R Shift, Control, Menu
];

pub fn key_name(vk: u32) -> String {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(vk).unwrap().to_string(),
        0x60..=0x69 => format!("Num{}", vk - 0x60),
        0x70..=0x87 => format!("F{}", vk - 0x70 + 1),
        _ => KEY_NAMES
            .iter()
            .find(|(_, code)| *code == vk)
            .map(|(name, _)| (*name).to_string())
            .unwrap_or_else(|| format!("0x{vk:02X}")),
    }
}

/// The MOD_* flag a modifier key stands for, left and right alike.
pub fn modifier_of(vk: u32) -> Option<u32> {
    match vk {
        0x10 | 0xA0 | 0xA1 => Some(MOD_SHIFT),
        0x11 | 0xA2 | 0xA3 => Some(MOD_CONTROL),
        0x12 | 0xA4 | 0xA5 => Some(MOD_ALT),
        0x5B | 0x5C => Some(MOD_WIN),
        _ => None,
    }
}

/// One key as the settings window draws it: its name, or for the arrow keys
/// the direction, which reads better as a chevron than as a word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keycap {
    Name(&'static str),
    Key(u32),
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
            Keycap::Key(vk) => Some(key_name(vk)),
            Keycap::Arrow(_) => None,
        }
    }
}

/// The keys of a combination as the settings window draws them, one cap
/// each: the modifiers in the order they are written, then the key. The key
/// is left out while it is 0.
pub fn keycaps(modifiers: u32, vk: u32) -> Vec<Keycap> {
    let mut caps: Vec<Keycap> = [
        (MOD_WIN, "Win"),
        (MOD_CONTROL, "Ctrl"),
        (MOD_ALT, "Alt"),
        (MOD_SHIFT, "Shift"),
    ]
    .into_iter()
    .filter(|(m, _)| modifiers & m != 0)
    .map(|(_, name)| Keycap::Name(name))
    .collect();
    if vk != 0 {
        caps.push(match vk {
            0x25 => Keycap::Arrow(Arrow::Left),
            0x26 => Keycap::Arrow(Arrow::Up),
            0x27 => Keycap::Arrow(Arrow::Right),
            0x28 => Keycap::Arrow(Arrow::Down),
            _ => Keycap::Key(vk),
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
    held: u32,
    /// The key that is down and has been recorded, so that its repeats are
    /// not taken for another stroke.
    down: u32,
    /// What was recorded: modifiers and the keys pressed with them, one per
    /// stroke (none yet while only modifiers have been pressed).
    pub modifiers: u32,
    vks: Vec<u32>,
    /// Whether the modifiers have stayed held since the last stroke, so
    /// that the next key follows it.
    open: bool,
}

impl Recorder {
    /// Starts out showing `strokes`, with nothing held.
    pub fn showing(strokes: &[Hotkey]) -> Recorder {
        Recorder {
            modifiers: strokes.first().map_or(0, |h| h.modifiers),
            vks: strokes.iter().map(|h| h.vk).collect(),
            ..Recorder::default()
        }
    }

    /// The strokes recorded so far.
    pub fn strokes(&self) -> Vec<Hotkey> {
        self.vks
            .iter()
            .map(|&vk| Hotkey {
                modifiers: self.modifiers,
                vk,
            })
            .collect()
    }

    pub fn key_down(&mut self, vk: u32) {
        match modifier_of(vk) {
            // Already held: a repeat, or the other of a left-right pair.
            Some(m) if self.held & m != 0 => {}
            Some(m) => {
                self.held |= m;
                self.modifiers = self.held;
                self.vks.clear();
                self.open = false;
            }
            None if vk == self.down => {}
            None => {
                let follows =
                    self.open && self.vks.len() < MAX_STROKES && self.vks.last() != Some(&vk);
                if !follows {
                    self.modifiers = self.held;
                    self.vks.clear();
                }
                self.vks.push(vk);
                self.down = vk;
                self.open = true;
            }
        }
    }

    pub fn key_up(&mut self, vk: u32) {
        if let Some(m) = modifier_of(vk) {
            self.held &= !m;
            self.open = false;
        }
        if vk == self.down {
            self.down = 0;
        }
    }

    /// What was recorded as a shortcut, or why it cannot be one. `None`
    /// while no key other than modifiers has been pressed.
    pub fn result(&self) -> Option<Result<Keys, HotkeyError>> {
        (!self.vks.is_empty()).then(|| Keys::new(self.strokes()))
    }
}

fn parse_key(name: &str) -> Option<u32> {
    let upper = name.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    if bytes.len() == 1 && (bytes[0].is_ascii_uppercase() || bytes[0].is_ascii_digit()) {
        return Some(u32::from(bytes[0]));
    }
    if let Some(hex) = upper.strip_prefix("0X") {
        return u32::from_str_radix(hex, 16)
            .ok()
            .filter(|vk| (0x01..=0xFE).contains(vk));
    }
    if let Some(n) = upper
        .strip_prefix("NUM")
        .and_then(|d| d.parse::<u32>().ok())
    {
        return (n <= 9).then_some(0x60 + n);
    }
    if let Some(n) = upper.strip_prefix('F').and_then(|d| d.parse::<u32>().ok()) {
        return (1..=24).contains(&n).then_some(0x70 + n - 1);
    }
    KEY_NAMES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, vk)| *vk)
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

impl Hotkey {
    pub fn new(modifiers: u32, vk: u32) -> Result<Self, HotkeyError> {
        if MODIFIER_VKS.contains(&vk) {
            return Err(HotkeyError::ModifierAsKey);
        }
        if modifiers & (MOD_CONTROL | MOD_ALT | MOD_WIN) == 0 {
            return Err(HotkeyError::NeedsModifier);
        }
        Ok(Hotkey { modifiers, vk })
    }
}

impl FromStr for Hotkey {
    type Err = HotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.trim().is_empty() {
            return Err(HotkeyError::Empty);
        }
        let mut modifiers = 0;
        let mut vk = None;
        for part in s.split('+').map(str::trim) {
            let modifier = match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => Some(MOD_CONTROL),
                "alt" => Some(MOD_ALT),
                "shift" => Some(MOD_SHIFT),
                "win" => Some(MOD_WIN),
                _ => None,
            };
            if let Some(m) = modifier {
                modifiers |= m;
                continue;
            }
            let key = parse_key(part).ok_or_else(|| HotkeyError::UnknownPart(part.to_string()))?;
            if vk.replace(key).is_some() {
                return Err(HotkeyError::TwoKeys);
            }
        }
        Hotkey::new(modifiers, vk.ok_or(HotkeyError::NoKey)?)
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (flag, name) in [
            (MOD_WIN, "Win"),
            (MOD_CONTROL, "Ctrl"),
            (MOD_ALT, "Alt"),
            (MOD_SHIFT, "Shift"),
        ] {
            if self.modifiers & flag != 0 {
                write!(f, "{name}+")?;
            }
        }
        f.write_str(&key_name(self.vk))
    }
}

/// A shortcut: one combination, or two keys pressed one after the other
/// while the same modifiers stay held ("Ctrl+Left, Ctrl+Up"). Each stroke is
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
        for h in &strokes {
            Hotkey::new(h.modifiers, h.vk)?;
        }
        if strokes.iter().any(|h| h.modifiers != strokes[0].modifiers) {
            return Err(HotkeyError::ModifiersDiffer);
        }
        if strokes.windows(2).any(|w| w[0].vk == w[1].vk) {
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

impl FromStr for Keys {
    type Err = HotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let strokes = s
            .split(',')
            .map(str::parse)
            .collect::<Result<Vec<Hotkey>, _>>()?;
        Keys::new(strokes)
    }
}

impl fmt::Display for Keys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, h) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{h}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hk(s: &str) -> Hotkey {
        s.parse().unwrap()
    }

    #[test]
    fn parses_modifiers_and_named_keys() {
        assert_eq!(
            hk("Ctrl+Alt+Left"),
            Hotkey {
                modifiers: MOD_CONTROL | MOD_ALT,
                vk: 0x25
            }
        );
        assert_eq!(hk("win + shift + F12").vk, 0x7B);
        assert_eq!(hk("win+shift+F12").modifiers, MOD_WIN | MOD_SHIFT);
        assert_eq!(hk("Control+Alt+c").vk, u32::from(b'C'));
        assert_eq!(hk("Ctrl+Alt+7").vk, u32::from(b'7'));
        assert_eq!(hk("Ctrl+Alt+Num5").vk, 0x65);
        assert_eq!(hk("Ctrl+Alt+enter").vk, 0x0D);
        assert_eq!(hk("Ctrl+Alt+0xBA").vk, 0xBA);
    }

    #[test]
    fn writes_modifiers_in_a_fixed_order() {
        assert_eq!(
            hk("Shift+Alt+Win+Ctrl+Up").to_string(),
            "Win+Ctrl+Alt+Shift+Up"
        );
        assert_eq!(hk("ctrl+alt+c").to_string(), "Ctrl+Alt+C");
    }

    #[test]
    fn every_key_it_writes_reads_back() {
        for vk in 0x01..=0xFE {
            if MODIFIER_VKS.contains(&vk) {
                continue;
            }
            let h = Hotkey::new(MOD_CONTROL | MOD_ALT, vk).unwrap();
            assert_eq!(h.to_string().parse::<Hotkey>(), Ok(h), "vk 0x{vk:02X}");
        }
    }

    fn keys(s: &str) -> Keys {
        s.parse().unwrap()
    }

    #[test]
    fn records_the_modifiers_held_with_a_key() {
        let mut r = Recorder::default();
        assert_eq!(r.result(), None);
        r.key_down(0xA2); // left Ctrl
        r.key_down(0xA1); // right Shift
        assert_eq!(
            (r.modifiers, r.strokes()),
            (MOD_CONTROL | MOD_SHIFT, vec![])
        );
        assert_eq!(r.result(), None);
        r.key_down(0x25);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Shift+Left"))));
        // Held keys repeat; that records nothing more.
        r.key_down(0x25);
        r.key_down(0xA2);
        r.key_up(0x25);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Shift+Left"))));
        r.key_up(0xA1);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Shift+Left"))));
        // Shift was let go: the next key starts over with Ctrl alone.
        r.key_down(0x4B);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+K"))));
    }

    #[test]
    fn a_second_key_with_the_modifiers_still_held_follows() {
        let mut r = Recorder::default();
        r.key_down(0xA2);
        r.key_down(0x25);
        r.key_up(0x25);
        r.key_down(0x26);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Left, Ctrl+Up"))));
        // A third starts over, and so does the same key twice.
        r.key_up(0x26);
        r.key_down(0x28);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Down"))));
        r.key_up(0x28);
        r.key_down(0x28);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Down"))));
    }

    #[test]
    fn letting_go_of_a_modifier_ends_the_shortcut() {
        let mut r = Recorder::default();
        r.key_down(0xA2);
        r.key_down(0x25);
        r.key_up(0x25);
        r.key_up(0xA2);
        r.key_down(0xA2);
        assert_eq!((r.modifiers, r.strokes()), (MOD_CONTROL, vec![]));
        r.key_down(0x26);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Up"))));
        // What it starts out showing is not followed either.
        let mut r = Recorder::showing(keys("Ctrl+Alt+K").strokes());
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Alt+K"))));
        r.key_down(0xA2);
        r.key_down(0xA4);
        r.key_down(0x4A);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Alt+J"))));
    }

    #[test]
    fn a_modifier_starts_the_recording_over() {
        let mut r = Recorder::default();
        r.key_down(0x5B);
        r.key_down(0x41);
        r.key_up(0x41);
        r.key_up(0x5B);
        r.key_down(0xA4); // left Alt
        assert_eq!((r.modifiers, r.strokes()), (MOD_ALT, vec![]));
        assert_eq!(r.result(), None);
    }

    #[test]
    fn says_why_a_recorded_combination_cannot_be_used() {
        let mut r = Recorder::default();
        r.key_down(0x10);
        r.key_down(0x41);
        assert_eq!(r.result(), Some(Err(HotkeyError::NeedsModifier)));
    }

    #[test]
    fn sequences_are_written_with_commas() {
        let k = keys("ctrl+left ,ctrl + up");
        assert_eq!(k.strokes(), [hk("Ctrl+Left"), hk("Ctrl+Up")]);
        assert_eq!(k.to_string(), "Ctrl+Left, Ctrl+Up");
        assert_eq!(k.to_string().parse::<Keys>(), Ok(k.clone()));
        assert_eq!((k.first(), k.last()), (hk("Ctrl+Left"), hk("Ctrl+Up")));
        assert_eq!(keys("Ctrl+Alt+K"), Keys::from(hk("Ctrl+Alt+K")));
        assert_eq!("Ctrl+A,".parse::<Keys>(), Err(HotkeyError::Empty));
        assert_eq!(
            "Ctrl+A, Shift+B".parse::<Keys>(),
            Err(HotkeyError::NeedsModifier)
        );
        assert_eq!(
            "Ctrl+A, Ctrl+B, Ctrl+C".parse::<Keys>(),
            Err(HotkeyError::TooManyStrokes)
        );
        assert_eq!(
            "Ctrl+Left, Shift+Up".parse::<Keys>(),
            Err(HotkeyError::NeedsModifier)
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
            "Ctrl+Left, Ctrl+Left".parse::<Keys>(),
            Err(HotkeyError::SameKeyTwice)
        );
    }

    #[test]
    fn keycaps_follow_the_written_order() {
        assert_eq!(
            keycaps(MOD_SHIFT | MOD_CONTROL | MOD_WIN, 0x25),
            [
                Keycap::Name("Win"),
                Keycap::Name("Ctrl"),
                Keycap::Name("Shift"),
                Keycap::Arrow(Arrow::Left)
            ]
        );
        let labels: Vec<_> = keycaps(MOD_CONTROL | MOD_ALT, 0x0D)
            .into_iter()
            .map(|c| c.label().unwrap())
            .collect();
        assert_eq!(labels, ["Ctrl", "Alt", "Enter"]);
        assert_eq!(keycaps(MOD_ALT, 0), [Keycap::Name("Alt")]);
        assert_eq!(keycaps(0, 0x28), [Keycap::Arrow(Arrow::Down)]);
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
            Hotkey::new(MOD_CONTROL, 0x10),
            Err(HotkeyError::ModifierAsKey)
        );
    }
}
