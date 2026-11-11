//! Apps and scaling (spec §6): five global switches and per-app fixes.
//!
//! The domain model (`Compat`), its strict write-time validation, its lenient load-time repair
//! and the History summary. The names it renders are in [`hypr`] and [`browsers`]; the launch
//! command and the launcher-file override are built in [`launch`]. Every refusal is a plain
//! sentence.

pub mod browsers;
pub mod hypr;
pub mod launch;
mod summary;

pub use summary::summary;

use crate::desktop_entry::{normalise_id, valid_desktop_id};
use crate::env::{self, Var};
use crate::settings::{SettingError, check_known, invalid};
use hypr::SwitchKey;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const MAX_APPS: usize = 100;
pub const MAX_APP_ENV: usize = 20;
pub const MAX_ARGS: usize = 20;
pub const MAX_ARG_CHARS: usize = 1024;

const NOTHING: &str = "This fix doesn't change anything.";
const FIX_FIELDS: [&str; 6] = ["id", "platform", "scale", "env", "args", "elsewhere"];

/// Which display system an app is told to use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    #[default]
    Auto,
    Wayland,
    X11,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Auto => "auto",
            Platform::Wayland => "wayland",
            Platform::X11 => "x11",
        }
    }
}

/// One app's fix, keyed by its desktop id (always ending in `.desktop`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppFix {
    pub id: String,
    #[serde(default)]
    pub platform: Platform,
    #[serde(default)]
    pub scale: Option<f64>,
    #[serde(default)]
    pub env: Vec<Var>,
    #[serde(default)]
    pub args: Vec<String>,
    /// "Also when started from other launchers": write a launcher-file override.
    #[serde(default)]
    pub elsewhere: bool,
}

impl AppFix {
    /// A fix for `id` with nothing set.
    pub fn new(id: &str) -> AppFix {
        AppFix {
            id: normalise_id(id),
            platform: Platform::Auto,
            scale: None,
            env: Vec::new(),
            args: Vec::new(),
            elsewhere: false,
        }
    }

    /// Nothing that changes how the app starts is set (`elsewhere` alone changes nothing).
    pub fn is_empty(&self) -> bool {
        self.platform == Platform::Auto
            && self.scale.is_none()
            && self.env.is_empty()
            && self.args.is_empty()
    }
}

/// The Apps and scaling section.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Compat {
    pub sharp_x11: bool,
    pub electron_wayland: bool,
    pub chromium_wayland: bool,
    pub qt_wayland: bool,
    pub steam_scale: bool,
    pub apps: Vec<AppFix>,
}

/// A size is 1 to 3 in steps of a quarter.
pub fn valid_scale(s: f64) -> bool {
    s.is_finite() && (1.0..=3.0).contains(&s) && (s * 4.0).fract() == 0.0
}

/// Every rule of one fix, on a parsed value; `key` is `compat.apps[i]`.
fn check_fix(key: &str, fix: &AppFix) -> Result<(), SettingError> {
    if !valid_desktop_id(&fix.id) {
        return Err(invalid(
            &format!("{key}.id"),
            "an app is named by its file name, such as firefox.desktop",
        ));
    }
    if fix.scale.is_some_and(|s| !valid_scale(s)) {
        return Err(invalid(
            &format!("{key}.scale"),
            "a size is between 1 and 3, in steps of 0.25",
        ));
    }
    if fix.env.len() > MAX_APP_ENV {
        return Err(invalid(
            &format!("{key}.env"),
            "at most 20 extra settings for one app",
        ));
    }
    let mut seen = BTreeSet::new();
    for (j, var) in fix.env.iter().enumerate() {
        let k = format!("{key}.env[{j}]");
        env::check_var(&k, &var.name, &var.value)?;
        if !seen.insert(var.name.as_str()) {
            return Err(invalid(&k, format!("{} is already set above", var.name)));
        }
    }
    if fix.args.len() > MAX_ARGS {
        return Err(invalid(
            &format!("{key}.args"),
            "at most 20 extra options for one app",
        ));
    }
    for (j, arg) in fix.args.iter().enumerate() {
        let n = arg.chars().count();
        if n == 0 || n > MAX_ARG_CHARS {
            return Err(invalid(
                &format!("{key}.args[{j}]"),
                "an option has 1 to 1024 characters",
            ));
        }
        if arg.chars().any(char::is_control) {
            return Err(invalid(
                &format!("{key}.args[{j}]"),
                "an option can't contain line breaks, tabs or other control characters",
            ));
        }
    }
    if fix.is_empty() {
        return Err(invalid(key, NOTHING));
    }
    Ok(())
}

/// One fix from the section JSON, with plain refusals for each field.
fn parse_fix(key: &str, v: &Value) -> Result<AppFix, SettingError> {
    let Some(o) = v.as_object() else {
        return Err(invalid(key, "a fix is a set of fields, not a single value"));
    };
    check_known(key, v, &FIX_FIELDS)?;
    let field = |f: &str| o.get(f).filter(|v| !v.is_null());
    let at = |f: &str| format!("{key}.{f}");
    let id = field("id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(&at("id"), "a fix needs the app it is for"))?;
    let platform = match field("platform") {
        None => Platform::Auto,
        Some(p) => serde_json::from_value(p.clone()).map_err(|_| {
            invalid(
                &at("platform"),
                "run on is automatic, Wayland or X11 (auto, wayland or x11)",
            )
        })?,
    };
    let scale = match field("scale") {
        None => None,
        Some(s) => Some(
            s.as_f64()
                .ok_or_else(|| invalid(&at("scale"), "a size is a number, or empty"))?,
        ),
    };
    let list = |f: &str| match field(f) {
        None => Ok(&[][..]),
        Some(Value::Array(items)) => Ok(items.as_slice()),
        Some(_) => Err(invalid(&at(f), "expected a list")),
    };
    let env = list("env")?
        .iter()
        .enumerate()
        .map(|(j, e)| {
            serde_json::from_value::<Var>(e.clone()).map_err(|_| {
                invalid(
                    &format!("{key}.env[{j}]"),
                    "a variable needs a name and a value, both text",
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let args = list("args")?
        .iter()
        .enumerate()
        .map(|(j, a)| {
            a.as_str()
                .map(String::from)
                .ok_or_else(|| invalid(&format!("{key}.args[{j}]"), "an option is text"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let elsewhere = match field("elsewhere") {
        None => false,
        Some(b) => b
            .as_bool()
            .ok_or_else(|| invalid(&at("elsewhere"), "expected on or off (true or false)"))?,
    };
    let fix = AppFix {
        id: normalise_id(id),
        platform,
        scale,
        env,
        args,
        elsewhere,
    };
    check_fix(key, &fix)?;
    Ok(fix)
}

/// The section loads leniently: a switch that isn't on or off is off, and a fix that doesn't
/// parse or breaks a rule (a hand edit, or a newer uQ's) is dropped with a log line while the
/// others are kept; an `apps` that isn't a list is empty.
impl<'de> Deserialize<'de> for Compat {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Compat, D::Error> {
        let v = Value::deserialize(d)?;
        let mut c = Compat::default();
        for key in SwitchKey::ALL {
            match v.get(key.key()) {
                None => {}
                Some(Value::Bool(b)) => c.set_switch(key, *b),
                Some(_) => tracing::warn!("state: compat.{} is not on or off; off", key.key()),
            }
        }
        c.apps = match v.get("apps") {
            None => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .enumerate()
                .filter_map(|(i, e)| match parse_fix(&format!("compat.apps[{i}]"), e) {
                    Ok(fix) => Some(fix),
                    Err(e) => {
                        tracing::warn!("state: dropping compat.apps[{i}]: {e}");
                        None
                    }
                })
                .collect(),
            Some(_) => {
                tracing::warn!("state: compat.apps is not a list; ignoring it");
                Vec::new()
            }
        };
        Ok(c.repaired())
    }
}

impl Compat {
    /// Strict: the whole section is refused for its first bad entry.
    pub fn from_section(v: &Value) -> Result<Compat, SettingError> {
        const KEY: &str = "compat";
        if !v.is_object() {
            return Err(invalid(KEY, "expected an object"));
        }
        let mut known: Vec<&str> = SwitchKey::ALL.iter().map(|k| k.key()).collect();
        known.push("apps");
        check_known(KEY, v, &known)?;
        let mut c = Compat::default();
        for key in SwitchKey::ALL {
            match v.get(key.key()) {
                None | Some(Value::Null) => {}
                Some(Value::Bool(b)) => c.set_switch(key, *b),
                Some(_) => {
                    return Err(invalid(
                        &format!("compat.{}", key.key()),
                        "expected on or off (true or false)",
                    ));
                }
            }
        }
        let items = match v.get("apps") {
            None | Some(Value::Null) => &[][..],
            Some(Value::Array(items)) => items.as_slice(),
            Some(_) => return Err(invalid("compat.apps", "expected a list")),
        };
        if items.len() > MAX_APPS {
            return Err(invalid("compat.apps", "at most 100 app fixes"));
        }
        let mut seen = BTreeSet::new();
        for (i, item) in items.iter().enumerate() {
            let key = format!("compat.apps[{i}]");
            let fix = parse_fix(&key, item)?;
            if !seen.insert(fix.id.clone()) {
                return Err(invalid(
                    &format!("{key}.id"),
                    "this app already has a fix above",
                ));
            }
            c.apps.push(fix);
        }
        Ok(c)
    }

    /// Drop the fixes that break a rule (the first of a repeated id wins) and those over the
    /// limit, with a log line each. Loading never fails for it.
    pub fn repaired(mut self) -> Compat {
        let mut seen = BTreeSet::new();
        let mut i = 0;
        self.apps.retain_mut(|fix| {
            let at = i;
            i += 1;
            fix.id = normalise_id(&fix.id);
            if let Err(e) = check_fix("compat.apps", fix) {
                tracing::warn!("state: dropping compat.apps[{at}]: {e}");
                false
            } else if !seen.insert(fix.id.clone()) {
                tracing::warn!("state: dropping compat.apps[{at}]: its app already has a fix");
                false
            } else {
                true
            }
        });
        if self.apps.len() > MAX_APPS {
            tracing::warn!("state: compat has more than {MAX_APPS} app fixes; keeping the first");
            self.apps.truncate(MAX_APPS);
        }
        self
    }

    pub fn section(&self) -> Value {
        serde_json::to_value(self).expect("compat serialises")
    }

    pub fn switch(&self, key: SwitchKey) -> bool {
        match key {
            SwitchKey::SharpX11 => self.sharp_x11,
            SwitchKey::ElectronWayland => self.electron_wayland,
            SwitchKey::ChromiumWayland => self.chromium_wayland,
            SwitchKey::QtWayland => self.qt_wayland,
            SwitchKey::SteamScale => self.steam_scale,
        }
    }

    pub fn set_switch(&mut self, key: SwitchKey, on: bool) {
        *match key {
            SwitchKey::SharpX11 => &mut self.sharp_x11,
            SwitchKey::ElectronWayland => &mut self.electron_wayland,
            SwitchKey::ChromiumWayland => &mut self.chromium_wayland,
            SwitchKey::QtWayland => &mut self.qt_wayland,
            SwitchKey::SteamScale => &mut self.steam_scale,
        } = on;
    }

    /// The fix for a desktop id, with or without its `.desktop`.
    pub fn fix_for(&self, id: &str) -> Option<&AppFix> {
        let id = normalise_id(id);
        self.apps.iter().find(|f| f.id == id)
    }
}

#[cfg(test)]
mod tests;
