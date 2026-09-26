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
