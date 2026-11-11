//! Turning a Qt key press into one of uQ's key spellings, for the editor's "Press the keys…"
//! field. Plain Rust: the codes are copied here, so there is no Qt dependency and it runs in the
//! page tests.
//!
//! The values are Qt 6's `Qt::Key` and `Qt::KeyboardModifier` from `qnamespace.h` (letters,
//! digits and punctuation are their ASCII codes; the rest are `0x01000000 +`). A key that has
//! no row here (or none in uQ's key list) gives `None`, and the editor's pickers cover it.

use uq_core::keybinds::combo::{self, Combo};
use uq_core::keybinds::hypr::MODIFIERS;
use uq_core::keybinds::keys;

/// `Qt::ShiftModifier`, `ControlModifier`, `AltModifier`, `MetaModifier` (`qnamespace.h`).
const SHIFT: u32 = 0x0200_0000;
const CONTROL: u32 = 0x0400_0000;
const ALT: u32 = 0x0800_0000;
const META: u32 = 0x1000_0000;

/// Keys that are only a modifier (or a lock): pressing one waits for the next key.
const MODIFIER_KEYS: [i32; 9] = [
    0x0100_0020, // Key_Shift
    0x0100_0021, // Key_Control
    0x0100_0022, // Key_Meta
    0x0100_0023, // Key_Alt
    0x0100_0024, // Key_CapsLock
    0x0100_0025, // Key_NumLock
    0x0100_0053, // Key_Super_L
    0x0100_0054, // Key_Super_R
    0x0100_1103, // Key_AltGr
];

/// `Qt::Key_*` → uQ's spelling, for the keys that aren't a letter, digit or F key.
const NAMED: &[(i32, &str)] = &[
    (0x20, "Space"),
    (0x2c, "comma"),
    (0x2d, "minus"),
    (0x2e, "period"),
    (0x2f, "slash"),
    (0x27, "apostrophe"),
    (0x3b, "semicolon"),
    (0x3d, "equal"),
    (0x5b, "bracketleft"),
    (0x5c, "backslash"),
    (0x5d, "bracketright"),
    (0x60, "grave"), // Key_QuoteLeft
    (0x0100_0000, "Escape"),
    (0x0100_0001, "Tab"),
    (0x0100_0002, "Tab"), // Key_Backtab: Shift + Tab
    (0x0100_0003, "BackSpace"),
    (0x0100_0004, "Return"),
    (0x0100_0005, "Return"), // Key_Enter (the keypad's)
    (0x0100_0006, "Insert"),
    (0x0100_0007, "Delete"),
    (0x0100_0009, "Print"),
    (0x0100_0010, "Home"),
    (0x0100_0011, "End"),
    (0x0100_0012, "left"),
    (0x0100_0013, "up"),
    (0x0100_0014, "right"),
    (0x0100_0015, "down"),
    (0x0100_0016, "Prior"), // Key_PageUp
    (0x0100_0017, "Next"),  // Key_PageDown
    (0x0100_0070, "XF86AudioLowerVolume"),
    (0x0100_0071, "XF86AudioMute"),
    (0x0100_0072, "XF86AudioRaiseVolume"),
    (0x0100_0080, "XF86AudioPlay"),  // Key_MediaPlay
    (0x0100_0081, "XF86AudioStop"),  // Key_MediaStop
    (0x0100_0082, "XF86AudioPrev"),  // Key_MediaPrevious
    (0x0100_0083, "XF86AudioNext"),  // Key_MediaNext
    (0x0100_0085, "XF86AudioPause"), // Key_MediaPause
    (0x0100_0086, "XF86AudioPlay"),  // Key_MediaTogglePlayPause
    (0x0100_0090, "XF86HomePage"),   // Key_HomePage
    (0x0100_0092, "XF86Search"),     // Key_Search
    (0x0100_00a0, "XF86Mail"),       // Key_LaunchMail
    (0x0100_00a3, "XF86Calculator"), // Key_Launch1
    (0x0100_00b2, "XF86MonBrightnessUp"),
    (0x0100_00b3, "XF86MonBrightnessDown"),
];

/// What Shift turns a key into on a US keyboard, which is what Qt then reports: Shift + 1 is
/// `Key_Exclam`. The shortcut is Shift plus the key under it.
const SHIFTED: &[(char, &str)] = &[
    ('!', "1"),
    ('@', "2"),
    ('#', "3"),
    ('$', "4"),
    ('%', "5"),
    ('^', "6"),
    ('&', "7"),
    ('*', "8"),
    ('(', "9"),
    (')', "0"),
    ('_', "minus"),
    ('+', "equal"),
    ('{', "bracketleft"),
    ('}', "bracketright"),
    (':', "semicolon"),
    ('"', "apostrophe"),
    ('~', "grave"),
    ('<', "comma"),
    ('>', "period"),
    ('?', "slash"),
    ('|', "backslash"),
];

fn spelling(key: i32, shift: bool) -> Option<String> {
    let ch = u32::try_from(key).ok().and_then(char::from_u32);
    if let Some(c @ ('A'..='Z' | '0'..='9')) = ch {
        return Some(c.to_string());
    }
    // F1 … F24 (`Key_F1` is 0x01000030).
    if (0x0100_0030..=0x0100_0047).contains(&key) {
        return Some(format!("F{}", key - 0x0100_0030 + 1));
    }
    if let Some((_, s)) = NAMED.iter().find(|(k, _)| *k == key) {
        return Some((*s).to_string());
    }
    if shift && let Some(c) = ch {
        return SHIFTED
            .iter()
            .find(|(s, _)| *s == c)
            .map(|(_, name)| (*name).to_string());
    }
    None
}

/// The shortcut `key` and `modifiers` (Qt's `event.key` and `event.modifiers`) make, in the
/// canonical form (`SUPER + SHIFT + Q`). `None` for a modifier pressed on its own (keep
/// waiting) and for a key uQ has no name for.
pub fn combo_from_qt(key: i32, modifiers: u32) -> Option<String> {
    if MODIFIER_KEYS.contains(&key) {
        return None;
    }
    let shift = modifiers & SHIFT != 0 || key == 0x0100_0002;
    let name = spelling(key, shift)?;
    let (key_index, _) = keys::find(&name)?;
    let mut mods = 0u8;
    for (word, bit, _) in MODIFIERS {
        let held = match word {
            "SUPER" => modifiers & META != 0,
            "SHIFT" => shift,
            "CTRL" => modifiers & CONTROL != 0,
            _ => modifiers & ALT != 0,
        };
        if held {
            mods |= bit;
        }
    }
    Some(combo::canonical(Combo {
        mods,
        key: key_index as u16,
    }))
}
