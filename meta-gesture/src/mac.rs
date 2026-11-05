//! What the macOS app hears from the system and says back to it, in
//! meta-gesture's own types: the virtual key codes (`kVK_*` of HIToolbox's
//! Events.h) and modifier flags (`CGEventFlags`) of a key event. The app
//! hears them while the settings window records a combination, and sends
//! them when a gesture is made. Also the shortcut a menu item shows, as the
//! accessibility API reports it (`kAXMenuItemCmd*Attribute`).
//!
//! Nothing here touches the operating system, so it is tested on every
//! platform.

use crate::keys::{Combo, Key, Modifiers, Press};

// The modifier masks of CGEventFlags.
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const FLAG_OPTION: u64 = 0x0008_0000;
const FLAG_COMMAND: u64 = 0x0010_0000;

// The masks of AXMenuItemModifiers, which a menu item's
// kAXMenuItemCmdModifiersAttribute holds. Command is held unless NoCommand
// says it is not.
const MENU_SHIFT: u32 = 1 << 0;
const MENU_OPTION: u32 = 1 << 1;
const MENU_CONTROL: u32 = 1 << 2;
const MENU_NO_COMMAND: u32 = 1 << 3;

/// Keys by the characters a menu item names them by: as the US layout
/// prints them, unshifted and shifted.
const CHARS: &[(char, Option<char>, Key)] = &[
    ('1', Some('!'), Key::Digit(1)),
    ('2', Some('@'), Key::Digit(2)),
    ('3', Some('#'), Key::Digit(3)),
    ('4', Some('$'), Key::Digit(4)),
    ('5', Some('%'), Key::Digit(5)),
    ('6', Some('^'), Key::Digit(6)),
    ('7', Some('&'), Key::Digit(7)),
    ('8', Some('*'), Key::Digit(8)),
    ('9', Some('('), Key::Digit(9)),
    ('0', Some(')'), Key::Digit(0)),
    ('`', Some('~'), Key::Oem3),
    ('-', Some('_'), Key::OemMinus),
    ('=', Some('+'), Key::OemPlus),
    ('[', Some('{'), Key::Oem4),
    (']', Some('}'), Key::Oem6),
    ('\\', Some('|'), Key::Oem5),
    (';', Some(':'), Key::Oem1),
    ('\'', Some('"'), Key::Oem7),
    (',', Some('<'), Key::OemComma),
    ('.', Some('>'), Key::OemPeriod),
    ('/', Some('?'), Key::Oem2),
    (' ', None, Key::Space),
    ('\r', None, Key::Enter),
    ('\t', None, Key::Tab),
    ('\u{1B}', None, Key::Escape),
    // NSMenuItem's keyEquivalent documents Backspace and forward delete as
    // these.
    ('\u{08}', None, Key::Backspace),
    ('\u{7F}', None, Key::Delete),
];

/// Keys by their codes. A key's position decides its code, as the US layout
/// places it; the punctuation keys are meta-gesture's `Oem*` keys at the same
/// positions as on a Windows keyboard.
const CODES: &[(u16, Key)] = &[
    (0x00, Key::Letter(b'A')),
    (0x01, Key::Letter(b'S')),
    (0x02, Key::Letter(b'D')),
    (0x03, Key::Letter(b'F')),
    (0x04, Key::Letter(b'H')),
    (0x05, Key::Letter(b'G')),
    (0x06, Key::Letter(b'Z')),
    (0x07, Key::Letter(b'X')),
    (0x08, Key::Letter(b'C')),
    (0x09, Key::Letter(b'V')),
    (0x0A, Key::Oem102),
    (0x0B, Key::Letter(b'B')),
    (0x0C, Key::Letter(b'Q')),
    (0x0D, Key::Letter(b'W')),
    (0x0E, Key::Letter(b'E')),
    (0x0F, Key::Letter(b'R')),
    (0x10, Key::Letter(b'Y')),
    (0x11, Key::Letter(b'T')),
    (0x12, Key::Digit(1)),
    (0x13, Key::Digit(2)),
    (0x14, Key::Digit(3)),
    (0x15, Key::Digit(4)),
    (0x16, Key::Digit(6)),
    (0x17, Key::Digit(5)),
    (0x18, Key::OemPlus),
    (0x19, Key::Digit(9)),
    (0x1A, Key::Digit(7)),
    (0x1B, Key::OemMinus),
    (0x1C, Key::Digit(8)),
    (0x1D, Key::Digit(0)),
    (0x1E, Key::Oem6),
    (0x1F, Key::Letter(b'O')),
    (0x20, Key::Letter(b'U')),
    (0x21, Key::Oem4),
    (0x22, Key::Letter(b'I')),
    (0x23, Key::Letter(b'P')),
    (0x24, Key::Enter),
    (0x25, Key::Letter(b'L')),
    (0x26, Key::Letter(b'J')),
    (0x27, Key::Oem7),
    (0x28, Key::Letter(b'K')),
    (0x29, Key::Oem1),
    (0x2A, Key::Oem5),
    (0x2B, Key::OemComma),
    (0x2C, Key::Oem2),
    (0x2D, Key::Letter(b'N')),
    (0x2E, Key::Letter(b'M')),
    (0x2F, Key::OemPeriod),
    (0x30, Key::Tab),
    (0x31, Key::Space),
    (0x32, Key::Oem3),
    (0x33, Key::Backspace),
    (0x35, Key::Escape),
    (0x40, Key::F(17)),
    (0x41, Key::NumDecimal),
    (0x43, Key::NumMultiply),
    (0x45, Key::NumAdd),
    (0x4B, Key::NumDivide),
    // The keypad's Enter is Enter, as it is on Windows.
    (0x4C, Key::Enter),
    (0x4E, Key::NumSubtract),
    (0x4F, Key::F(18)),
    (0x50, Key::F(19)),
    (0x52, Key::Num(0)),
    (0x53, Key::Num(1)),
    (0x54, Key::Num(2)),
    (0x55, Key::Num(3)),
    (0x56, Key::Num(4)),
    (0x57, Key::Num(5)),
    (0x58, Key::Num(6)),
    (0x59, Key::Num(7)),
    (0x5A, Key::F(20)),
    (0x5B, Key::Num(8)),
    (0x5C, Key::Num(9)),
    // A JIS keyboard's ¥ and ろ, which a Windows JIS keyboard calls Oem5 and
    // Oem102.
    (0x5D, Key::Oem5),
    (0x5E, Key::Oem102),
    (0x60, Key::F(5)),
    (0x61, Key::F(6)),
    (0x62, Key::F(7)),
    (0x63, Key::F(3)),
    (0x64, Key::F(8)),
    (0x65, Key::F(9)),
    (0x67, Key::F(11)),
    (0x69, Key::F(13)),
    (0x6A, Key::F(16)),
    (0x6B, Key::F(14)),
    (0x6D, Key::F(10)),
    (0x6F, Key::F(12)),
    (0x71, Key::F(15)),
    // Help, where a Windows keyboard has Insert.
    (0x72, Key::Insert),
    (0x73, Key::Home),
    (0x74, Key::PageUp),
    // Forward delete.
    (0x75, Key::Delete),
    (0x76, Key::F(4)),
    (0x77, Key::End),
    (0x78, Key::F(2)),
    (0x79, Key::PageDown),
    (0x7A, Key::F(1)),
    (0x7B, Key::Left),
    (0x7C, Key::Right),
    (0x7D, Key::Down),
    (0x7E, Key::Up),
];

/// The key with the virtual key code `code`, unless it is a modifier or a
/// key meta-gesture has no name for.
pub fn key(code: u16) -> Option<Key> {
    CODES.iter().find(|(c, _)| *c == code).map(|(_, key)| *key)
}

/// The modifiers in the CGEventFlags `flags`: Command is meta-gesture's Win and
/// Option its Alt, the keys at the same places. Other flags, such as the
/// one arrow keys carry, are left out.
pub fn modifiers(flags: u64) -> Modifiers {
    [
        (FLAG_CONTROL, Modifiers::CTRL),
        (FLAG_OPTION, Modifiers::ALT),
        (FLAG_SHIFT, Modifiers::SHIFT),
        (FLAG_COMMAND, Modifiers::META),
    ]
    .into_iter()
    .filter(|(flag, _)| flags & flag != 0)
    .fold(Modifiers::NONE, |m, (_, modifier)| m | modifier)
}

/// The virtual key code that sends `key`: the first of the codes that are
/// it, so that Enter is the main one rather than the keypad's. `None` for a
/// key a Mac keyboard does not have.
pub fn code(key: Key) -> Option<u16> {
    CODES.iter().find(|(_, k)| *k == key).map(|(c, _)| *c)
}

/// The CGEventFlags of `modifiers`, as a key event sent with them carries.
pub fn flags(modifiers: Modifiers) -> u64 {
    [
        (Modifiers::CTRL, FLAG_CONTROL),
        (Modifiers::ALT, FLAG_OPTION),
        (Modifiers::SHIFT, FLAG_SHIFT),
        (Modifiers::META, FLAG_COMMAND),
    ]
    .into_iter()
    .filter(|(m, _)| modifiers.contains(*m))
    .fold(0, |flags, (_, flag)| flags | flag)
}

/// The combination a key event stands for, unless its key has no name.
pub fn combo(code: u16, flags: u64) -> Option<Combo> {
    Some(Combo::new(modifiers(flags), key(code)?))
}

/// The shortcut a menu item shows: its key as the character
/// `character` (kAXMenuItemCmdChar) or else the virtual key code `code`
/// (kAXMenuItemCmdVirtualKey), held with the AXMenuItemModifiers
/// `modifiers`. `None` when it has no key meta-gesture can name.
pub fn menu_shortcut(character: Option<&str>, code: Option<u16>, modifiers: u32) -> Option<Combo> {
    let (key, shifted) = character
        .and_then(char_key)
        .or_else(|| code.and_then(key).map(|k| (k, false)))?;
    let held = [
        (MENU_CONTROL, Modifiers::CTRL),
        (MENU_OPTION, Modifiers::ALT),
        (MENU_SHIFT, Modifiers::SHIFT),
    ]
    .into_iter()
    .filter(|(mask, _)| modifiers & mask != 0)
    .fold(Modifiers::NONE, |m, (_, modifier)| m | modifier);
    let shift = if shifted {
        Modifiers::SHIFT
    } else {
        Modifiers::NONE
    };
    let command = if modifiers & MENU_NO_COMMAND == 0 {
        Modifiers::META
    } else {
        Modifiers::NONE
    };
    Some(Combo::new(held | shift | command, key))
}

/// The key a single character names, and whether it is the key's shifted
/// character. A letter is its key in either case.
fn char_key(text: &str) -> Option<(Key, bool)> {
    let mut chars = text.chars();
    let (c, None) = (chars.next()?, chars.next()) else {
        return None;
    };
    if c.is_ascii_alphabetic() {
        return Some((Key::Letter(c.to_ascii_uppercase() as u8), false));
    }
    CHARS.iter().find_map(|(plain, shifted, key)| {
        if *plain == c {
            Some((*key, false))
        } else if *shifted == Some(c) {
            Some((*key, true))
        } else {
            None
        }
    })
}

/// What a change of the modifier flags from `before` to `after` presses
/// and lets go of, as a flags-changed event reports it: each modifier that
/// went down or up, and whether it went down. Those let go of come first.
pub fn modifier_changes(before: u64, after: u64) -> Vec<(Press, bool)> {
    let (before, after) = (modifiers(before), modifiers(after));
    let each = [
        Modifiers::CTRL,
        Modifiers::ALT,
        Modifiers::SHIFT,
        Modifiers::META,
    ];
    let up = each
        .into_iter()
        .filter(|m| before.contains(*m) && !after.contains(*m))
        .map(|m| (Press::Modifier(m), false));
    let down = each
        .into_iter()
        .filter(|m| after.contains(*m) && !before.contains(*m))
        .map(|m| (Press::Modifier(m), true));
    up.chain(down).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_code_is_one_key() {
        for (i, (code, _)) in CODES.iter().enumerate() {
            assert!(
                CODES[i + 1..].iter().all(|(c, _)| c != code),
                "0x{code:02X}"
            );
        }
        // Every letter and digit has a key of its own.
        for c in b'A'..=b'Z' {
            assert!(CODES.iter().any(|(_, k)| *k == Key::Letter(c)), "{c}");
        }
        for d in 0..=9 {
            assert!(CODES.iter().any(|(_, k)| *k == Key::Digit(d)), "{d}");
            assert!(CODES.iter().any(|(_, k)| *k == Key::Num(d)), "Num{d}");
        }
    }

    #[test]
    fn names_keys_by_their_place() {
        assert_eq!(key(0x00), Some(Key::Letter(b'A')));
        assert_eq!(key(0x12), Some(Key::Digit(1)));
        assert_eq!(key(0x7B), Some(Key::Left));
        assert_eq!(key(0x7E), Some(Key::Up));
        assert_eq!(key(0x24), Some(Key::Enter));
        assert_eq!(key(0x4C), Some(Key::Enter));
        assert_eq!(key(0x29), Some(Key::Oem1));
        assert_eq!(key(0x7A), Some(Key::F(1)));
        assert_eq!(key(0x5D), Some(Key::Oem5));
        // Command, Shift, Option, Control and Fn are modifiers.
        for code in [0x37, 0x38, 0x3A, 0x3B, 0x3F] {
            assert_eq!(key(code), None, "0x{code:02X}");
        }
    }

    #[test]
    fn command_and_option_are_meta_and_alt() {
        assert_eq!(modifiers(0), Modifiers::NONE);
        assert_eq!(
            modifiers(FLAG_CONTROL | FLAG_OPTION),
            Modifiers::CTRL | Modifiers::ALT
        );
        assert_eq!(
            modifiers(FLAG_COMMAND | FLAG_SHIFT),
            Modifiers::META | Modifiers::SHIFT
        );
        // An arrow key's event also carries the numeric pad and Fn flags.
        assert_eq!(
            combo(0x7B, FLAG_CONTROL | FLAG_OPTION | 0x0020_0000 | 0x0080_0000),
            Some("Ctrl+Alt+Left".parse().unwrap())
        );
        assert_eq!(combo(0x37, FLAG_COMMAND), None);
    }

    #[test]
    fn a_change_of_flags_is_modifiers_pressed_and_let_go() {
        let control = Press::Modifier(Modifiers::CTRL);
        let command = Press::Modifier(Modifiers::META);
        assert_eq!(modifier_changes(0, FLAG_CONTROL), [(control, true)]);
        assert_eq!(
            modifier_changes(FLAG_CONTROL, FLAG_COMMAND),
            [(control, false), (command, true)]
        );
        // Flags that are no modifiers change nothing.
        assert_eq!(
            modifier_changes(FLAG_CONTROL, FLAG_CONTROL | 0x0080_0000),
            []
        );
    }

    #[test]
    fn reads_a_menu_items_shortcut() {
        let k = |s: &str| Some(s.parse::<Combo>().unwrap());
        // Command is held unless the item says it is not.
        assert_eq!(menu_shortcut(Some("W"), None, 0), k("Cmd+W"));
        assert_eq!(menu_shortcut(Some("w"), None, MENU_SHIFT), k("Shift+Cmd+W"));
        assert_eq!(
            menu_shortcut(Some("T"), None, MENU_CONTROL | MENU_OPTION),
            k("Ctrl+Option+Cmd+T")
        );
        assert_eq!(
            menu_shortcut(Some("Q"), None, MENU_NO_COMMAND | MENU_CONTROL),
            k("Ctrl+Q")
        );
        // Punctuation by where the US layout has it; a shifted character
        // holds Shift.
        assert_eq!(menu_shortcut(Some("["), None, 0), k("Cmd+Oem4"));
        assert_eq!(menu_shortcut(Some("}"), None, 0), k("Shift+Cmd+Oem6"));
        assert_eq!(menu_shortcut(Some("?"), None, 0), k("Shift+Cmd+Oem2"));
        assert_eq!(menu_shortcut(Some("1"), None, 0), k("Cmd+1"));
        assert_eq!(menu_shortcut(Some("\u{08}"), None, 0), k("Cmd+Backspace"));
        // The key code where the character names no key.
        assert_eq!(menu_shortcut(Some(""), Some(0x7B), 0), k("Cmd+Left"));
        assert_eq!(menu_shortcut(None, Some(0x7A), MENU_NO_COMMAND), k("F1"));
        assert_eq!(menu_shortcut(Some("\u{F702}"), None, 0), None);
        assert_eq!(menu_shortcut(Some("ab"), None, 0), None);
        assert_eq!(menu_shortcut(None, None, 0), None);
    }

    #[test]
    fn sends_what_it_hears() {
        for (code, key) in CODES {
            let sent = super::code(*key).unwrap();
            assert_eq!(super::key(sent), Some(*key), "0x{code:02X}");
        }
        // The main Enter, not the keypad's.
        assert_eq!(code(Key::Enter), Some(0x24));
        assert_eq!(code(Key::F(24)), None);
        let all = Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT | Modifiers::META;
        assert_eq!(modifiers(flags(all)), all);
        assert_eq!(flags(Modifiers::META), FLAG_COMMAND);
    }
}
