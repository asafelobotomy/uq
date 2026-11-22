//! Startup apps (spec §7.3): the user's choices about the XDG autostart entries, and the apps
//! and commands they added.
//!
//! The domain model (`Autostart`), its strict write-time validation, its lenient load-time
//! repair and the History summary. The folder read and the run plan are in [`scan`]. Every
//! refusal is a plain sentence.

mod check;
pub mod scan;
mod summary;

pub use summary::summary;

use crate::desktop_entry::normalise_id;
use crate::rules::fresh_prefixed;
use crate::settings::{SettingError, check_known, invalid};
use check::{check_added, check_choice, items, parse_added, parse_choice};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub const MAX_FILES: usize = 200;
pub const MAX_ADDED: usize = 50;
pub const MAX_DELAY: u32 = 600;
/// The most seconds between two apps' starts.
pub const MAX_STAGGER: u32 = 30;
pub const MAX_ARGV: usize = 64;
pub const MAX_ARG_CHARS: usize = 1024;
pub const MAX_NAME_CHARS: usize = 100;

/// The first line of the hidden copy uQ writes to turn a system entry off.
pub const HIDDEN_COPY_HEADER: &str = "# Written by uQ Settings: turned off on the Startup page. \
                                      uQ removes this file when you turn it back on.";

/// The user's choice about one entry found in the autostart folders. `None` leaves the file's
/// own setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChoice {
    pub id: String,
    pub enabled: Option<bool>,
    pub delay: Option<u32>,
    /// Start the app hidden: to its tray icon, or minimised when it has none. `None` leaves it
    /// to open normally.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimize: Option<bool>,
}

/// An app (by desktop id) or a command (as argv, never through a shell) the user added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Added {
    App {
        id: String,
        name: String,
        app: String,
        enabled: bool,
        delay: Option<u32>,
        #[serde(skip_serializing_if = "is_false")]
        minimize: bool,
    },
    Command {
        id: String,
        name: String,
        argv: Vec<String>,
        enabled: bool,
        delay: Option<u32>,
        #[serde(skip_serializing_if = "is_false")]
        minimize: bool,
    },
}

impl Added {
    pub fn id(&self) -> &str {
        match self {
            Added::App { id, .. } | Added::Command { id, .. } => id,
        }
    }

    fn id_mut(&mut self) -> &mut String {
        match self {
            Added::App { id, .. } | Added::Command { id, .. } => id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Added::App { name, .. } | Added::Command { name, .. } => name,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            Added::App { enabled, .. } | Added::Command { enabled, .. } => *enabled,
        }
    }

    pub fn delay(&self) -> Option<u32> {
        match self {
            Added::App { delay, .. } | Added::Command { delay, .. } => *delay,
        }
    }

    pub fn minimize(&self) -> bool {
        match self {
            Added::App { minimize, .. } | Added::Command { minimize, .. } => *minimize,
        }
    }

    pub fn set_minimize(&mut self, on: bool) {
        match self {
            Added::App { minimize, .. } | Added::Command { minimize, .. } => *minimize = on,
        }
    }

    pub fn set_enabled(&mut self, on: bool) {
        match self {
            Added::App { enabled, .. } | Added::Command { enabled, .. } => *enabled = on,
        }
    }

    pub fn set_delay(&mut self, d: Option<u32>) {
        match self {
            Added::App { delay, .. } | Added::Command { delay, .. } => *delay = d,
        }
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// The Startup section.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Autostart {
    pub files: Vec<FileChoice>,
    pub added: Vec<Added>,
    /// Seconds between one app's start and the next (0 starts them together).
    #[serde(skip_serializing_if = "is_zero")]
    pub stagger: u32,
    /// Start an app that failed again once, ten seconds later.
    #[serde(skip_serializing_if = "is_false")]
    pub retry: bool,
}

fn field_of<'a>(o: &'a Map<String, Value>, f: &str) -> Option<&'a Value> {
    o.get(f).filter(|v| !v.is_null())
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// `^a-[0-9a-f]{4,8}$`
pub fn valid_added_id(id: &str) -> bool {
    id.strip_prefix("a-").is_some_and(|h| {
        (4..=8).contains(&h.len()) && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

/// Entries load one by one: one that doesn't parse or breaks a rule (a hand edit, or a newer
/// uQ's) is dropped with a log line and the others are kept; a list that isn't a list is empty.
impl<'de> Deserialize<'de> for Autostart {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Autostart, D::Error> {
        let v = Value::deserialize(d)?;
        let empty = Map::new();
        let o = v.as_object().unwrap_or(&empty);
        let each = |f: &str| {
            items(o, f).unwrap_or_else(|_| {
                tracing::warn!("[STATE-002] state: autostart.{f} is not a list; ignoring it");
                &[]
            })
        };
        let keep = |f: &str, e: SettingError| {
            tracing::warn!("[STATE-002] state: dropping an entry of autostart.{f}: {e}");
        };
        let files = each("files")
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                parse_choice(&format!("autostart.files[{i}]"), e)
                    .map_err(|err| keep("files", err))
                    .ok()
            })
            .collect();
        let added = each("added")
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                parse_added(&format!("autostart.added[{i}]"), e)
                    .map_err(|err| keep("added", err))
                    .ok()
            })
            .collect();
        // A bad value of these is ignored, not a reason to lose the entries.
        let stagger = o
            .get("stagger")
            .and_then(Value::as_u64)
            .filter(|n| *n <= u64::from(MAX_STAGGER))
            .map_or(0, |n| n as u32);
        let retry = o.get("retry").and_then(Value::as_bool).unwrap_or(false);
        Ok(Autostart {
            files,
            added,
            stagger,
            retry,
        }
        .repaired())
    }
}

impl Autostart {
    /// A whole-section write: strict, the whole section refused for its first bad entry.
    /// `current` only steers new ids; ids that exist now are accepted again, and ids must be
    /// unique within `v`.
    pub fn from_section(v: &Value, current: &Autostart) -> Result<Autostart, SettingError> {
        let Some(o) = v.as_object() else {
            return Err(invalid("autostart", "expected an object"));
        };
        check_known("autostart", v, &["files", "added", "stagger", "retry"])?;
        let files = items(o, "files")?;
        if files.len() > MAX_FILES {
            return Err(invalid("autostart.files", "at most 200 startup choices"));
        }
        let added = items(o, "added")?;
        if added.len() > MAX_ADDED {
            return Err(invalid(
                "autostart.added",
                "at most 50 added startup entries",
            ));
        }
        let stagger = match field_of(o, "stagger") {
            None => 0,
            Some(n) => n
                .as_u64()
                .filter(|n| *n <= u64::from(MAX_STAGGER))
                .map(|n| n as u32)
                .ok_or_else(|| invalid("autostart.stagger", "a gap is 0 to 30 seconds"))?,
        };
        let retry = match field_of(o, "retry") {
            None => false,
            Some(b) => b
                .as_bool()
                .ok_or_else(|| invalid("autostart.retry", "expected on or off (true or false)"))?,
        };
        let mut out = Autostart {
            stagger,
            retry,
            ..Autostart::default()
        };
        let mut seen = BTreeSet::new();
        for (i, item) in files.iter().enumerate() {
            let key = format!("autostart.files[{i}]");
            let c = parse_choice(&key, item)?;
            if !seen.insert(c.id.clone()) {
                return Err(invalid(&key, "this entry already has a choice above"));
            }
            out.files.push(c);
        }
        let mut ids = BTreeSet::new();
        for (i, item) in added.iter().enumerate() {
            let key = format!("autostart.added[{i}]");
            let a = parse_added(&key, item)?;
            if !a.id().is_empty() {
                if !valid_added_id(a.id()) {
                    return Err(invalid(&format!("{key}.id"), "an id looks like a-1a2b"));
                }
                if !ids.insert(a.id().to_string()) {
                    return Err(invalid(&format!("{key}.id"), "another entry has this id"));
                }
            }
            out.added.push(a);
        }
        ids.extend(current.added.iter().map(|a| a.id().to_string()));
        out.fill_ids(&mut ids);
        Ok(out)
    }

    fn fill_ids(&mut self, taken: &mut BTreeSet<String>) {
        for a in &mut self.added {
            if a.id().is_empty() {
                let id = fresh_prefixed("a-", taken);
                taken.insert(id.clone());
                *a.id_mut() = id;
            }
        }
    }

    /// Load-time repair, so the section stays writable: entries that break a rule, repeat an
    /// id, or pass the limit are dropped with a log line. A bad or repeated added id doesn't
    /// cost the entry: a new one is made, as for a missing id.
    pub fn repaired(mut self) -> Autostart {
        let mut seen = BTreeSet::new();
        self.files.retain_mut(|c| {
            c.id = normalise_id(&c.id);
            match check_choice("autostart.files", c) {
                Err(e) => {
                    tracing::warn!("[STATE-002] state: dropping a startup choice: {e}");
                    false
                }
                Ok(()) if !seen.insert(c.id.clone()) => {
                    tracing::warn!("[STATE-002] state: dropping a repeated startup choice");
                    false
                }
                Ok(()) => true,
            }
        });
        self.added
            .retain(|a| match check_added("autostart.added", a) {
                Err(e) => {
                    tracing::warn!("[STATE-002] state: dropping an added startup entry: {e}");
                    false
                }
                Ok(()) => true,
            });
        let mut ids = BTreeSet::new();
        for a in &mut self.added {
            let id = a.id_mut();
            if !id.is_empty() && !(valid_added_id(id) && ids.insert(id.clone())) {
                tracing::warn!(
                    "[STATE-002] state: an added startup entry has a bad or repeated id"
                );
                id.clear();
            }
        }
        if self.files.len() > MAX_FILES || self.added.len() > MAX_ADDED {
            tracing::warn!(
                "[STATE-002] state: autostart is over its limits; keeping the first entries"
            );
        }
        self.files.truncate(MAX_FILES);
        self.added.truncate(MAX_ADDED);
        let mut ids: BTreeSet<String> = self.added.iter().map(|a| a.id().to_string()).collect();
        self.fill_ids(&mut ids);
        self
    }

    pub fn section(&self) -> Value {
        serde_json::to_value(self).expect("autostart serialises")
    }

    /// The choice about a found entry, by its file id.
    pub fn choice(&self, id: &str) -> Option<&FileChoice> {
        self.files.iter().find(|c| c.id == id)
    }

    /// Drop the choices that set nothing.
    pub fn prune_empty_choices(&mut self) {
        self.files
            .retain(|c| c.enabled.is_some() || c.delay.is_some() || c.minimize.is_some());
    }
}

#[cfg(test)]
mod tests;
