//! The identity, main display, bar choice, workspace planner and saved arrangements.

use super::*;
use crate::history::summarize;
use crate::state::State;
use serde_json::json;
use std::collections::BTreeSet;

fn section(v: Value) -> Result<Monitors, SettingError> {
    Monitors::from_section(&v)
}

fn id(make: &str, model: &str, serial: &str) -> Value {
    json!({"make": make, "model": model, "serial": serial})
}

fn arrangement(aid: &str, name: &str, serial: &str) -> Value {
    json!({"id": aid, "name": name, "workspaces": null,
        "outputs": [{"monitor": id("Dell Inc.", "DELL U2720Q", serial), "hint": "DP-1",
            "config": {"mode": "2560x1440@144", "position": {"x": 0, "y": 0}}}]})
}

fn full() -> Value {
    json!({
        "outputs": {"DP-1": {"mode": "preferred"}},
        "primary": id("Dell Inc.", "DELL U2720Q", "ABC123"),
        "auto_profiles": false,
        "bar": {"show": "chosen", "on": [id("Dell Inc.", "DELL U2720Q", "ABC123")]},
        "workspaces": {"strategy": "manual", "count": 8,
            "manual": [{"workspace": 2, "monitor": id("LG", "27GL850", "X1")}],
            "move_back": false},
        "profiles": [arrangement("m-7f2c", "Desk", "ABC123")],
    })
}

fn refused(v: Value, key: &str) {
    match section(v.clone()) {
        Err(SettingError::Invalid { key: k, reason }) => {
            assert!(
                k.starts_with(key),
                "{v}: key {k:?} should start with {key:?}"
            );
            assert!(!reason.is_empty());
        }
        other => panic!("{v} should be refused, got {other:?}"),
    }
}

fn lines(before: Value, after: Value) -> String {
    summarize("monitors", &before, &after, &|d, k| {
        crate::history::label(d, k)
    })
}

mod identity;
mod load;
