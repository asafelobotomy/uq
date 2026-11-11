//! uQ's closed key list: the spellings a shortcut may use, with plain labels.
//!
//! Spellings are what Hyprland's `hl.bind` takes (xkb keysym names, matched case-insensitively,
//! and its own `mouse_*` / `mouse:<n>` forms). `XF86` keys keep their `XF86` prefix here, as
//! `base.lua` writes them; their labels never show it.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    Key,
    Wheel,
    Button,
    ModKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Letter,
    Digit,
    Function,
    Navigation,
    Editing,
    Punctuation,
    Media,
    Mouse,
}

#[derive(Debug, Clone, Copy)]
pub struct KeyRow {
    pub spelling: &'static str,
    pub label: &'static str,
    pub kind: KeyKind,
    pub group: Group,
}

const fn row(spelling: &'static str, label: &'static str, kind: KeyKind, group: Group) -> KeyRow {
    KeyRow {
        spelling,
        label,
        kind,
        group,
    }
}

pub static KEYS: &[KeyRow] = &[
    row("A", "A", KeyKind::Key, Group::Letter),
    row("B", "B", KeyKind::Key, Group::Letter),
    row("C", "C", KeyKind::Key, Group::Letter),
    row("D", "D", KeyKind::Key, Group::Letter),
    row("E", "E", KeyKind::Key, Group::Letter),
    row("F", "F", KeyKind::Key, Group::Letter),
    row("G", "G", KeyKind::Key, Group::Letter),
    row("H", "H", KeyKind::Key, Group::Letter),
    row("I", "I", KeyKind::Key, Group::Letter),
    row("J", "J", KeyKind::Key, Group::Letter),
    row("K", "K", KeyKind::Key, Group::Letter),
    row("L", "L", KeyKind::Key, Group::Letter),
    row("M", "M", KeyKind::Key, Group::Letter),
    row("N", "N", KeyKind::Key, Group::Letter),
    row("O", "O", KeyKind::Key, Group::Letter),
    row("P", "P", KeyKind::Key, Group::Letter),
    row("Q", "Q", KeyKind::Key, Group::Letter),
    row("R", "R", KeyKind::Key, Group::Letter),
    row("S", "S", KeyKind::Key, Group::Letter),
    row("T", "T", KeyKind::Key, Group::Letter),
    row("U", "U", KeyKind::Key, Group::Letter),
    row("V", "V", KeyKind::Key, Group::Letter),
    row("W", "W", KeyKind::Key, Group::Letter),
    row("X", "X", KeyKind::Key, Group::Letter),
    row("Y", "Y", KeyKind::Key, Group::Letter),
    row("Z", "Z", KeyKind::Key, Group::Letter),
    row("0", "0", KeyKind::Key, Group::Digit),
    row("1", "1", KeyKind::Key, Group::Digit),
    row("2", "2", KeyKind::Key, Group::Digit),
    row("3", "3", KeyKind::Key, Group::Digit),
    row("4", "4", KeyKind::Key, Group::Digit),
    row("5", "5", KeyKind::Key, Group::Digit),
    row("6", "6", KeyKind::Key, Group::Digit),
    row("7", "7", KeyKind::Key, Group::Digit),
    row("8", "8", KeyKind::Key, Group::Digit),
    row("9", "9", KeyKind::Key, Group::Digit),
    row("F1", "F1", KeyKind::Key, Group::Function),
    row("F2", "F2", KeyKind::Key, Group::Function),
    row("F3", "F3", KeyKind::Key, Group::Function),
    row("F4", "F4", KeyKind::Key, Group::Function),
    row("F5", "F5", KeyKind::Key, Group::Function),
    row("F6", "F6", KeyKind::Key, Group::Function),
    row("F7", "F7", KeyKind::Key, Group::Function),
    row("F8", "F8", KeyKind::Key, Group::Function),
    row("F9", "F9", KeyKind::Key, Group::Function),
    row("F10", "F10", KeyKind::Key, Group::Function),
    row("F11", "F11", KeyKind::Key, Group::Function),
    row("F12", "F12", KeyKind::Key, Group::Function),
    row("F13", "F13", KeyKind::Key, Group::Function),
    row("F14", "F14", KeyKind::Key, Group::Function),
    row("F15", "F15", KeyKind::Key, Group::Function),
    row("F16", "F16", KeyKind::Key, Group::Function),
    row("F17", "F17", KeyKind::Key, Group::Function),
    row("F18", "F18", KeyKind::Key, Group::Function),
    row("F19", "F19", KeyKind::Key, Group::Function),
    row("F20", "F20", KeyKind::Key, Group::Function),
    row("F21", "F21", KeyKind::Key, Group::Function),
    row("F22", "F22", KeyKind::Key, Group::Function),
    row("F23", "F23", KeyKind::Key, Group::Function),
    row("F24", "F24", KeyKind::Key, Group::Function),
    row("Return", "Enter", KeyKind::Key, Group::Editing),
    row("Space", "Space", KeyKind::Key, Group::Editing),
    row("Tab", "Tab", KeyKind::Key, Group::Editing),
    row("Escape", "Escape", KeyKind::Key, Group::Editing),
    row("BackSpace", "Backspace", KeyKind::Key, Group::Editing),
    row("Delete", "Delete", KeyKind::Key, Group::Editing),
    row("Insert", "Insert", KeyKind::Key, Group::Editing),
    row("Home", "Home", KeyKind::Key, Group::Navigation),
    row("End", "End", KeyKind::Key, Group::Navigation),
    row("Prior", "Page up", KeyKind::Key, Group::Navigation),
    row("Next", "Page down", KeyKind::Key, Group::Navigation),
    row("left", "Left arrow", KeyKind::Key, Group::Navigation),
    row("right", "Right arrow", KeyKind::Key, Group::Navigation),
    row("up", "Up arrow", KeyKind::Key, Group::Navigation),
    row("down", "Down arrow", KeyKind::Key, Group::Navigation),
    row("Print", "Print Screen", KeyKind::Key, Group::Function),
    row("minus", "-", KeyKind::Key, Group::Punctuation),
    row("equal", "=", KeyKind::Key, Group::Punctuation),
    row("bracketleft", "[", KeyKind::Key, Group::Punctuation),
    row("bracketright", "]", KeyKind::Key, Group::Punctuation),
    row("semicolon", ";", KeyKind::Key, Group::Punctuation),
    row("apostrophe", "'", KeyKind::Key, Group::Punctuation),
    row("grave", "`", KeyKind::Key, Group::Punctuation),
    row("comma", ",", KeyKind::Key, Group::Punctuation),
    row("period", ".", KeyKind::Key, Group::Punctuation),
    row("slash", "/", KeyKind::Key, Group::Punctuation),
    row("backslash", "\\", KeyKind::Key, Group::Punctuation),
    row(
        "XF86AudioRaiseVolume",
        "Volume up key",
        KeyKind::Key,
        Group::Media,
    ),
    row(
        "XF86AudioLowerVolume",
        "Volume down key",
        KeyKind::Key,
        Group::Media,
    ),
    row("XF86AudioMute", "Mute key", KeyKind::Key, Group::Media),
    row(
        "XF86AudioMicMute",
        "Microphone mute key",
        KeyKind::Key,
        Group::Media,
    ),
    row(
        "XF86MonBrightnessUp",
        "Brightness up key",
        KeyKind::Key,
        Group::Media,
    ),
    row(
        "XF86MonBrightnessDown",
        "Brightness down key",
        KeyKind::Key,
        Group::Media,
    ),
    row(
        "XF86AudioNext",
        "Next track key",
        KeyKind::Key,
        Group::Media,
    ),
    row(
        "XF86AudioPrev",
        "Previous track key",
        KeyKind::Key,
        Group::Media,
    ),
    row("XF86AudioPlay", "Play key", KeyKind::Key, Group::Media),
    row("XF86AudioPause", "Pause key", KeyKind::Key, Group::Media),
    row("XF86AudioStop", "Stop key", KeyKind::Key, Group::Media),
    row(
        "XF86Calculator",
        "Calculator key",
        KeyKind::Key,
        Group::Media,
    ),
    row("XF86Mail", "Mail key", KeyKind::Key, Group::Media),
    row("XF86Search", "Search key", KeyKind::Key, Group::Media),
    row("XF86Explorer", "Files key", KeyKind::Key, Group::Media),
    row("XF86HomePage", "Home page key", KeyKind::Key, Group::Media),
    row("XF86Tools", "Tools key", KeyKind::Key, Group::Media),
    row("mouse_down", "Scroll down", KeyKind::Wheel, Group::Mouse),
    row("mouse_up", "Scroll up", KeyKind::Wheel, Group::Mouse),
    row("mouse_left", "Scroll left", KeyKind::Wheel, Group::Mouse),
    row("mouse_right", "Scroll right", KeyKind::Wheel, Group::Mouse),
    row(
        "mouse:272",
        "Left mouse button",
        KeyKind::Button,
        Group::Mouse,
    ),
    row(
        "mouse:273",
        "Right mouse button",
        KeyKind::Button,
        Group::Mouse,
    ),
    row(
        "mouse:274",
        "Middle mouse button",
        KeyKind::Button,
        Group::Mouse,
    ),
    row(
        "mouse:275",
        "Side mouse button",
        KeyKind::Button,
        Group::Mouse,
    ),
    row(
        "mouse:276",
        "Extra mouse button",
        KeyKind::Button,
        Group::Mouse,
    ),
    row("SUPER_L", "Super key", KeyKind::ModKey, Group::Navigation),
    row(
        "SUPER_R",
        "Right Super key",
        KeyKind::ModKey,
        Group::Navigation,
    ),
];

/// The key named `name` (xkb matches case-insensitively), with its index in [`KEYS`].
/// `Page_Up` and `Page_Down` are accepted for `Prior` and `Next`. `Enter` is not a key: Hyprland
/// refuses it too ("did you mean Return").
pub fn find(name: &str) -> Option<(usize, &'static KeyRow)> {
    let name = match name.to_ascii_lowercase().as_str() {
        "page_up" => "prior".to_string(),
        "page_down" => "next".to_string(),
        other => other.to_string(),
    };
    KEYS.iter()
        .enumerate()
        .find(|(_, k)| k.spelling.eq_ignore_ascii_case(&name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn spellings_unique_case_insensitively() {
        let mut seen = BTreeSet::new();
        for k in KEYS {
            assert!(
                seen.insert(k.spelling.to_ascii_lowercase()),
                "{}",
                k.spelling
            );
        }
    }

    #[test]
    fn labels_are_plain() {
        for k in KEYS {
            assert!(!k.label.is_empty());
            for bad in ["XF86", ":", "_", "hl."] {
                assert!(!k.label.contains(bad), "{} -> {}", k.spelling, k.label);
            }
            // "Left mouse button" is a plain label; a raw `mouse_down` or `mouse:272` is not.
            assert!(!k.label.contains("mouse_") && !k.label.contains("mouse:"));
            if matches!(k.kind, KeyKind::Wheel | KeyKind::Button) {
                assert_ne!(k.label, k.spelling);
            }
        }
    }

    #[test]
    fn find_is_case_insensitive() {
        assert_eq!(find("q").unwrap().0, find("Q").unwrap().0);
        assert_eq!(find("RETURN").unwrap().1.spelling, "Return");
        assert_eq!(find("xf86audiomute").unwrap().1.spelling, "XF86AudioMute");
        assert!(find("Foo").is_none());
    }

    #[test]
    fn enter_is_refused_page_up_is_alias() {
        assert!(find("Enter").is_none());
        assert_eq!(find("Page_Up").unwrap().1.spelling, "Prior");
        assert_eq!(find("page_down").unwrap().1.spelling, "Next");
    }

    #[test]
    fn counts() {
        assert_eq!(KEYS.len(), 26 + 10 + 24 + 16 + 11 + 17 + 4 + 5 + 2);
    }

    /// Every plain keysym is a real X11 keysym; XF86 rows are `XF86XK_`. Skipped without the
    /// headers: CI's verify fixture is the authority.
    #[test]
    fn keysyms_exist_in_x11_headers() {
        let read = |p: &str| {
            std::fs::read_to_string(p)
                .ok()
                .map(|t| t.to_ascii_lowercase())
        };
        let (Some(plain), Some(xf86)) = (
            read("/usr/include/X11/keysymdef.h"),
            read("/usr/include/X11/XF86keysym.h"),
        ) else {
            eprintln!("skipped: X11 keysym headers are missing");
            return;
        };
        for k in KEYS {
            if !matches!(k.kind, KeyKind::Key | KeyKind::ModKey) {
                continue;
            }
            let lower = k.spelling.to_ascii_lowercase();
            let needle = match lower.strip_prefix("xf86") {
                Some(rest) => (&xf86, format!("xf86xk_{rest}")),
                None => (&plain, format!("xk_{lower}")),
            };
            let found = needle
                .0
                .lines()
                .any(|l| l.split_whitespace().nth(1).is_some_and(|w| w == needle.1));
            assert!(found, "{} is not a keysym", k.spelling);
        }
    }
}
