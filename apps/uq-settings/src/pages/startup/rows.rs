//! The snapshot's rows: each startup app with its plain status line, each variable with where
//! its value comes from, and the read-only switches.

use serde_json::{Value, json};
use uq_core::compat::hypr::SwitchKey;
use uq_core::desktop_entry::quote_command;
use uq_core::env;

pub(super) const NEXT_LOGIN: &str = "Starts next time you log in";

/// The Explain entry for `key` as the page shows it: `{source, detail}`, or null.
pub(crate) fn origin(explained: &Value, key: &str) -> Value {
    explained.get(key).map_or(
        Value::Null,
        |r| json!({"source": r["source"], "detail": r["detail"]}),
    )
}

/// A section row that says the last write failed: `{source, message}`, or null.
pub(crate) fn section_error(explained: &Value, key: &str) -> Value {
    match explained.get(key) {
        Some(r) if r["source"] == "error" => {
            json!({"source": "error", "message": r["detail"]["message"]})
        }
        _ => Value::Null,
    }
}

/// The plain status line of a `ListAutostart` row. `off_in_uq`: the person's own entry is off
/// only in uQ's settings, not in its file (R15).
pub(super) fn status_text(row: &Value, off_in_uq: bool) -> String {
    let reason = row["reason"].as_str();
    let text = match row["status"].as_str().unwrap_or_default() {
        "running" => "Running",
        "exited" => "Started, then finished",
        "failed" => {
            return if let Some(code) = row["exit"].as_i64() {
                format!("Failed (exit {code})")
            } else if let Some(signal) = row["signal"].as_i64() {
                format!("Failed (stopped by signal {signal})")
            } else if let Some(r) = reason {
                format!("Failed: {r}")
            } else {
                "Failed".to_string()
            };
        }
        "waiting" => {
            return match row["delay"].as_u64() {
                Some(d) if d > 0 => format!("Waiting {d} s"),
                _ => "Waiting".to_string(),
            };
        }
        "off" if off_in_uq => "Off in uQ",
        "off" => "Off",
        "not_started" => reason.unwrap_or(NEXT_LOGIN),
        "skipped" => reason.unwrap_or("Not started"),
        "unknown_exit" => reason.unwrap_or("Started earlier; finished"),
        _ => reason.unwrap_or_default(),
    };
    text.to_string()
}

/// Whether `id` is the person's own entry turned off in the `autostart` section.
fn off_in_uq(row: &Value, section: &Value, id: &str) -> bool {
    row["source"] == "yours"
        && section["files"]
            .as_array()
            .is_some_and(|f| f.iter().any(|c| c["id"] == id && c["enabled"] == false))
}

/// One app row of the snapshot from uqd's `ListAutostart` row.
pub(super) fn app_row(row: &Value, section: &Value, explained: &Value) -> Value {
    let id = row["id"].as_str().unwrap_or_default();
    // Shown only; the command is stored and run as its parts, never through a shell (F22).
    let command_text = row["argv"].as_array().map(|parts| {
        let argv: Vec<String> = parts
            .iter()
            .filter_map(|p| p.as_str().map(String::from))
            .collect();
        quote_command(&argv)
    });
    json!({
        "id": id, "name": row["name"], "icon": row["icon"], "source": row["source"],
        "enabled": row["enabled"], "can_toggle": row["can_toggle"], "delay": row["delay"],
        "status": row["status"],
        "status_text": status_text(row, off_in_uq(row, section, id)),
        "reason": row["reason"], "exit": row["exit"], "signal": row["signal"],
        "log": row.get("log").cloned().unwrap_or_else(|| json!([])),
        "command_text": command_text,
        "origin": origin(explained, id),
    })
}

/// The variables in list order, each with its warning and where its value comes from.
pub(super) fn var_rows(section: &Value, explained: &Value) -> Vec<Value> {
    let vars = section["vars"].as_array().map_or(&[][..], Vec::as_slice);
    vars.iter()
        .map(|v| {
            let name = v["name"].as_str().unwrap_or_default();
            let origin = origin(explained, &format!("vars.{name}"));
            let detail = &origin["detail"];
            let replaced_by = detail["replaced_by"]
                .as_str()
                .and_then(SwitchKey::from_key)
                .map(SwitchKey::label);
            json!({
                "name": name, "value": v["value"],
                "warn": matches!(env::check_name("env", name), Ok(Some(_))),
                "line": detail["line"], "replaced_by_label": replaced_by,
                "origin": origin,
            })
        })
        .collect()
}

/// The §6 switches, read-only here, from the `compat` section.
pub(super) fn switches(compat: &Value) -> Vec<Value> {
    SwitchKey::ALL
        .iter()
        .map(|k| {
            json!({"key": k.key(), "label": k.label(),
                "on": compat[k.key()].as_bool().unwrap_or(false)})
        })
        .collect()
}
