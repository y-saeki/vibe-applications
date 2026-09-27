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
pub const MAX_STROKES: usize = 3;
/// How long after a stroke the next one of the same shortcut may come, in
/// milliseconds. Later, it starts afresh.
pub const SEQUENCE_TIMEOUT_MS: u32 = 1500;

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

/// Follows the keys pressed while the settings window records a shortcut,
/// which may take several strokes. Pressing a key that is not a modifier
/// records a stroke of it with the modifiers held at that moment. A stroke
/// that begins within [`SEQUENCE_TIMEOUT_MS`] of the last one follows it, as
/// it would when the shortcut is used; one that begins later, or after
/// [`MAX_STROKES`], starts the recording over. Modifiers pressed for a stroke
/// show until its key is pressed or they are all let go.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recorder {
    held: u32,
    /// The key that is down and has been recorded, so that its repeats are
    /// not taken for further strokes.
    down: u32,
    /// The strokes recorded so far.
    strokes: Vec<Hotkey>,
    /// The modifiers of a stroke under way, shown after `strokes`; 0 for none.
    pub partial: u32,
    /// When the last stroke was recorded, in milliseconds.
    last: Option<u32>,
}

impl Recorder {
    /// Starts out showing `strokes`, with nothing held.
    pub fn showing(strokes: &[Hotkey]) -> Recorder {
        Recorder {
            strokes: strokes.to_vec(),
            ..Recorder::default()
        }
    }

    pub fn strokes(&self) -> &[Hotkey] {
        &self.strokes
    }

    /// Whether a stroke that begins at `time` follows the ones recorded.
    fn follows(&self, time: u32) -> bool {
        self.strokes.len() < MAX_STROKES
            && self
                .last
                .is_some_and(|t| time.wrapping_sub(t) <= SEQUENCE_TIMEOUT_MS)
    }

    /// Key `vk` went down at `time` (milliseconds, as the system counts
    /// them).
    pub fn key_down(&mut self, vk: u32, time: u32) {
        match modifier_of(vk) {
            // Already held: a repeat, or the other of a left-right pair.
            Some(m) if self.held & m != 0 => {}
            Some(m) => {
                self.held |= m;
                if self.partial == 0 && !self.follows(time) {
                    self.strokes.clear();
                }
                self.partial = self.held;
            }
            None if vk == self.down => {}
            None => {
                if self.partial == 0 && !self.follows(time) {
                    self.strokes.clear();
                }
                self.strokes.push(Hotkey {
                    modifiers: self.held,
                    vk,
                });
                self.partial = 0;
                self.down = vk;
                self.last = Some(time);
            }
        }
    }

    pub fn key_up(&mut self, vk: u32) {
        if let Some(m) = modifier_of(vk) {
            self.held &= !m;
            if self.held == 0 {
                self.partial = 0;
            }
        }
        if vk == self.down {
            self.down = 0;
        }
    }

    /// What was recorded as a shortcut, or why it cannot be one. `None`
    /// while no stroke has been recorded, or one is under way.
    pub fn result(&self) -> Option<Result<Keys, HotkeyError>> {
        (!self.strokes.is_empty() && self.partial == 0).then(|| Keys::new(self.strokes.clone()))
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
                write!(f, "続けて押せるのは {MAX_STROKES} 回までです")
            }
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

/// A shortcut: one combination, or several pressed one after another
/// ("Ctrl+Left, Ctrl+Up"). Each stroke is a valid [`Hotkey`] of its own.
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
        r.key_down(0xA2, 0); // left Ctrl
        r.key_down(0xA1, 0); // right Shift
        assert_eq!((r.strokes(), r.partial), (&[][..], MOD_CONTROL | MOD_SHIFT));
        assert_eq!(r.result(), None);
        r.key_down(0x25, 0);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Shift+Left"))));
        // Held keys repeat; that records nothing more.
        r.key_down(0x25, 30);
        r.key_down(0xA2, 30);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Shift+Left"))));
        r.key_up(0x25);
        r.key_up(0xA1);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Shift+Left"))));
    }

    #[test]
    fn a_key_pressed_soon_after_is_the_next_stroke() {
        let mut r = Recorder::default();
        r.key_down(0xA2, 0);
        r.key_down(0x25, 100);
        r.key_up(0x25);
        // Ctrl is still down: the next key goes with it.
        r.key_down(0x26, 400);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Left, Ctrl+Up"))));
        // And again, as a cycle is pressed; that is a stroke of its own here.
        r.key_up(0x26);
        r.key_down(0x26, 600);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+Left, Ctrl+Up, Ctrl+Up"))));
    }

    #[test]
    fn modifiers_let_go_between_strokes() {
        let mut r = Recorder::default();
        r.key_down(0xA2, 0);
        r.key_down(0x58, 0);
        r.key_up(0x58);
        r.key_up(0xA2);
        r.key_down(0xA4, 1000); // left Alt
        assert_eq!(r.strokes(), keys("Ctrl+X").strokes());
        assert_eq!(r.partial, MOD_ALT);
        assert_eq!(r.result(), None);
        r.key_down(0x4B, 1200);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+X, Alt+K"))));
        // A stroke begun and let go of leaves what was recorded.
        r.key_up(0x4B);
        r.key_up(0xA4);
        r.key_down(0xA2, 1500);
        r.key_up(0xA2);
        assert_eq!(r.partial, 0);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+X, Alt+K"))));
    }

    #[test]
    fn a_late_or_extra_stroke_starts_the_recording_over() {
        let mut r = Recorder::showing(keys("Ctrl+Alt+K").strokes());
        // What it starts out showing is not followed.
        r.key_down(0xA2, 0);
        r.key_down(0x41, 0);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+A"))));
        r.key_down(0x42, SEQUENCE_TIMEOUT_MS + 1);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+B"))));
        for (i, vk) in [0x43, 0x44, 0x45].into_iter().enumerate() {
            r.key_down(vk, SEQUENCE_TIMEOUT_MS + 2 + i as u32);
        }
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+E"))));
        // Modifiers pressed late start over too.
        r.key_up(0xA2);
        r.key_down(0xA4, 10_000);
        assert_eq!((r.strokes(), r.partial), (&[][..], MOD_ALT));
        // The system's clock wraps around.
        let mut r = Recorder::default();
        r.key_down(0xA2, u32::MAX - 10);
        r.key_down(0x41, u32::MAX - 10);
        r.key_down(0x42, 10);
        assert_eq!(r.result(), Some(Ok(keys("Ctrl+A, Ctrl+B"))));
    }

    #[test]
    fn says_why_a_recorded_combination_cannot_be_used() {
        let mut r = Recorder::default();
        r.key_down(0x10, 0);
        r.key_down(0x41, 0);
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
            "Ctrl+A, Ctrl+B, Ctrl+C, Ctrl+D".parse::<Keys>(),
            Err(HotkeyError::TooManyStrokes)
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
