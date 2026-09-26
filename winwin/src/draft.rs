//! A shortcut as the settings window edits it: every field as the user left
//! it, including ones that do not make a valid shortcut yet. Turning a draft
//! into a [`Shortcut`] is where the fields are checked.
//!
//! Nothing here touches Win32, so it is tested on every platform.

use crate::config::Shortcut;
use crate::hotkey::{self, Hotkey};
use crate::layout::{Anchor, Length, Placement};

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub name: String,
    /// MOD_* flags and a virtual-key code; `vk` is 0 until a key is chosen.
    pub modifiers: u32,
    pub vk: u32,
    pub anchor: Anchor,
    pub width: String,
    pub height: String,
    pub offset_x: String,
    pub offset_y: String,
}

impl Draft {
    /// What 追加 starts from.
    pub fn new() -> Draft {
        Draft {
            name: "新しい配置".into(),
            modifiers: 0,
            vk: 0,
            anchor: Anchor::Center,
            width: "50%".into(),
            height: "50%".into(),
            offset_x: Length::ZERO.to_string(),
            offset_y: Length::ZERO.to_string(),
        }
    }

    pub fn from_shortcut(s: &Shortcut) -> Draft {
        let p = &s.placement;
        Draft {
            name: s.name.clone(),
            modifiers: s.keys.modifiers,
            vk: s.keys.vk,
            anchor: p.anchor,
            width: p.width.to_string(),
            height: p.height.to_string(),
            offset_x: p.offset_x.to_string(),
            offset_y: p.offset_y.to_string(),
        }
    }

    /// A copy that can be saved next to the original: the shortcut is left
    /// for the user to choose, since two entries cannot share one.
    pub fn duplicate(&self) -> Draft {
        Draft {
            name: format!("{} のコピー", self.name),
            modifiers: 0,
            vk: 0,
            ..self.clone()
        }
    }

    pub fn placement(&self) -> Result<Placement, String> {
        let field =
            |label: &str, text: &str| text.parse::<Length>().map_err(|e| format!("{label}: {e}"));
        Ok(Placement {
            anchor: self.anchor,
            width: field("幅", &self.width)?,
            height: field("高さ", &self.height)?,
            offset_x: field("横のずらし幅", &self.offset_x)?,
            offset_y: field("縦のずらし幅", &self.offset_y)?,
        })
    }

    pub fn to_shortcut(&self) -> Result<Shortcut, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("名前を入力してください".into());
        }
        if self.vk == 0 {
            return Err("ショートカットを入力してください".into());
        }
        let keys = Hotkey::new(self.modifiers, self.vk).map_err(|e| e.to_string())?;
        Ok(Shortcut {
            name: name.into(),
            keys,
            placement: self.placement()?,
        })
    }

    /// The line in the list of shortcuts.
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
        format!("{}    {keys}", self.name)
    }
}

// The hotkey control reports keys as a virtual-key code in the low byte and
// HOTKEYF_* flags in the high byte. It has no flag for the Windows key, which
// the settings window takes from a check box beside it instead.
const HOTKEYF_SHIFT: u16 = 0x01;
const HOTKEYF_CONTROL: u16 = 0x02;
const HOTKEYF_ALT: u16 = 0x04;
const HOTKEYF_EXT: u16 = 0x08;

/// Keys that exist twice on a keyboard, where the control needs HOTKEYF_EXT
/// to name the one away from the numeric keypad (矢印 rather than テンキー 4).
fn is_extended(vk: u32) -> bool {
    matches!(vk, 0x21..=0x28 | 0x2D | 0x2E | 0x6F | 0x90)
}

/// Reads the control's value. The Windows key is not part of it.
pub fn from_hotkey_control(value: u16) -> (u32, u32) {
    let vk = u32::from(value & 0xFF);
    let flags = value >> 8;
    let mut modifiers = 0;
    for (flag, m) in [
        (HOTKEYF_SHIFT, hotkey::MOD_SHIFT),
        (HOTKEYF_CONTROL, hotkey::MOD_CONTROL),
        (HOTKEYF_ALT, hotkey::MOD_ALT),
    ] {
        if flags & flag != 0 {
            modifiers |= m;
        }
    }
    (modifiers, vk)
}

/// The value to put in the control. The Windows key is dropped.
pub fn to_hotkey_control(modifiers: u32, vk: u32) -> u16 {
    if vk == 0 {
        return 0;
    }
    let mut flags = 0;
    for (flag, m) in [
        (HOTKEYF_SHIFT, hotkey::MOD_SHIFT),
        (HOTKEYF_CONTROL, hotkey::MOD_CONTROL),
        (HOTKEYF_ALT, hotkey::MOD_ALT),
    ] {
        if modifiers & m != 0 {
            flags |= flag;
        }
    }
    if is_extended(vk) {
        flags |= HOTKEYF_EXT;
    }
    (flags << 8) | (vk as u16 & 0xFF)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::hotkey::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};

    #[test]
    fn a_saved_shortcut_comes_back_unchanged() {
        for s in Config::defaults().shortcuts {
            assert_eq!(Draft::from_shortcut(&s).to_shortcut().unwrap().keys, s.keys);
            let placement = Draft::from_shortcut(&s).to_shortcut().unwrap().placement;
            assert_eq!(placement.anchor, s.placement.anchor);
            assert!((placement.width.value() - s.placement.width.value()).abs() < 0.001);
        }
    }

    #[test]
    fn a_new_entry_needs_a_shortcut_before_it_saves() {
        let mut d = Draft::new();
        assert_eq!(
            d.to_shortcut().unwrap_err(),
            "ショートカットを入力してください"
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

        d = Draft::from_shortcut(&Config::defaults().shortcuts[0]);
        d.name = " ".into();
        assert_eq!(d.to_shortcut().unwrap_err(), "名前を入力してください");

        d = Draft::from_shortcut(&Config::defaults().shortcuts[0]);
        d.modifiers = MOD_SHIFT;
        assert!(d.to_shortcut().unwrap_err().contains("Ctrl・Alt・Win"));
    }

    #[test]
    fn a_duplicate_keeps_the_placement_but_not_the_shortcut() {
        let original = Draft::from_shortcut(&Config::defaults().shortcuts[0]);
        let copy = original.duplicate();
        assert_eq!(copy.name, "左半分 のコピー");
        assert_eq!(copy.vk, 0);
        assert_eq!(copy.placement(), original.placement());
    }

    #[test]
    fn list_lines_show_the_shortcut() {
        let d = Draft::from_shortcut(&Config::defaults().shortcuts[0]);
        assert_eq!(d.list_text(), "左半分    Ctrl+Alt+Left");
        assert_eq!(Draft::new().list_text(), "新しい配置    (未設定)");
    }

    #[test]
    fn hotkey_control_values() {
        // Ctrl+Alt+Left, as the control reports it: extended, since it is
        // the arrow key rather than the keypad 4.
        let value = to_hotkey_control(MOD_CONTROL | MOD_ALT, 0x25);
        assert_eq!(
            value,
            ((HOTKEYF_CONTROL | HOTKEYF_ALT | HOTKEYF_EXT) << 8) | 0x25
        );
        assert_eq!(from_hotkey_control(value), (MOD_CONTROL | MOD_ALT, 0x25));
        // Win is not the control's to show.
        assert_eq!(
            to_hotkey_control(MOD_WIN | MOD_SHIFT, 0x41),
            (HOTKEYF_SHIFT << 8) | 0x41
        );
        assert_eq!(to_hotkey_control(MOD_CONTROL, 0), 0);
        assert_eq!(from_hotkey_control(0), (0, 0));
    }
}
