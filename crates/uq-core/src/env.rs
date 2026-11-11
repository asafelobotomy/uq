//! Environment variables for the session.
//!
//! The domain model (`Env`), its strict write-time validation, its lenient load-time repair,
//! the `environment.d` text and the History summary. Every refusal is a plain sentence.

use crate::settings::{SettingError, check_known, invalid};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const MAX_VARS: usize = 100;
pub const MAX_VALUE_BYTES: usize = 4096;
const MAX_NAME_CHARS: usize = 128;
const MAX_NAMED: usize = 3;
const MAX_SHOWN_NAME: usize = 40;

const SESSION_REASON: &str = "uQ needs this one to run your session, so it can't be changed here";
const APPEARANCE_REASON: &str = "This is set by the Appearance page.";

/// The first line of the `environment.d` file uQ writes.
pub const ENVIRONMENT_D_HEADER: &str =
    "# Written by uQ (Startup page). Changes here are replaced; use uQ Settings.";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Var {
    pub name: String,
    pub value: String,
}

/// The variables, in list order.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Env {
    pub vars: Vec<Var>,
}

/// A variable that is allowed but can stop apps from starting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warn {
    BreaksApps,
}

/// Variables load one by one: an entry that doesn't parse or break a rule (a hand edit, or a
/// newer uQ's) is dropped with a log line and the others are kept; a list that isn't a list is
/// empty.
impl<'de> Deserialize<'de> for Env {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Env, D::Error> {
        let v = Value::deserialize(d)?;
        let vars = match v.get("vars") {
            None => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .enumerate()
                .filter_map(|(i, e)| match serde_json::from_value::<Var>(e.clone()) {
                    Ok(var) => Some(var),
                    Err(_) => {
                        tracing::warn!(
                            "state: dropping env.vars[{i}]: it doesn't fit this version"
                        );
                        None
                    }
                })
                .collect(),
            Some(_) => {
                tracing::warn!("state: env.vars is not a list; ignoring it");
                Vec::new()
            }
        };
        Ok(Env { vars }.repaired())
    }
}

/// The reason a name is not ours to set, in plain words.
pub fn blocked(name: &str) -> Option<&'static str> {
    const EXACT: [&str; 14] = [
        "WAYLAND_DISPLAY",
        "DISPLAY",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_CURRENT_DESKTOP",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "XDG_CACHE_HOME",
        "XDG_SEAT",
        "XDG_VTNR",
        "HOME",
        "USER",
        "LOGNAME",
    ];
    const PREFIXES: [&str; 3] = ["XDG_SESSION_", "HYPRLAND_", "UQ_"];
    const APPEARANCE: [&str; 4] = [
        "XCURSOR_THEME",
        "XCURSOR_SIZE",
        "HYPRCURSOR_SIZE",
        "QT_QPA_PLATFORMTHEME",
    ];
    if EXACT.contains(&name) || PREFIXES.iter().any(|p| name.starts_with(p)) {
        Some(SESSION_REASON)
    } else if APPEARANCE.contains(&name) {
        Some(APPEARANCE_REASON)
    } else {
        None
    }
}

fn warned(name: &str) -> bool {
    [
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "LD_AUDIT",
        "PATH",
        "XDG_DATA_DIRS",
        "XDG_CONFIG_DIRS",
    ]
    .contains(&name)
}

/// The name rule and the blocklist. `key` names the field in the message.
pub fn check_name(key: &str, name: &str) -> Result<Option<Warn>, SettingError> {
    let ok = !name.is_empty()
        && name.len() <= MAX_NAME_CHARS
        && name
            .bytes()
            .enumerate()
            .all(|(i, b)| b.is_ascii_uppercase() || b == b'_' || (i > 0 && b.is_ascii_digit()));
    if !ok {
        return Err(invalid(
            key,
            "a name uses capital letters, digits and _, can't start with a digit, \
             and has at most 128 characters",
        ));
    }
    if let Some(reason) = blocked(name) {
        return Err(invalid(key, reason));
    }
    Ok(warned(name).then_some(Warn::BreaksApps))
}

fn check_value(key: &str, value: &str) -> Result<(), SettingError> {
    if value.len() > MAX_VALUE_BYTES {
        return Err(invalid(key, "a value has at most 4096 bytes"));
    }
    if value.contains('$') {
        return Err(invalid(key, "a value can't contain $"));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid(
            key,
            "a value can't contain line breaks, tabs or other control characters",
        ));
    }
    Ok(())
}

/// A whole variable: its name, the blocklist and its value. A value is only data, so quotes
/// and brackets are fine; `$` is the one character refused.
pub fn check_var(key: &str, name: &str, value: &str) -> Result<Option<Warn>, SettingError> {
    let warn = check_name(key, name)?;
    check_value(key, value)?;
    Ok(warn)
}

impl Env {
    /// Strict: the whole section is refused for its first bad entry.
    pub fn from_section(v: &Value) -> Result<Env, SettingError> {
        const KEY: &str = "env";
        let Some(obj) = v.as_object() else {
            return Err(invalid(KEY, "expected an object"));
        };
        check_known(KEY, v, &["vars"])?;
        let items = match obj.get("vars") {
            None => &[][..],
            Some(Value::Array(items)) => items.as_slice(),
            Some(_) => return Err(invalid("env.vars", "expected a list")),
        };
        if items.len() > MAX_VARS {
            return Err(invalid("env.vars", "at most 100 variables"));
        }
        let mut seen = BTreeSet::new();
        let mut vars = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            let key = format!("env.vars[{i}]");
            let var: Var = serde_json::from_value(item.clone())
                .map_err(|_| invalid(&key, "a variable needs a name and a value, both text"))?;
            check_var(&key, &var.name, &var.value)?;
            if !seen.insert(var.name.clone()) {
                return Err(invalid(&key, format!("{} is already set above", var.name)));
            }
            vars.push(var);
        }
        Ok(Env { vars })
    }

    /// Drop what breaks a rule (first of a repeated name wins) and what is over the limit, with
    /// a log line each. Loading never fails for it.
    pub fn repaired(mut self) -> Env {
        let mut seen = BTreeSet::new();
        let mut i = 0;
        self.vars.retain(|v| {
            let at = i;
            i += 1;
            if let Err(e) = check_var("env.vars", &v.name, &v.value) {
                tracing::warn!("state: dropping env.vars[{at}]: {e}");
                false
            } else if !seen.insert(v.name.clone()) {
                tracing::warn!("state: dropping env.vars[{at}]: its name is already set");
                false
            } else {
                true
            }
        });
        if self.vars.len() > MAX_VARS {
            tracing::warn!("state: env has more than {MAX_VARS} variables; keeping the first");
            self.vars.truncate(MAX_VARS);
        }
        self
    }

    pub fn section(&self) -> Value {
        serde_json::to_value(self).expect("env serialises")
    }

    /// The variables that carry a warning, by name, in list order.
    pub fn warnings(&self) -> Vec<(String, Warn)> {
        self.vars
            .iter()
            .filter(|v| warned(&v.name))
            .map(|v| (v.name.clone(), Warn::BreaksApps))
            .collect()
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars
            .iter()
            .find(|v| v.name == name)
            .map(|v| v.value.as_str())
    }
}

/// The text of `environment.d/60-uq.conf`: the header, then `NAME='value'` per line. A value
/// holding `'` is double-quoted instead, with `\`, `"` and `` ` `` escaped (`$` can't occur,
/// so systemd's expansion never fires).
pub fn environment_d(lines: &[(String, String)]) -> String {
    let mut out = format!("{ENVIRONMENT_D_HEADER}\n");
    for (name, value) in lines {
        out.push_str(name);
        out.push('=');
        if value.contains('\'') {
            out.push('"');
            for c in value.chars() {
                if matches!(c, '\\' | '"' | '`') {
                    out.push('\\');
                }
                out.push(c);
            }
            out.push('"');
        } else {
            out.push('\'');
            out.push_str(value);
            out.push('\'');
        }
        out.push('\n');
    }
    out
}

fn vars_of(v: &Value) -> Vec<(&str, &str)> {
    v.get("vars")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|e| {
            Some((
                e.get("name")?.as_str()?,
                e.get("value").and_then(Value::as_str).unwrap_or(""),
            ))
        })
        .collect()
}

fn shown(name: &str) -> String {
    if name.chars().count() > MAX_SHOWN_NAME {
        let head: String = name.chars().take(MAX_SHOWN_NAME).collect();
        format!("{head}…")
    } else {
        name.to_string()
    }
}

/// "Environment variable NAME added", "… changed" or "… removed": up to three, then
/// ", and N more". Names only: a value can be a secret.
pub fn summary(before: &Value, after: &Value) -> String {
    let (b, a) = (vars_of(before), vars_of(after));
    let mut lines = Vec::new();
    let mut say = |name: &str, what: &str| {
        lines.push(format!("Environment variable {} {what}", shown(name)));
    };
    for (name, value) in &a {
        match b.iter().find(|(n, _)| n == name) {
            None => say(name, "added"),
            Some((_, old)) if old != value => say(name, "changed"),
            Some(_) => {}
        }
    }
    for (name, _) in &b {
        if !a.iter().any(|(n, _)| n == name) {
            say(name, "removed");
        }
    }
    if lines.is_empty() {
        return if b == a {
            "Environment variables".to_string()
        } else {
            "Environment variables reordered".to_string()
        };
    }
    let mut line = lines[..lines.len().min(MAX_NAMED)].join(", ");
    if lines.len() > MAX_NAMED {
        line.push_str(&format!(", and {} more", lines.len() - MAX_NAMED));
    }
    line
}

#[cfg(test)]
mod tests;
