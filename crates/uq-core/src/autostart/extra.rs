//! The Startup section's session fields: restore the last session, the condition an entry
//! waits for, and the commands run at logout. Each refusal is a plain sentence.

use super::check::{check_added, parse_added};
use super::{Added, valid_added_id};
use crate::desktop_entry::valid_desktop_id;
use crate::settings::{SettingError, invalid};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The most commands run at logout.
pub const MAX_LOGOUT: usize = 20;
/// The most entries with a condition.
pub const MAX_WHEN: usize = 250;

/// What an entry waits for before it starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum When {
    /// A network connection (up to [`NETWORK_WAIT_SECS`]).
    Network,
    /// Mains power: on battery it isn't started this login.
    Power,
}

/// How long an entry that waits for the network waits before it starts anyway.
pub const NETWORK_WAIT_SECS: u64 = 60;

impl When {
    pub fn parse(s: &str) -> Option<When> {
        match s {
            "network" => Some(When::Network),
            "power" => Some(When::Power),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            When::Network => "network",
            When::Power => "power",
        }
    }
}

/// An id an entry can have: a launcher file's (`syncthing.desktop`) or an added one's (`a-1f2e`).
pub fn valid_entry_id(id: &str) -> bool {
    valid_desktop_id(id) || valid_added_id(id)
}

const WHEN_REASON: &str = "a condition is network or power";

pub(super) fn parse_when(o: &Map<String, Value>) -> Result<BTreeMap<String, When>, SettingError> {
    let Some(v) = o.get("when").filter(|v| !v.is_null()) else {
        return Ok(BTreeMap::new());
    };
    let Some(m) = v.as_object() else {
        return Err(invalid("autostart.when", "expected a set of entries"));
    };
    if m.len() > MAX_WHEN {
        return Err(invalid("autostart.when", "at most 250 conditions"));
    }
    let mut out = BTreeMap::new();
    for (id, w) in m {
        let key = format!("autostart.when.{id}");
        if !valid_entry_id(id) {
            return Err(invalid(
                &key,
                "an entry is named by its file name or its id",
            ));
        }
        let w = w
            .as_str()
            .and_then(When::parse)
            .ok_or_else(|| invalid(&key, WHEN_REASON))?;
        out.insert(id.clone(), w);
    }
    Ok(out)
}

/// Load-time reading: bad conditions are dropped one by one.
pub(super) fn lenient_when(o: &Map<String, Value>) -> BTreeMap<String, When> {
    o.get("when")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(id, _)| valid_entry_id(id))
        .filter_map(|(id, w)| Some((id.clone(), When::parse(w.as_str()?)?)))
        .take(MAX_WHEN)
        .collect()
}

/// A logout command is an added command with no delay and no start-hidden.
pub(super) fn check_logout(key: &str, a: &Added) -> Result<(), SettingError> {
    check_added(key, a)?;
    match a {
        Added::Command {
            delay, minimize, ..
        } if delay.is_none() && !minimize => Ok(()),
        Added::Command { .. } => Err(invalid(
            key,
            "a command at logout has no delay and doesn't start hidden",
        )),
        Added::App { .. } => Err(invalid(key, "at logout only commands are run")),
    }
}

pub(super) fn parse_logout(key: &str, v: &Value) -> Result<Added, SettingError> {
    let a = parse_added(key, v)?;
    check_logout(key, &a)?;
    Ok(a)
}

pub(super) fn parse_restore(o: &Map<String, Value>) -> Result<bool, SettingError> {
    match o.get("restore").filter(|v| !v.is_null()) {
        None => Ok(false),
        Some(b) => b
            .as_bool()
            .ok_or_else(|| invalid("autostart.restore", "expected on or off (true or false)")),
    }
}
