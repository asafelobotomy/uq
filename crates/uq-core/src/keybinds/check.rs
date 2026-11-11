//! Value checks for the keybinds domain: one function per entry kind, used by the strict write
//! and by the lenient load repair.

use super::combo::{self, Combo};
use super::defaults::{self, LOCK_RECOVERY};
use super::keys::KeyKind;
use super::{CustomBind, MAX_NAME_CHARS, Override};
use crate::settings::{SettingError, invalid};

const LOCK_REFUSED: &str = "Shortcuts that open things can't work over the lock screen.";
const REPEAT_REFUSED: &str = "This would start it over and over while the key is held";
const BARE_REFUSED: &str = "Add Super, Ctrl or Alt, so typing doesn't trigger it.";
pub(crate) const RECOVERY_REFUSED: &str =
    "This shortcut gets you back in if the lock screen stops working, so it stays on.";

/// Parses `text` for a shortcut: plain refusals, and the bare-key rule (R9).
fn parse(key: &str, text: &str) -> Result<Combo, SettingError> {
    let c = combo::parse(text).map_err(|e| invalid(key, e.to_string()))?;
    if combo::needs_modifier(c) {
        return Err(invalid(key, BARE_REFUSED));
    }
    Ok(c)
}

/// An override entry, normalised: the combo canonical, and values equal to the default dropped.
/// `Ok(None)` is an override that changes nothing.
pub fn override_(i: usize, o: &Override) -> Result<Option<Override>, SettingError> {
    let at = |f: &str| format!("keybinds.overrides[{i}].{f}");
    let Some(d) = defaults::default_of(&o.id) else {
        return Err(invalid(&at("id"), "this isn't one of uQ's shortcuts"));
    };
    if o.enabled == Some(false) && d.id == LOCK_RECOVERY {
        return Err(invalid(&at("enabled"), RECOVERY_REFUSED));
    }
    let mut combo = None;
    if let Some(text) = &o.combo {
        let c = parse(&at("combo"), text)?;
        let own = d.combo.and_then(|t| combo::parse(t).ok());
        let own_kind = own.map(combo::kind).unwrap_or(KeyKind::Key);
        let k = combo::kind(c);
        // The app menu is a release bind on a lone Super key; it may also take a plain key.
        let fits = k == own_kind || (d.id == "uq.appmenu" && k == KeyKind::Key);
        if !fits {
            return Err(invalid(
                &at("combo"),
                match own_kind {
                    KeyKind::Wheel => "This one needs a scroll step, such as Super + Scroll down.",
                    KeyKind::Button => {
                        "This one needs a mouse button, such as Super + Left mouse button."
                    }
                    _ => "This one needs a key, not a mouse button or scroll step.",
                },
            ));
        }
        if own != Some(c) {
            combo = Some(combo::canonical(c));
        }
    }
    let enabled = (o.enabled == Some(false)).then_some(false);
    if combo.is_none() && enabled.is_none() {
        return Ok(None);
    }
    Ok(Some(Override {
        id: o.id.clone(),
        combo,
        enabled,
    }))
}

/// A custom shortcut, normalised in place (name trimmed, combo canonical, launch id
/// normalised). The id is left to the caller.
pub fn custom(i: usize, b: &mut CustomBind) -> Result<(), SettingError> {
    let at = |f: &str| format!("keybinds.custom[{i}].{f}");
    let name = b.name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(invalid(&at("name"), "a name is 1 to 100 characters"));
    }
    if name.chars().any(char::is_control) {
        return Err(invalid(&at("name"), "a name is plain text"));
    }
    b.name = name.to_string();
    let c = parse(&at("combo"), &b.combo)?;
    if !matches!(combo::kind(c), KeyKind::Key | KeyKind::Wheel) {
        return Err(invalid(
            &at("combo"),
            "Your own shortcuts use a key or a scroll step, not a mouse button.",
        ));
    }
    b.combo = combo::canonical(c);
    b.action = b.action.check_at(&at("action"))?;
    if b.locked && !b.action.may_lock() {
        return Err(invalid(&at("locked"), LOCK_REFUSED));
    }
    if b.repeating && !b.action.may_repeat() {
        return Err(invalid(&at("repeating"), REPEAT_REFUSED));
    }
    Ok(())
}
