//! The History line for an Apps and scaling change.

use super::hypr::SwitchKey;
use crate::desktop_entry::valid_desktop_id;
use crate::env;
use serde_json::Value;

const MAX_NAMED: usize = 3;

/// An app as History names it: its id without `.desktop`, or "an app" for an id that isn't
/// one (a hand-edited journal).
fn app_name(id: &str) -> String {
    if valid_desktop_id(id) {
        env::shown(id.strip_suffix(".desktop").unwrap_or(id))
    } else {
        "an app".to_string()
    }
}

fn apps_of(v: &Value) -> Vec<(&str, &Value)> {
    v.get("apps")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|e| Some((e.get("id")?.as_str()?, e)))
        .collect()
}

/// "Sharp X11 apps turned on" per switch, then "Fix for code added", "… changed" or
/// "… removed": up to three, then ", and N more". No values and no JSON.
pub fn summary(before: &Value, after: &Value) -> String {
    let mut lines = Vec::new();
    for key in SwitchKey::ALL {
        let on = |v: &Value| v.get(key.key()).and_then(Value::as_bool).unwrap_or(false);
        if on(before) != on(after) {
            let what = if on(after) { "on" } else { "off" };
            lines.push(format!("{} turned {what}", key.label()));
        }
    }
    let (b, a) = (apps_of(before), apps_of(after));
    for (id, fix) in &a {
        match b.iter().find(|(i, _)| i == id) {
            None => lines.push(format!("Fix for {} added", app_name(id))),
            Some((_, old)) if old != fix => {
                lines.push(format!("Fix for {} changed", app_name(id)));
            }
            Some(_) => {}
        }
    }
    for (id, _) in &b {
        if !a.iter().any(|(i, _)| i == id) {
            lines.push(format!("Fix for {} removed", app_name(id)));
        }
    }
    if lines.is_empty() {
        let same_order = b.iter().map(|(i, _)| i).eq(a.iter().map(|(i, _)| i));
        return if !same_order {
            "App fixes reordered".to_string()
        } else {
            "Apps and scaling".to_string()
        };
    }
    let mut line = lines[..lines.len().min(MAX_NAMED)].join(", ");
    if lines.len() > MAX_NAMED {
        line.push_str(&format!(", and {} more", lines.len() - MAX_NAMED));
    }
    line
}
