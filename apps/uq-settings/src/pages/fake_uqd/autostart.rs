//! The fake uqd's startup apps: a scripted `ListAutostart` list, a switch that makes it fail,
//! and a count of `ShellReady` calls. Also `Explain("env")` and `Explain("autostart")`, built
//! by uq-core's `startup_explain` as uqd builds them.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};
use uq_core::autostart::{Added, Autostart};
use uq_core::startup_explain::{FileReport, explain_autostart, explain_env};
use uq_core::state::State;

use super::FakeError;

const NEXT_LOGIN: &str = "Starts next time you log in";

#[derive(Clone, Default)]
pub struct FakeAutostart {
    /// The entries found in the autostart folders, as uqd lists them. `ListAutostart` applies
    /// the state's choices to them and adds the added entries, as uqd does.
    pub rows: Arc<Mutex<Vec<Value>>>,
    /// When set, `ListAutostart` fails.
    pub fails: Arc<AtomicBool>,
    pub shell_ready: Arc<AtomicU32>,
}

impl FakeAutostart {
    pub fn set(&self, rows: Vec<Value>) {
        *self.rows.lock().unwrap() = rows;
    }

    /// Make every `ListAutostart` fail.
    pub fn fail(&self) {
        self.fails.store(true, Ordering::SeqCst);
    }

    pub fn shell_ready_calls(&self) -> u32 {
        self.shell_ready.load(Ordering::SeqCst)
    }

    /// `ListAutostart` over the state's `autostart` section.
    pub fn list(&self, state: &Autostart) -> Result<String, FakeError> {
        if self.fails.load(Ordering::SeqCst) {
            return Err(FakeError::Io("can't read the startup folders".into()));
        }
        let mut out: Vec<Value> = self
            .rows
            .lock()
            .unwrap()
            .iter()
            .map(|row| with_choice(row.clone(), state))
            .collect();
        out.extend(state.added.iter().map(added_row));
        Ok(Value::Array(out).to_string())
    }
}

/// A status with no extras, dropping the last run's.
fn set_status(row: &mut Value, status: &str, reason: Option<&str>) {
    if let Some(o) = row.as_object_mut() {
        for k in ["reason", "exit", "signal", "log"] {
            o.remove(k);
        }
        o.insert("status".into(), json!(status));
        if let Some(r) = reason {
            o.insert("reason".into(), json!(r));
        }
    }
}

/// A found entry with the person's choice about it: the switch and the delay. A change takes
/// effect at the next login, so an entry turned on reads "Starts next time you log in".
fn with_choice(mut row: Value, state: &Autostart) -> Value {
    let id = row["id"].as_str().unwrap_or_default().to_string();
    let Some(choice) = state.choice(&id) else {
        return row;
    };
    if let Some(d) = choice.delay {
        row["delay"] = json!(d);
    }
    if row["can_toggle"] != true {
        return row;
    }
    if let Some(on) = choice.enabled {
        row["enabled"] = json!(on);
        if !on {
            set_status(&mut row, "off", None);
        } else if row["status"] == "off" {
            set_status(&mut row, "not_started", Some(NEXT_LOGIN));
        }
    }
    row
}

fn added_row(a: &Added) -> Value {
    let mut row = json!({"id": a.id(), "name": a.name(), "icon": null, "source": "added",
        "enabled": a.enabled(), "can_toggle": true, "delay": a.delay().unwrap_or(0)});
    if a.enabled() {
        set_status(&mut row, "not_started", Some(NEXT_LOGIN));
    } else {
        set_status(&mut row, "off", None);
    }
    if let Added::Command { argv, .. } = a {
        row["argv"] = json!(argv);
    }
    row
}

/// `Explain` for `env` and `autostart`, as uqd's: the core rows (no `user.lua` here), then the
/// test's overrides, keyed `"<domain>.<row key>"`.
pub fn explain(
    domain: &str,
    state: &State,
    files: &FileReport,
    rejected: Option<&str>,
    overrides: &Map<String, Value>,
) -> Value {
    let mut out = match domain {
        "env" => explain_env(state, None, rejected, files, "user.lua"),
        _ => explain_autostart(state, files),
    };
    for (key, row) in out.as_object_mut().into_iter().flatten() {
        if let Some(Value::Object(o)) = overrides.get(&format!("{domain}.{key}")) {
            for (k, v) in o {
                row[k] = v.clone();
            }
        }
    }
    out
}
