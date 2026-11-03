//! meta-gesture's keys as Windows names them: virtual-key codes. The config
//! file writes a key with no name as its virtual-key code (`0xAD`), so
//! reading the file needs this table on every platform; `src/win/` uses it
//! to send combinations and to follow the keys pressed while one is
//! recorded.
//!
//! Nothing here touches Win32, so it is tested on every platform.

use crate::keys::{Key, Modifiers, Press};

/// The named keys other than letters, digits, `Num0`-`Num9` and `F1`-`F24`,
/// whose codes follow from their numbers.
const CODES: &[(Key, u32)] = &[
    (Key::Backspace, 0x08),
    (Key::Tab, 0x09),
    (Key::Enter, 0x0D),
    (Key::Escape, 0x1B),
    (Key::Space, 0x20),
    (Key::PageUp, 0x21),
    (Key::PageDown, 0x22),
    (Key::End, 0x23),
    (Key::Home, 0x24),
    (Key::Left, 0x25),
    (Key::Up, 0x26),
    (Key::Right, 0x27),
    (Key::Down, 0x28),
    (Key::Insert, 0x2D),
    (Key::Delete, 0x2E),
    (Key::NumMultiply, 0x6A),
    (Key::NumAdd, 0x6B),
    (Key::NumSubtract, 0x6D),
    (Key::NumDecimal, 0x6E),
    (Key::NumDivide, 0x6F),
    (Key::Oem1, 0xBA),
    (Key::OemPlus, 0xBB),
    (Key::OemComma, 0xBC),
    (Key::OemMinus, 0xBD),
    (Key::OemPeriod, 0xBE),
    (Key::Oem2, 0xBF),
    (Key::Oem3, 0xC0),
    (Key::Oem4, 0xDB),
    (Key::Oem5, 0xDC),
    (Key::Oem6, 0xDD),
    (Key::Oem7, 0xDE),
    (Key::Oem102, 0xE2),
];

/// What the virtual-key code `vk` stands for: a modifier, left and right
/// alike, or a key. `None` for the codes outside `0x01..=0xFE`, which no key
/// has.
pub fn press(vk: u32) -> Option<Press> {
    let modifier = match vk {
        0x10 | 0xA0 | 0xA1 => Some(Modifiers::SHIFT),
        0x11 | 0xA2 | 0xA3 => Some(Modifiers::CTRL),
        0x12 | 0xA4 | 0xA5 => Some(Modifiers::ALT),
        0x5B | 0x5C => Some(Modifiers::META),
        _ => None,
    };
    if let Some(m) = modifier {
        return Some(Press::Modifier(m));
    }
    let byte = u8::try_from(vk)
        .ok()
        .filter(|b| (0x01..=0xFE).contains(b))?;
    let key = match byte {
        b'A'..=b'Z' => Key::Letter(byte),
        b'0'..=b'9' => Key::Digit(byte - b'0'),
        0x60..=0x69 => Key::Num(byte - 0x60),
        0x70..=0x87 => Key::F(byte - 0x70 + 1),
        _ => CODES
            .iter()
            .find(|(_, code)| *code == vk)
            .map_or(Key::Code(byte), |(key, _)| *key),
    };
    Some(Press::Key(key))
}

/// The virtual-key code of `key`.
pub fn code(key: Key) -> u32 {
    match key {
        Key::Letter(c) => u32::from(c),
        Key::Digit(d) => u32::from(b'0' + d),
        Key::Num(d) => 0x60 + u32::from(d),
        Key::F(n) => 0x70 + u32::from(n) - 1,
        Key::Code(code) => u32::from(code),
        key => CODES
            .iter()
            .find(|(k, _)| *k == key)
            .map_or(0, |(_, code)| *code),
    }
}

/// The virtual-key codes of the modifiers, as a combination presses them:
/// the left-hand keys.
pub fn modifier_codes(modifiers: Modifiers) -> Vec<u32> {
    [
        (Modifiers::META, 0x5B),
        (Modifiers::CTRL, 0xA2),
        (Modifiers::ALT, 0xA4),
        (Modifiers::SHIFT, 0xA0),
    ]
    .into_iter()
    .filter(|(m, _)| modifiers.contains(*m))
    .map(|(_, code)| code)
    .collect()
}

/// Whether Windows takes `key` as an extended key, which a key sent in its
/// place has to say: the Insert, Delete, Home, End, Page Up, Page Down and
/// arrow keys beside the keypad, and the keypad's divide key. Without the
/// flag, Windows reads them as the keypad's keys of the same codes.
pub fn is_extended(key: Key) -> bool {
    matches!(
        key,
        Key::Insert
            | Key::Delete
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown
            | Key::Left
            | Key::Up
            | Key::Right
            | Key::Down
            | Key::NumDivide
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Combo;

    #[test]
    fn every_key_it_writes_reads_back() {
        let mut modifiers = 0;
        for vk in 0x01..=0xFE {
            match press(vk) {
                Some(Press::Key(key)) => {
                    assert_eq!(code(key), vk, "vk 0x{vk:02X}");
                    let c = Combo::new(Modifiers::CTRL, key);
                    assert_eq!(c.to_string().parse::<Combo>(), Ok(c), "vk 0x{vk:02X}");
                }
                Some(Press::Modifier(_)) => modifiers += 1,
                None => panic!("vk 0x{vk:02X}"),
            }
        }
        assert_eq!(modifiers, 11);
        assert_eq!(press(0x00), None);
        assert_eq!(press(0xFF), None);
    }

    #[test]
    fn presses_the_left_modifiers() {
        assert_eq!(
            modifier_codes(Modifiers::CTRL | Modifiers::SHIFT),
            vec![0xA2, 0xA0]
        );
        for code in
            modifier_codes(Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT | Modifiers::META)
        {
            assert!(matches!(press(code), Some(Press::Modifier(_))));
        }
    }

    #[test]
    fn marks_the_keys_beside_the_keypad_extended() {
        assert!(is_extended(Key::Left));
        assert!(is_extended(Key::Home));
        assert!(!is_extended(Key::Letter(b'W')));
        assert!(!is_extended(Key::Num(4)));
    }
}
