//! Reading `systemctl --user`'s output into the rows the Services tab shows.

use std::collections::BTreeMap;

use serde_json::{Value, json};

/// A unit file's install state as `list-unit-files` prints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitFile {
    pub unit: String,
    pub state: String,
}

/// The unit files in `list-unit-files --no-legend --plain` output, without templates
/// (`name@.service` can't start on its own).
pub fn unit_files(out: &str) -> Vec<UnitFile> {
    out.lines()
        .filter_map(|line| {
            let mut cols = line.split_whitespace();
            let unit = cols.next()?;
            let state = cols.next()?;
            (unit.ends_with(".service") && !unit.contains("@.")).then(|| UnitFile {
                unit: unit.to_string(),
                state: state.to_string(),
            })
        })
        .collect()
}

/// `show --property=…` output: one block of `Key=value` lines per unit, blocks split by an
/// empty line, keyed by `Id`.
pub fn shown(out: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut all = BTreeMap::new();
    for block in out.split("\n\n") {
        let props: BTreeMap<String, String> = block
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        if let Some(id) = props.get("Id").filter(|id| !id.is_empty()) {
            all.insert(id.clone(), props);
        }
    }
    all
}

/// Install states whose unit can be switched on and off for login.
const SWITCHABLE: &[&str] = &[
    "enabled",
    "disabled",
    "enabled-runtime",
    "linked",
    "linked-runtime",
];

/// Whether a unit in install state `state` is listed when "show all" is off: the ones a
/// person can turn on or off, and the blocked ones (so they know why one never starts).
pub fn listed_by_default(state: &str) -> bool {
    SWITCHABLE.contains(&state) || state == "masked"
}

/// uQ's own services: the desktop needs them, so the page shows them but won't change them.
pub fn locked(unit: &str) -> bool {
    unit.starts_with("uq")
}

/// The words for a unit's run state.
pub fn status(active: &str, sub: &str) -> (&'static str, &'static str) {
    match (active, sub) {
        ("active", "exited") => ("done", "Ran and finished"),
        ("active", _) => ("running", "Running"),
        ("reloading", _) => ("running", "Reloading"),
        ("activating", _) => ("starting", "Starting"),
        ("deactivating", _) => ("stopping", "Stopping"),
        ("failed", _) => ("failed", "Failed"),
        _ => ("stopped", "Not running"),
    }
}

/// One row: `{unit, name, description, install, on, switchable, blocked, locked, state,
/// status}`.
pub fn row(file: &UnitFile, props: Option<&BTreeMap<String, String>>) -> Value {
    let get = |k: &str| {
        props
            .and_then(|p| p.get(k))
            .map(String::as_str)
            .unwrap_or("")
    };
    let (state, status) = status(get("ActiveState"), get("SubState"));
    let stem = file.unit.trim_end_matches(".service");
    let description = get("Description");
    let description = if description == file.unit {
        ""
    } else {
        description
    };
    json!({
        "unit": file.unit,
        "name": stem,
        "description": description,
        "install": file.state,
        "on": file.state.starts_with("enabled") || file.state.starts_with("linked"),
        "switchable": SWITCHABLE.contains(&file.state.as_str()) && !locked(&file.unit),
        "blocked": file.state == "masked",
        "locked": locked(&file.unit),
        "state": state,
        "status": status,
    })
}

/// Whether any row is between states, so the page should look again soon.
pub fn settling(rows: &[Value]) -> bool {
    rows.iter()
        .any(|r| matches!(r["state"].as_str(), Some("starting" | "stopping")))
}
