//! Per-domain diff of two states, with a one-line human summary of each change.

use crate::settings::{SECTION_DOMAINS, SETTING_KEYS};
use crate::state::State;
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

/// Looks up the display label for a setting: `(domain, json key)`.
pub type Labels<'a> = &'a dyn Fn(&str, &str) -> Option<&'static str>;

#[derive(Debug, Clone, PartialEq)]
pub struct DomainChange {
    pub domain: String,
    pub before: Value,
    pub after: Value,
    pub summary: String,
}

const MAX_NAMED: usize = 3;

/// Every domain whose content differs between `old` and `new`. `launch_counts` is not compared.
pub fn diff(old: &State, new: &State, labels: Labels) -> Vec<DomainChange> {
    let mut domains: Vec<&str> = SECTION_DOMAINS.to_vec();
    if !domains.contains(&"layouts") {
        domains.push("layouts");
    }
    domains.extend(["theme", "schedule", "settings"]);
    domains
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

fn domain_value(st: &State, domain: &str) -> Value {
    match domain {
        "theme" => json!({"theme": st.theme, "variant": st.variant}),
        "schedule" => serde_json::to_value(&st.schedule).expect("schedule serialises"),
        "settings" => {
            let all = serde_json::to_value(&st.settings).expect("settings serialise");
            let mut m = Map::new();
            for key in SETTING_KEYS {
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
                let name = labels(domain, k).unwrap_or(k.as_str());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(d: &str, k: &str) -> Option<&'static str> {
        (d == "look" && k == "gaps_in").then_some("Gaps between windows")
    }

    #[test]
    fn diff_reports_only_changed_domains() {
        let old = State::default();
        let mut new = old.clone();
        new.settings.look.gaps_in = Some(8);
        let mut old = old;
        old.settings.look.gaps_in = Some(5);
        let changes = diff(&old, &new, &labels);
        assert_eq!(changes.len(), 1, "{changes:?}");
        assert_eq!(changes[0].domain, "look");
        assert_eq!(changes[0].summary, "Gaps between windows: 5 → 8");
        assert_eq!(changes[0].before["gaps_in"], 5);
        assert_eq!(changes[0].after["gaps_in"], 8);
    }

    #[test]
    fn diff_ignores_launch_counts() {
        let old = State::default();
        let mut new = old.clone();
        new.launch_counts.insert("firefox.desktop".into(), 4);
        assert!(diff(&old, &new, &|_, _| None).is_empty());
    }

    #[test]
    fn diff_theme_pseudo_domain() {
        let old = State::default();
        let mut new = old.clone();
        new.theme = "rosewood".into();
        let changes = diff(&old, &new, &|_, _| None);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].domain, "theme");
        assert_eq!(changes[0].summary, "theme: graphite → rosewood");
        assert_eq!(changes[0].after["theme"], "rosewood");
    }

    #[test]
    fn diff_settings_and_layouts_domains() {
        let old = State::default();
        let mut new = old.clone();
        new.settings.pinned = vec!["a.desktop".into()];
        new.layouts
            .insert("2".into(), crate::lua::Layout::Scrolling);
        let names: Vec<_> = diff(&old, &new, &|_, _| None)
            .into_iter()
            .map(|c| c.domain)
            .collect();
        assert_eq!(names, ["layouts", "settings"]);
    }

    #[test]
    fn summary_caps_at_three_leaves() {
        let before = json!({});
        let after =
            json!({"gaps_in": 1, "gaps_out": 2, "rounding": 3, "blur": true, "shadow": false});
        assert_eq!(
            summarize("look", &before, &after, &|_, _| None),
            "blur: default → true, gaps_in: default → 1, gaps_out: default → 2, and 2 more"
        );
    }
}
