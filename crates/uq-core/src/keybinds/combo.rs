//! Shortcut combos: parse, normalise and show.
//!
//! A combo is a set of modifiers and one key from [`keys::KEYS`]. `Combo` equality is the
//! conflict key (R6): two spellings of one shortcut parse to the same value.

use super::hypr::{MODIFIERS, REFUSED_MODIFIERS};
use super::keys::{self, Group, KEYS, KeyKind};
use crate::settings::clean_text;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Combo {
    /// Hyprland's own modifier bits ([`MODIFIERS`]).
    pub mods: u8,
    /// Index into [`KEYS`].
    pub key: u16,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ComboError {
    #[error("Pick a key as well as Super, Ctrl, Alt or Shift.")]
    NoKey,
    #[error("uQ doesn't know the key “{0}”.")]
    UnknownKey(String),
    #[error("Use Return for the Enter key.")]
    UseReturn,
    #[error("Put Super, Ctrl, Alt and Shift before the key.")]
    ModifiersFirst,
    #[error("Caps Lock and other lock keys can't be part of a shortcut.")]
    LockKey,
    #[error("A shortcut has one key.")]
    OneKey,
}

fn modifier(part: &str) -> Option<u8> {
    MODIFIERS
        .iter()
        .find(|(name, _, aliases)| {
            name.eq_ignore_ascii_case(part) || aliases.iter().any(|a| a.eq_ignore_ascii_case(part))
        })
        .map(|(_, bit, _)| *bit)
}

fn refused(part: &str) -> bool {
    REFUSED_MODIFIERS
        .iter()
        .any(|r| r.eq_ignore_ascii_case(part))
}

/// Parses `SUPER + SHIFT + Q` and its aliases (`WIN`, `META`, `CONTROL`, case-insensitive).
pub fn parse(text: &str) -> Result<Combo, ComboError> {
    let mut mods = 0u8;
    let mut key: Option<usize> = None;
    for part in text.split('+').map(str::trim) {
        if part.is_empty() {
            return Err(ComboError::NoKey);
        }
        if refused(part) {
            return Err(ComboError::LockKey);
        }
        if let Some(bit) = modifier(part) {
            if key.is_some() {
                return Err(ComboError::ModifiersFirst);
            }
            mods |= bit;
            continue;
        }
        if key.is_some() {
            return Err(ComboError::OneKey);
        }
        if part.eq_ignore_ascii_case("enter") {
            return Err(ComboError::UseReturn);
        }
        match keys::find(part) {
            Some((i, _)) => key = Some(i),
            None => return Err(ComboError::UnknownKey(clean_text(part, 32))),
        }
    }
    let key = key.ok_or(ComboError::NoKey)?;
    Ok(Combo {
        mods,
        key: key as u16,
    })
}

fn row(c: Combo) -> &'static keys::KeyRow {
    &KEYS[c.key as usize]
}

/// `SUPER + SHIFT + Q`: modifiers in the order SUPER, SHIFT, CTRL, ALT, then the key's spelling.
/// This is the form stored and rendered.
pub fn canonical(c: Combo) -> String {
    let mut parts: Vec<&str> = MODIFIERS
        .iter()
        .filter(|(_, bit, _)| c.mods & bit != 0)
        .map(|(name, _, _)| *name)
        .collect();
    parts.push(row(c).spelling);
    parts.join(" + ")
}

/// Plain words for people: `Super + Shift + Q`, `Super + Left mouse button`, `Volume up key`.
pub fn text(c: Combo) -> String {
    let mut parts: Vec<&str> = MODIFIERS
        .iter()
        .filter(|(_, bit, _)| c.mods & bit != 0)
        .map(|(name, _, _)| match *name {
            "SUPER" => "Super",
            "SHIFT" => "Shift",
            "CTRL" => "Ctrl",
            _ => "Alt",
        })
        .collect();
    parts.push(row(c).label);
    parts.join(" + ")
}

/// R9: true when the combo has no Super, Ctrl or Alt and its key isn't one that may stand alone
/// (the media keys, Print, F13 to F24). Typing a bare letter would trigger it.
pub fn needs_modifier(c: Combo) -> bool {
    let real: u8 = MODIFIERS
        .iter()
        .filter(|m| m.0 != "SHIFT")
        .fold(0, |a, m| a | m.1);
    if c.mods & real != 0 {
        return false;
    }
    let k = row(c);
    let high_f = k
        .spelling
        .strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (13..=24).contains(&n));
    !(k.group == Group::Media || k.spelling == "Print" || high_f)
}

pub fn kind(c: Combo) -> KeyKind {
    row(c).kind
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE_LUA: [&str; 55] = [
        "SUPER + Return",
        "SUPER + Q",
        "SUPER + V",
        "SUPER + SHIFT + M",
        "SUPER + Space",
        "SUPER + period",
        "SUPER + SUPER_L",
        "SUPER + A",
        "SUPER + N",
        "SUPER + L",
        "SUPER + Escape",
        "SUPER + ALT + D",
        "SUPER + ALT + M",
        "SUPER + ALT + S",
        "SUPER + left",
        "SUPER + right",
        "SUPER + up",
        "SUPER + down",
        "SUPER + 1",
        "SUPER + SHIFT + 1",
        "SUPER + 2",
        "SUPER + SHIFT + 2",
        "SUPER + 3",
        "SUPER + SHIFT + 3",
        "SUPER + 4",
        "SUPER + SHIFT + 4",
        "SUPER + 5",
        "SUPER + SHIFT + 5",
        "SUPER + 6",
        "SUPER + SHIFT + 6",
        "SUPER + 7",
        "SUPER + SHIFT + 7",
        "SUPER + 8",
        "SUPER + SHIFT + 8",
        "SUPER + 9",
        "SUPER + SHIFT + 9",
        "SUPER + 0",
        "SUPER + SHIFT + 0",
        "SUPER + S",
        "SUPER + SHIFT + S",
        "SUPER + mouse_down",
        "SUPER + mouse_up",
        "SUPER + mouse:272",
        "SUPER + mouse:273",
        "SUPER + CTRL + L",
        "XF86AudioRaiseVolume",
        "XF86AudioLowerVolume",
        "XF86AudioMute",
        "XF86AudioMicMute",
        "XF86MonBrightnessUp",
        "XF86MonBrightnessDown",
        "XF86AudioNext",
        "XF86AudioPause",
        "XF86AudioPlay",
        "XF86AudioPrev",
    ];

    #[test]
    fn round_trips_every_base_lua_string() {
        for s in BASE_LUA {
            let c = parse(s).unwrap_or_else(|e| panic!("{s}: {e}"));
            assert_eq!(canonical(c), s);
        }
    }

    #[test]
    fn aliases_normalise() {
        let want = parse("SUPER + Q").unwrap();
        for s in ["WIN + q", "META+Q", "super + q", "LOGO + Q", "MOD4 + q"] {
            assert_eq!(parse(s).unwrap(), want, "{s}");
        }
        assert_eq!(canonical(want), "SUPER + Q");
        assert_eq!(
            parse("CONTROL + MOD1 + x").unwrap(),
            parse("CTRL + ALT + X").unwrap()
        );
        assert_eq!(parse("SUPER + SUPER + Q").unwrap(), want);
    }

    #[test]
    fn modifier_order_is_canonical() {
        assert_eq!(
            canonical(parse("ALT + SUPER + d").unwrap()),
            "SUPER + ALT + D"
        );
        assert_eq!(
            canonical(parse("ALT + CTRL + SHIFT + SUPER + d").unwrap()),
            "SUPER + SHIFT + CTRL + ALT + D"
        );
    }

    #[test]
    fn refusals_are_plain() {
        let long = "a".repeat(300);
        for s in [
            "SUPER",
            "SUPER + Q + W",
            "Q + SUPER",
            "CAPS + Q",
            "code:24",
            "switch:Lid Switch",
            "catchall",
            "SUPER + Enter",
            "SUPER + Foo",
            "SUPER +",
            "",
            "SUPER + Fo\no",
            long.as_str(),
        ] {
            let e = parse(s).unwrap_err().to_string();
            assert!(!e.contains('\n'), "{s:?}: {e:?}");
            assert!(e.len() < 120, "{s:?}");
        }
        assert!(parse(&long).unwrap_err().to_string().len() < 100);
        assert_eq!(parse("SUPER + Enter"), Err(ComboError::UseReturn));
        assert_eq!(parse("Q + SUPER"), Err(ComboError::ModifiersFirst));
        assert_eq!(parse("SUPER + Q + W"), Err(ComboError::OneKey));
        assert_eq!(parse("CAPS + Q"), Err(ComboError::LockKey));
        assert_eq!(parse("SUPER"), Err(ComboError::NoKey));
    }

    #[test]
    fn text_is_plain() {
        assert_eq!(
            text(parse("SUPER + mouse:272").unwrap()),
            "Super + Left mouse button"
        );
        assert_eq!(
            text(parse("XF86AudioRaiseVolume").unwrap()),
            "Volume up key"
        );
        assert_eq!(
            text(parse("SUPER + SHIFT + Return").unwrap()),
            "Super + Shift + Enter"
        );
        for s in BASE_LUA {
            let t = text(parse(s).unwrap());
            for bad in ["XF86", "mouse:", "mouse_", "hl."] {
                assert!(!t.contains(bad), "{t}");
            }
        }
    }

    #[test]
    fn bare_key_rule() {
        for s in ["Q", "SHIFT + Q", "F5", "SHIFT + F5", "Return"] {
            assert!(needs_modifier(parse(s).unwrap()), "{s}");
        }
        for s in [
            "XF86AudioMute",
            "Print",
            "F13",
            "F24",
            "SUPER + Q",
            "CTRL + F5",
        ] {
            assert!(!needs_modifier(parse(s).unwrap()), "{s}");
        }
    }

    #[test]
    fn kinds() {
        assert_eq!(kind(parse("SUPER + Q").unwrap()), KeyKind::Key);
        assert_eq!(kind(parse("SUPER + mouse_up").unwrap()), KeyKind::Wheel);
        assert_eq!(kind(parse("SUPER + mouse:273").unwrap()), KeyKind::Button);
        assert_eq!(kind(parse("SUPER + SUPER_L").unwrap()), KeyKind::ModKey);
    }
}
