//! Per-domain diff of two states, with a one-line human summary of each change.

use crate::settings::{SECTION_DOMAINS, SETTING_KEYS};
use crate::state::State;
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

/// Looks up the display label for a setting: `(domain, json key)`.
pub type Labels<'a> = &'a dyn Fn(&str, &str) -> Option<String>;

#[derive(Debug, Clone, PartialEq)]
pub struct DomainChange {
    pub domain: String,
    pub before: Value,
    pub after: Value,
    pub summary: String,
}

const MAX_NAMED: usize = 3;

/// Every domain whose content differs between `old` and `new`. `launch_counts` and `welcome`
/// (bookkeeping, not settings) are not compared.
pub fn diff(old: &State, new: &State, labels: Labels) -> Vec<DomainChange> {
    journal_domains()
        .into_iter()
        .filter_map(|d| {
            let (before, after) = (domain_value(old, d), domain_value(new, d));
            (before != after).then(|| DomainChange {
                summary: summarize(d, &before, &after, labels),
                domain: d.to_string(),
                before,
                after,
            })
        })
        .collect()
}

/// Every domain the journal records, in summary order.
pub(crate) fn journal_domains() -> Vec<&'static str> {
    let mut domains: Vec<&str> = SECTION_DOMAINS.to_vec();
    if !domains.contains(&"layouts") {
        domains.push("layouts");
    }
    domains.extend(["theme", "schedule", "settings"]);
    domains
}

/// The dotted keys a summary of `domain` can name for `st`.
#[cfg(test)]
pub(crate) fn leaf_keys(st: &State, domain: &str) -> Vec<String> {
    leaf_map(domain, &domain_value(st, domain))
        .into_keys()
        .collect()
}

fn domain_value(st: &State, domain: &str) -> Value {
    match domain {
        "theme" => json!({"theme": st.theme, "variant": st.variant}),
        "schedule" => serde_json::to_value(&st.schedule).expect("schedule serialises"),
        "settings" => {
            let all = serde_json::to_value(&st.settings).expect("settings serialise");
            let mut m = Map::new();
            // `mouse.*` is the `input` section's mouse part: it is recorded there, once, so
            // the Input page owns it in History.
            for key in SETTING_KEYS.iter().filter(|k| !k.starts_with("mouse.")) {
                let v = key
                    .split('.')
                    .try_fold(&all, |v, part| v.get(part))
                    .cloned()
                    .unwrap_or(Value::Null);
                m.insert(key.to_string(), v);
            }
            Value::Object(m)
        }
        _ => st.section(domain).unwrap_or(Value::Null),
    }
}

/// Object leaves by dotted path. Arrays and scalars are leaves; the `settings` domain's keys
/// are already dotted setting keys, so its values are not descended into.
fn leaves(domain: &str, v: &Value, prefix: &str, out: &mut Vec<(String, Value)>) {
    match v {
        Value::Object(m) if !(domain == "settings" && !prefix.is_empty()) => {
            for (k, child) in m {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                leaves(domain, child, &path, out);
            }
        }
        // An empty object is "nothing set", the same as a missing key.
        Value::Object(m) if m.is_empty() => {}
        _ => out.push((prefix.to_string(), v.clone())),
    }
}

fn leaf_map(domain: &str, v: &Value) -> std::collections::BTreeMap<String, Value> {
    let mut out = Vec::new();
    leaves(domain, v, "", &mut out);
    out.into_iter().collect()
}

fn render(v: &Value) -> String {
    match v {
        Value::Null => "default".into(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// One line naming at most three changed leaves, then "and N more". A leaf is named by the
/// label `labels` knows for `(domain, key)`, else by its JSON key. `null` reads "default".
pub fn summarize(domain: &str, before: &Value, after: &Value, labels: Labels) -> String {
    if domain == "rules" {
        return crate::rules::summary(before, after);
    }
    if domain == "keybinds" {
        return crate::keybinds::summary(before, after);
    }
    if domain == "env" {
        return crate::env::summary(before, after);
    }
    if domain == "compat" {
        return crate::compat::summary(before, after);
    }
    if domain == "autostart" {
        return crate::autostart::summary(before, after);
    }
    if domain == crate::integration::rules::DOMAIN {
        return crate::integration::rules::summary(before, after);
    }
    if domain == "input" {
        return crate::settings::input_summary(before, after, labels);
    }
    if domain == "monitors" {
        return crate::settings::monitors_summary(before, after, labels);
    }
    summarize_leaves(domain, before, after, labels)
}

/// The generic body of [`summarize`]: one line naming at most three changed leaves.
pub(crate) fn summarize_leaves(
    domain: &str,
    before: &Value,
    after: &Value,
    labels: Labels,
) -> String {
    let (b, a) = (leaf_map(domain, before), leaf_map(domain, after));
    let keys: BTreeSet<&String> = b.keys().chain(a.keys()).collect();
    let changed: Vec<String> = keys
        .into_iter()
        .filter_map(|k| {
            let (old, new) = (
                b.get(k).unwrap_or(&Value::Null),
                a.get(k).unwrap_or(&Value::Null),
            );
            (old != new).then(|| {
                let name = labels(domain, k).unwrap_or_else(|| k.clone());
                format!("{name}: {} → {}", render(old), render(new))
            })
        })
        .collect();
    let mut line = changed
        .iter()
        .take(MAX_NAMED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if changed.len() > MAX_NAMED {
        line.push_str(&format!(", and {} more", changed.len() - MAX_NAMED));
    }
    if line.is_empty() {
        line = domain.to_string();
    }
    line
}

/// Object leaves by path segment (arrays and scalars are leaves; an empty object is none).
pub(crate) fn segment_leaves(
    v: &Value,
    prefix: &mut Vec<String>,
    out: &mut Vec<(Vec<String>, Value)>,
) {
    match v {
        Value::Object(m) => {
            for (k, child) in m {
                prefix.push(k.clone());
                segment_leaves(child, prefix, out);
                prefix.pop();
            }
        }
        _ => out.push((prefix.clone(), v.clone())),
    }
}

pub(crate) fn get_path<'a>(v: &'a Value, path: &[String]) -> Option<&'a Value> {
    path.iter().try_fold(v, |v, part| v.get(part))
}

/// Set `path` in `root` to `value`, creating objects on the way; `None` removes it and then
/// any object on the way that this left empty.
pub(crate) fn put_path(root: &mut Value, path: &[String], value: Option<Value>) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    match value {
        Some(value) => {
            let mut at = root;
            for part in parents {
                if !at.is_object() {
                    *at = Value::Object(Map::new());
                }
                at = at
                    .as_object_mut()
                    .expect("made an object")
                    .entry(part.clone())
                    .or_insert_with(|| Value::Object(Map::new()));
            }
            if !at.is_object() {
                *at = Value::Object(Map::new());
            }
            at.as_object_mut()
                .expect("made an object")
                .insert(last.clone(), value);
        }
        None => {
            fn remove(at: &mut Value, path: &[String]) {
                let Some(obj) = at.as_object_mut() else {
                    return;
                };
                match path {
                    [] => {}
                    [last] => {
                        obj.remove(last);
                    }
                    [first, rest @ ..] => {
                        if let Some(child) = obj.get_mut(first) {
                            remove(child, rest);
                            if child.as_object().is_some_and(Map::is_empty) {
                                obj.remove(first);
                            }
                        }
                    }
                }
            }
            remove(root, path);
        }
    }
}

/// `current` with only the leaves that differ between a section entry's `before` and `after`
/// set back to their `before` value (removed when `before` lacked them). Every other key keeps
/// its current value, so undoing one change never touches later changes to other keys.
pub fn revert_changed(current: &Value, before: &Value, after: &Value) -> Value {
    let (mut b, mut a) = (Vec::new(), Vec::new());
    segment_leaves(before, &mut Vec::new(), &mut b);
    segment_leaves(after, &mut Vec::new(), &mut a);
    let paths: BTreeSet<&Vec<String>> = b.iter().chain(a.iter()).map(|(p, _)| p).collect();
    let mut out = current.clone();
    for path in paths {
        let (old, new) = (get_path(before, path), get_path(after, path));
        if old != new {
            put_path(&mut out, path, old.cloned());
        }
    }
    out
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod autostart_tests;
