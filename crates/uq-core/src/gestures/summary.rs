//! Plain lines for a change to `input.gestures` (spec §3). Never JSON.

use super::{Gesture, defaults};
use serde_json::Value;

fn list(v: &Value) -> Vec<Gesture> {
    match v.get("gestures") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|g| serde_json::from_value(g.clone()).ok())
            .collect(),
        _ => defaults(),
    }
}

/// One line per added, removed, turned-off, turned-on or changed gesture, matched by their
/// fingers, modifier and direction; "back to uQ's defaults" when the list is again the
/// defaults. Empty when nothing changed.
pub fn summary(before: &Value, after: &Value) -> Vec<String> {
    let (old, new) = (list(before), list(after));
    if old == new {
        return Vec::new();
    }
    if new == defaults() {
        return vec!["Touchpad gestures back to uQ's defaults".into()];
    }
    let same_trigger = |a: &Gesture, b: &Gesture| {
        (a.fingers, a.modifier, a.direction) == (b.fingers, b.modifier, b.direction)
    };
    let mut lines = Vec::new();
    let mut matched = vec![false; old.len()];
    for n in &new {
        let at = (0..old.len()).find(|&i| !matched[i] && same_trigger(&old[i], n));
        match at {
            None => lines.push(format!("Touchpad gesture added: {}", n.describe())),
            Some(i) => {
                matched[i] = true;
                let o = &old[i];
                if o == n {
                    // Unchanged.
                } else if o.enabled && !n.enabled {
                    lines.push(format!("Touchpad gesture turned off: {}", n.describe()));
                } else if !o.enabled && n.enabled {
                    lines.push(format!("Touchpad gesture turned on: {}", n.describe()));
                } else {
                    lines.push(format!("Touchpad gesture changed: {}", n.describe()));
                }
            }
        }
    }
    for o in old
        .iter()
        .zip(&matched)
        .filter(|(_, m)| !**m)
        .map(|(o, _)| o)
    {
        lines.push(format!("Touchpad gesture removed: {}", o.describe()));
    }
    lines
}
