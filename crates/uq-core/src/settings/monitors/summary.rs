//! One plain line for a change to the `monitors` section (spec §3): the main display, saved
//! arrangements, the bar choice and the planner are written out in words; every other leaf
//! falls back to the generic "label: old → new" lines. Never JSON.

use super::{MonitorId, MonitorProfile};
use crate::history::Labels;
use crate::history::summarize_leaves;
use serde_json::Value;

const MAX_NAMED: usize = 3;

fn profiles(v: &Value) -> Vec<MonitorProfile> {
    v.get("profiles")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|p| serde_json::from_value(p.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn primary(v: &Value) -> Option<MonitorId> {
    v.get("primary")
        .and_then(|p| serde_json::from_value(p.clone()).ok())
}

fn bar_line(v: &Value) -> String {
    let bar = v.get("bar");
    let show = bar
        .and_then(|b| b.get("show"))
        .and_then(Value::as_str)
        .unwrap_or("all");
    let n = bar
        .and_then(|b| b.get("on"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    match show {
        "main" => "Bar shown on the main display only".into(),
        "chosen" if n == 1 => "Bar shown on 1 chosen screen".into(),
        "chosen" => format!("Bar shown on {n} chosen screens"),
        _ => "Bar shown on every screen".into(),
    }
}

fn strategy(v: &Value) -> &str {
    v.get("workspaces")
        .and_then(|w| w.get("strategy"))
        .and_then(Value::as_str)
        .unwrap_or("off")
}

fn strategy_words(s: &str) -> &'static str {
    match s {
        "sequential" => "one after another",
        "interleave" => "in turn",
        "manual" => "by hand",
        _ => "off",
    }
}

fn quoted(name: &str) -> String {
    format!("“{name}”")
}

fn arrangement_lines(before: &Value, after: &Value, lines: &mut Vec<String>) {
    let (old, new) = (profiles(before), profiles(after));
    for n in &new {
        match old.iter().find(|o| o.id == n.id) {
            None => lines.push(format!("Display arrangement {} saved", quoted(&n.name))),
            Some(o) if o.name != n.name => lines.push(format!(
                "Display arrangement {} renamed to {}",
                quoted(&o.name),
                quoted(&n.name)
            )),
            Some(o) if o != n => {
                lines.push(format!("Display arrangement {} updated", quoted(&n.name)))
            }
            Some(_) => {}
        }
    }
    for o in old.iter().filter(|o| !new.iter().any(|n| n.id == o.id)) {
        lines.push(format!("Display arrangement {} removed", quoted(&o.name)));
    }
}

/// What is left to the generic lines once the keys above are taken out.
fn rest(v: &Value) -> Value {
    let mut v = v.clone();
    let Some(o) = v.as_object_mut() else {
        return v;
    };
    for k in ["primary", "profiles", "bar"] {
        o.remove(k);
    }
    if let Some(w) = o.get_mut("workspaces").and_then(Value::as_object_mut) {
        w.remove("strategy");
        // A plan written out in full (strategy on) repeats the defaults of one that was
        // left out (strategy off); those aren't changes.
        for (k, default) in [
            ("count", Value::from(10)),
            ("manual", Value::Array(Vec::new())),
            ("move_back", Value::Bool(true)),
        ] {
            if w.get(k) == Some(&default) {
                w.remove(k);
            }
        }
        if w.is_empty() {
            o.remove("workspaces");
        }
    }
    v
}

pub fn summary(before: &Value, after: &Value, labels: Labels) -> String {
    let mut lines = Vec::new();
    if primary(before) != primary(after) {
        lines.push(match primary(after) {
            Some(p) => format!("Main display set to {}", p.label()),
            None => "Main display cleared".into(),
        });
    }
    arrangement_lines(before, after, &mut lines);
    if before.get("bar") != after.get("bar") {
        lines.push(bar_line(after));
    }
    if strategy(before) != strategy(after) {
        lines.push(format!(
            "Workspaces across screens: {}",
            strategy_words(strategy(after))
        ));
    }
    let (b, a) = (rest(before), rest(after));
    if b != a {
        lines.push(summarize_leaves("monitors", &b, &a, labels));
    }
    let mut line = lines
        .iter()
        .take(MAX_NAMED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if lines.len() > MAX_NAMED {
        line.push_str(&format!(", and {} more", lines.len() - MAX_NAMED));
    }
    if line.is_empty() {
        line = "Displays".into();
    }
    line
}
