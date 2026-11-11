//! Profiles and startup apps, environment variables and app fixes (D1, spec §10).
//!
//! A profile holds only values the Startup and Apps and scaling pages accept themselves:
//! - `[env]`: `NAME = "value"` sets a variable (replaced in place, or added at the end), and
//!   `NAME = false` removes it while the profile is on;
//! - `[compat]`: the five switches, and `[compat.apps."<id>"]` with `platform` and `scale`;
//! - `[autostart."<id>"]`: `enabled` and `delay` of an entry that exists on this computer.
//!
//! It never adds a startup entry, a command, launch options, a fix's own variables or a
//! launcher file. Every write goes through the section's own `from_section`, so every limit
//! holds.

mod text;

pub(super) use text::{fallback_label, startup_value_text};
pub use text::{
    mark_startup, missing_startup, missing_startup_notice, startup_keys, startup_keys_of,
    startup_label, startup_notice,
};

use super::{ProfileError, invalid, plain};
use crate::autostart::{FileChoice, MAX_DELAY, valid_added_id};
use crate::compat::hypr::SwitchKey;
use crate::compat::{AppFix, Platform, valid_scale};
use crate::desktop_entry::valid_desktop_id;
use crate::env::{self, MAX_VARS, Var};
use crate::settings::SettingError;
use crate::state::State;
use serde_json::{Value, json};

/// Why an `[env]` leaf that isn't text or `false` is refused.
pub(super) const ENV_ONLY: &str =
    "a profile can only set a variable to a value, or remove it with false";
/// Why a fix's other fields are refused.
pub(super) const COMPAT_ONLY: &str = "a profile can only set an app's Run on and Size";
/// Why an autostart entry's other fields are refused.
pub(super) const AUTOSTART_ONLY: &str =
    "a profile can only turn a startup app on or off or set its delay";
/// An apply that would pass the variables' limit.
pub const TOO_MANY_VARS: &str = "This profile would make more than 100 environment variables.";

const UNKNOWN: &str = "not a setting uQ knows";
const APP_ID: &str = "this isn't the name of an app's file";
const STARTUP_ID: &str = "this isn't the name of a startup app's file, or of one you added";
const ON_OFF: &str = "expected on or off (true or false)";
const PLATFORM: &str = "run on is Automatic, Wayland or X11 (auto, wayland or x11)";
const SCALE: &str = "a size is between 1 and 3, in steps of 0.25";
const DELAY: &str = "a delay is 0 to 600 seconds";
const NO_ENTRY: &str = "isn't a startup app on this computer";

fn refuse(key: &str, reason: &str) -> SettingError {
    SettingError::Invalid {
        key: key.to_string(),
        reason: reason.to_string(),
    }
}

fn to_profile(e: SettingError) -> ProfileError {
    match e {
        SettingError::Invalid { key, reason } => invalid(key, reason),
        SettingError::UnknownKey(key) => invalid(key, UNKNOWN),
    }
}

/// A fix's id: a desktop file name, with its `.desktop`.
fn fix_id_ok(id: &str) -> bool {
    id.ends_with(".desktop") && valid_desktop_id(id)
}

/// A startup entry's id: a file id (with `.desktop`) or an added entry's `a-…`.
pub(super) fn startup_id_ok(id: &str) -> bool {
    fix_id_ok(id) || valid_added_id(id)
}

/// An `[env]` leaf: `Some(value)` sets it, `None` (from `false`) removes it.
fn env_value<'a>(key: &str, name: &str, v: &'a Value) -> Result<Option<&'a str>, SettingError> {
    env::check_name(key, name)?;
    match v {
        Value::String(s) => {
            env::check_var(key, name, s)?;
            Ok(Some(s))
        }
        Value::Bool(false) => Ok(None),
        _ => Err(refuse(key, ENV_ONLY)),
    }
}

fn bool_of(key: &str, v: &Value) -> Result<bool, SettingError> {
    v.as_bool().ok_or_else(|| refuse(key, ON_OFF))
}

fn platform_of(key: &str, v: &Value) -> Result<Platform, SettingError> {
    match v.as_str() {
        Some("auto") => Ok(Platform::Auto),
        Some("wayland") => Ok(Platform::Wayland),
        Some("x11") => Ok(Platform::X11),
        _ => Err(refuse(key, PLATFORM)),
    }
}

fn scale_of(key: &str, v: &Value) -> Result<f64, SettingError> {
    v.as_f64()
        .filter(|s| valid_scale(*s))
        .ok_or_else(|| refuse(key, SCALE))
}

fn delay_of(key: &str, v: &Value) -> Result<u32, SettingError> {
    v.as_u64()
        .filter(|d| *d <= u64::from(MAX_DELAY))
        .map(|d| d as u32)
        .ok_or_else(|| refuse(key, DELAY))
}

fn table<'a>(
    tree: &'a Value,
    key: &str,
    reason: &str,
) -> Result<&'a serde_json::Map<String, Value>, ProfileError> {
    tree.as_object().ok_or_else(|| invalid(key, reason))
}

/// A profile's `[env]`: each entry a variable name (the name rule and the blocklist) set to
/// text the Startup page would accept, or `false`.
pub fn check_env_tree(tree: &Value) -> Result<(), ProfileError> {
    for (name, v) in table(tree, "env", ENV_ONLY)? {
        env_value(&format!("env.{}", plain(name)), name, v).map_err(to_profile)?;
    }
    Ok(())
}

/// A profile's `[compat]`: the five switches, and each fix's `platform` and `scale`.
pub fn check_compat_tree(tree: &Value) -> Result<(), ProfileError> {
    for (k, v) in table(tree, "compat", COMPAT_ONLY)? {
        let key = format!("compat.{}", plain(k));
        if SwitchKey::from_key(k).is_some() {
            bool_of(&key, v).map_err(to_profile)?;
            continue;
        }
        if k != "apps" {
            return Err(invalid(key, UNKNOWN));
        }
        for (id, fix) in table(v, &key, COMPAT_ONLY)? {
            let at = format!("compat.apps.{}", plain(id));
            if !fix_id_ok(id) {
                return Err(invalid(at, APP_ID));
            }
            for (f, fv) in table(fix, &at, COMPAT_ONLY)? {
                let key = format!("{at}.{}", plain(f));
                match f.as_str() {
                    "platform" => platform_of(&key, fv).map(drop),
                    "scale" => scale_of(&key, fv).map(drop),
                    _ => Err(refuse(&key, COMPAT_ONLY)),
                }
                .map_err(to_profile)?;
            }
        }
    }
    Ok(())
}

/// A profile's `[autostart]`: each entry an id with `enabled` and `delay` only.
pub fn check_autostart_tree(tree: &Value) -> Result<(), ProfileError> {
    for (id, entry) in table(tree, "autostart", AUTOSTART_ONLY)? {
        let at = format!("autostart.{}", plain(id));
        if !startup_id_ok(id) {
            return Err(invalid(at, STARTUP_ID));
        }
        for (f, v) in table(entry, &at, AUTOSTART_ONLY)? {
            let key = format!("{at}.{}", plain(f));
            match f.as_str() {
                "enabled" => bool_of(&key, v).map(drop),
                "delay" => delay_of(&key, v).map(drop),
                _ => Err(refuse(&key, AUTOSTART_ONLY)),
            }
            .map_err(to_profile)?;
        }
    }
    Ok(())
}

/// One leaf as the state holds it. A variable that isn't set reads `false` (what a profile
/// writes to remove it); a fix's platform reads `auto` without the fix, its scale `None` without
/// a size;
/// an entry's are its own (added) or the choice's (found), `None` when unset.
pub fn read_leaf(state: &State, domain: &str, path: &[String]) -> Option<Value> {
    match (domain, path) {
        ("env", [name]) => Some(
            state
                .env
                .vars
                .iter()
                .find(|v| v.name == *name)
                .map_or(Value::Bool(false), |v| Value::String(v.value.clone())),
        ),
        ("compat", [k]) => SwitchKey::from_key(k).map(|s| Value::Bool(state.compat.switch(s))),
        ("compat", [apps, id, f]) if apps == "apps" => {
            let fix = state.compat.apps.iter().find(|x| x.id == *id);
            match f.as_str() {
                // An app without a fix runs automatically: what removing the fix writes back.
                "platform" => Some(json!(fix.map_or("auto", |x| x.platform.as_str()))),
                "scale" => fix?.scale.map(Value::from),
                _ => None,
            }
        }
        ("autostart", [id, f]) => {
            let a = &state.autostart;
            let (enabled, delay) = match a.added.iter().find(|e| e.id() == id) {
                Some(e) => (Some(e.enabled()), e.delay()),
                None => {
                    let c = a.choice(id)?;
                    (c.enabled, c.delay)
                }
            };
            match f.as_str() {
                "enabled" => enabled.map(Value::Bool),
                "delay" => delay.map(Value::from),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Write leaves of `env`, `compat` or `autostart` with one validated `set_section` (`None`
/// removes: a variable, a fix's platform back to automatic, its size, or a choice's field). A
/// fix or a choice left with nothing set is removed. An added id with no entry is refused
/// (callers leave those out). On an error `state` is unchanged.
pub fn write_leaves(
    state: &mut State,
    domain: &str,
    leaves: &[(Vec<String>, Option<Value>)],
) -> Result<(), SettingError> {
    match domain {
        "env" => write_env(state, leaves),
        "compat" => write_compat(state, leaves),
        "autostart" => write_autostart(state, leaves),
        _ => Err(SettingError::UnknownKey(domain.to_string())),
    }
}

/// The variables after `leaves`, in order: a set one keeps its place, a new one goes last.
fn write_env(
    state: &mut State,
    leaves: &[(Vec<String>, Option<Value>)],
) -> Result<(), SettingError> {
    let mut vars = state.env.vars.clone();
    for (path, value) in leaves {
        let [name] = path.as_slice() else {
            return Err(refuse(&format!("env.{}", plain(&path.join("."))), ENV_ONLY));
        };
        let key = format!("env.{}", plain(name));
        let set = match value {
            None => None,
            Some(v) => env_value(&key, name, v)?,
        };
        match (vars.iter().position(|v| v.name == *name), set) {
            (Some(i), Some(s)) => vars[i].value = s.to_string(),
            (None, Some(s)) => vars.push(Var {
                name: name.clone(),
                value: s.to_string(),
            }),
            (Some(i), None) => {
                vars.remove(i);
            }
            (None, None) => {}
        }
    }
    if vars.len() > MAX_VARS {
        return Err(refuse("env", TOO_MANY_VARS));
    }
    state.set_section("env", json!({ "vars": vars }))
}

fn write_compat(
    state: &mut State,
    leaves: &[(Vec<String>, Option<Value>)],
) -> Result<(), SettingError> {
    let mut c = state.compat.clone();
    let mut touched: Vec<&str> = Vec::new();
    for (path, value) in leaves {
        let key = format!("compat.{}", plain(&path.join(".")));
        match path.as_slice() {
            [k] => {
                let sw = SwitchKey::from_key(k).ok_or_else(|| refuse(&key, UNKNOWN))?;
                let on = match value {
                    None => false,
                    Some(v) => bool_of(&key, v)?,
                };
                c.set_switch(sw, on);
            }
            [apps, id, f] if apps == "apps" => {
                if !fix_id_ok(id) {
                    return Err(refuse(&key, APP_ID));
                }
                let i = match c.apps.iter().position(|x| x.id == *id) {
                    Some(i) => i,
                    None => {
                        c.apps.push(AppFix::new(id));
                        c.apps.len() - 1
                    }
                };
                let fix = &mut c.apps[i];
                match (f.as_str(), value) {
                    ("platform", None) => fix.platform = Platform::Auto,
                    ("platform", Some(v)) => fix.platform = platform_of(&key, v)?,
                    ("scale", None) => fix.scale = None,
                    ("scale", Some(v)) => fix.scale = Some(scale_of(&key, v)?),
                    _ => return Err(refuse(&key, COMPAT_ONLY)),
                }
                touched.push(id);
            }
            _ => return Err(refuse(&key, COMPAT_ONLY)),
        }
    }
    c.apps
        .retain(|f| !(f.is_empty() && touched.contains(&f.id.as_str())));
    state.set_section("compat", c.section())
}

fn write_autostart(
    state: &mut State,
    leaves: &[(Vec<String>, Option<Value>)],
) -> Result<(), SettingError> {
    let mut a = state.autostart.clone();
    for (path, value) in leaves {
        let key = format!("autostart.{}", plain(&path.join(".")));
        let [id, f] = path.as_slice() else {
            return Err(refuse(&key, AUTOSTART_ONLY));
        };
        if !startup_id_ok(id) {
            return Err(refuse(&key, STARTUP_ID));
        }
        let enabled = |v: &Option<Value>| v.as_ref().map(|v| bool_of(&key, v)).transpose();
        let delay = |v: &Option<Value>| v.as_ref().map(|v| delay_of(&key, v)).transpose();
        if let Some(e) = a.added.iter_mut().find(|e| e.id() == id) {
            match f.as_str() {
                "enabled" => e.set_enabled(enabled(value)?.unwrap_or(true)),
                "delay" => e.set_delay(delay(value)?),
                _ => return Err(refuse(&key, AUTOSTART_ONLY)),
            }
            continue;
        }
        if !fix_id_ok(id) {
            return Err(refuse(&key, NO_ENTRY));
        }
        let i = match a.files.iter().position(|c| c.id == *id) {
            Some(i) => i,
            None => {
                a.files.push(FileChoice {
                    id: id.clone(),
                    enabled: None,
                    delay: None,
                });
                a.files.len() - 1
            }
        };
        match f.as_str() {
            "enabled" => a.files[i].enabled = enabled(value)?,
            "delay" => a.files[i].delay = delay(value)?,
            _ => return Err(refuse(&key, AUTOSTART_ONLY)),
        }
    }
    a.prune_empty_choices();
    state.set_section("autostart", a.section())
}

/// The fixes the profile would empty (and so remove) that also apply in other launchers: their
/// keys are left out, since putting the fix back can't turn "elsewhere" on again.
pub fn emptied_elsewhere(current: &State, profile: &super::Profile) -> Vec<String> {
    let mut out = Vec::new();
    let Some(apps) = profile.values.get("compat").and_then(|c| c.get("apps")) else {
        return out;
    };
    for (id, fields) in apps.as_object().into_iter().flatten() {
        let Some(fix) = current.compat.apps.iter().find(|f| f.id == *id) else {
            continue;
        };
        let mut after = fix.clone();
        if let Some(p) = fields.get("platform") {
            after.platform = platform_of("", p).unwrap_or(after.platform);
        }
        if let Some(s) = fields.get("scale").and_then(Value::as_f64) {
            after.scale = Some(s);
        }
        if fix.elsewhere && after.is_empty() {
            out.push(id.clone());
        }
    }
    out
}

/// The apply notice for `n` fixes left alone by [`emptied_elsewhere`].
pub fn kept_fix_notice(n: usize) -> String {
    if n == 1 {
        "This profile would remove an app fix that also works in other launchers, so that part \
         was left out."
            .into()
    } else {
        format!(
            "This profile would remove {n} app fixes that also work in other launchers, so those \
             parts were left out."
        )
    }
}

#[cfg(test)]
mod tests;
