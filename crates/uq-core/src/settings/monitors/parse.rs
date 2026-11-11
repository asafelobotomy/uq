//! Strict parsing of a written `monitors` section's extra keys: unknown fields, wrong types
//! and rule breaks are refused with the key and a plain reason.

use super::extras::{
    BarChoice, MAX_BAR_SCREENS, MAX_PROFILE_SCREENS, MAX_PROFILES, ManualSlot, MonitorProfile,
    SavedOutput, WorkspacePlan,
};
use super::{MonitorId, OutputConfig, SettingError, check_known, invalid};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeSet;

fn object<'a>(
    key: &str,
    v: &'a Value,
    known: &[&str],
) -> Result<&'a serde_json::Map<String, Value>, SettingError> {
    let o = v
        .as_object()
        .ok_or_else(|| invalid(key, "expected an object"))?;
    check_known(key, v, known)?;
    Ok(o)
}

fn from<T: DeserializeOwned>(key: &str, v: &Value) -> Result<T, SettingError> {
    serde_json::from_value(v.clone()).map_err(|e| invalid(key, e.to_string()))
}

pub(super) fn parse_id(key: &str, v: &Value) -> Result<MonitorId, SettingError> {
    object(key, v, &MonitorId::FIELDS)?;
    let id: MonitorId = from(key, v)?;
    id.check(key)?;
    Ok(id)
}

pub(super) fn parse_bar(v: &Value) -> Result<BarChoice, SettingError> {
    const KEY: &str = "monitors.bar";
    let o = object(KEY, v, &["show", "on"])?;
    let mut bar = BarChoice::default();
    if let Some(s) = o.get("show").filter(|s| !s.is_null()) {
        bar.show = from(KEY, s).map_err(|_| invalid(KEY, "show is all, main or chosen"))?;
    }
    match o.get("on") {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            if items.len() > MAX_BAR_SCREENS {
                return Err(invalid(
                    KEY,
                    format!("at most {MAX_BAR_SCREENS} screens can be chosen"),
                ));
            }
            for m in items {
                bar.on.push(parse_id(KEY, m)?);
            }
        }
        Some(_) => return Err(invalid(KEY, "on is a list of screens")),
    }
    bar.validate(KEY)?;
    Ok(bar)
}

pub(super) fn parse_workspaces(key: &str, v: &Value) -> Result<WorkspacePlan, SettingError> {
    let o = object(key, v, &["strategy", "count", "manual", "move_back"])?;
    let mut w = WorkspacePlan::default();
    if let Some(s) = o.get("strategy").filter(|s| !s.is_null()) {
        w.strategy = from(key, s)
            .map_err(|_| invalid(key, "strategy is off, sequential, interleave or manual"))?;
    }
    if let Some(c) = o.get("count").filter(|c| !c.is_null()) {
        w.count = c
            .as_u64()
            .and_then(|n| u8::try_from(n).ok())
            .ok_or_else(|| invalid(key, "count is a whole number from 1 to 20"))?;
    }
    if let Some(b) = o.get("move_back").filter(|b| !b.is_null()) {
        w.move_back = b
            .as_bool()
            .ok_or_else(|| invalid(key, "move_back is true or false"))?;
    }
    match o.get("manual") {
        None | Some(Value::Null) => {}
        Some(Value::Array(slots)) => {
            for s in slots {
                let so = object(key, s, &["workspace", "monitor"])?;
                let workspace = so
                    .get("workspace")
                    .and_then(Value::as_u64)
                    .and_then(|n| u8::try_from(n).ok())
                    .ok_or_else(|| invalid(key, "a slot needs a workspace number"))?;
                let monitor = parse_id(
                    key,
                    so.get("monitor")
                        .ok_or_else(|| invalid(key, "a slot needs a screen"))?,
                )?;
                w.manual.push(ManualSlot { workspace, monitor });
            }
        }
        Some(_) => return Err(invalid(key, "manual is a list")),
    }
    w.validate(key)?;
    Ok(w)
}

fn parse_profile(i: usize, v: &Value) -> Result<MonitorProfile, SettingError> {
    let key = format!("monitors.profiles[{i}]");
    let o = object(&key, v, &MonitorProfile::FIELDS)?;
    let text = |k: &str| match o.get(k) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(invalid(&format!("{key}.{k}"), "expected text")),
    };
    let mut p = MonitorProfile {
        id: text("id")?,
        name: text("name")?,
        ..MonitorProfile::default()
    };
    match o.get("outputs") {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            if items.len() > MAX_PROFILE_SCREENS {
                return Err(invalid(
                    &format!("{key}.outputs"),
                    format!("an arrangement has 1 to {MAX_PROFILE_SCREENS} screens"),
                ));
            }
            for (j, item) in items.iter().enumerate() {
                let k = format!("{key}.outputs[{j}]");
                let io = object(&k, item, &["monitor", "hint", "config"])?;
                let monitor = parse_id(
                    &format!("{k}.monitor"),
                    io.get("monitor")
                        .ok_or_else(|| invalid(&k, "a saved screen needs its identity"))?,
                )?;
                let hint = match io.get("hint") {
                    None | Some(Value::Null) => String::new(),
                    Some(Value::String(s)) => s.clone(),
                    Some(_) => return Err(invalid(&format!("{k}.hint"), "expected text")),
                };
                let config = match io.get("config") {
                    None | Some(Value::Null) => OutputConfig::default(),
                    Some(c) => {
                        if !c.is_object() {
                            return Err(invalid(&k, "config is an object"));
                        }
                        check_known(&k, c, &OutputConfig::FIELDS)?;
                        OutputConfig::parse(&k, c)?
                    }
                };
                p.outputs.push(SavedOutput {
                    monitor,
                    hint,
                    config,
                });
            }
        }
        Some(_) => return Err(invalid(&format!("{key}.outputs"), "expected a list")),
    }
    p.workspaces = match o.get("workspaces") {
        None | Some(Value::Null) => None,
        Some(w) => Some(parse_workspaces(&format!("{key}.workspaces"), w)?),
    };
    Ok(p.normalised())
}

pub(super) fn parse_profiles(v: &Value) -> Result<Vec<MonitorProfile>, SettingError> {
    let items = v
        .as_array()
        .ok_or_else(|| invalid("monitors.profiles", "expected a list"))?;
    if items.len() > MAX_PROFILES {
        return Err(invalid(
            "monitors.profiles",
            format!("at most {MAX_PROFILES} arrangements"),
        ));
    }
    items
        .iter()
        .enumerate()
        .map(|(i, p)| parse_profile(i, p))
        .collect()
}

/// The cross-entry rules: unique ids, one arrangement per set of screens.
pub(super) fn validate_profiles(profiles: &[MonitorProfile]) -> Result<(), SettingError> {
    if profiles.len() > MAX_PROFILES {
        return Err(invalid(
            "monitors.profiles",
            format!("at most {MAX_PROFILES} arrangements"),
        ));
    }
    let (mut ids, mut sets) = (BTreeSet::new(), BTreeSet::new());
    for (i, p) in profiles.iter().enumerate() {
        let key = format!("monitors.profiles[{i}]");
        p.validate(&key)?;
        if !ids.insert(p.id.clone()) {
            return Err(invalid(
                &format!("{key}.id"),
                "two arrangements share an id",
            ));
        }
        if !sets.insert(p.set()) {
            return Err(invalid(
                &key,
                "two arrangements are for the same set of screens",
            ));
        }
    }
    Ok(())
}
