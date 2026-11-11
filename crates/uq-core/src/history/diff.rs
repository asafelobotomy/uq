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
mod tests {
    use super::*;

    fn labels(d: &str, k: &str) -> Option<String> {
        (d == "look" && k == "gaps_in").then(|| "Gaps between windows".into())
    }

    #[test]
    fn a_mouse_change_is_recorded_once() {
        let old = State::default();
        let mut new = old.clone();
        new.settings.mouse.sensitivity = 0.4;
        let domains: Vec<_> = diff(&old, &new, &labels)
            .into_iter()
            .map(|c| c.domain)
            .collect();
        assert_eq!(domains, ["input"]);
        new.settings.idle.dim = Some(30);
        let domains: Vec<_> = diff(&old, &new, &labels)
            .into_iter()
            .map(|c| c.domain)
            .collect();
        assert_eq!(domains, ["input", "settings"]);
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

    /// Welcome's bookkeeping is not a setting: finishing or resetting Welcome writes no history
    /// entry (so there's nothing to label), and Undo never brings Welcome back.
    #[test]
    fn diff_ignores_welcome() {
        let old = State::default();
        let mut new = old.clone();
        new.welcome.done = true;
        new.welcome.version = crate::welcome::CURRENT_WELCOME_VERSION;
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
    fn revert_changed_touches_only_the_entry_s_leaves() {
        let before = json!({"gaps_in": 5, "rounding": 3});
        let after = json!({"gaps_in": 8, "rounding": 3});
        let current = json!({"gaps_in": 8, "rounding": 6, "blur": true});
        assert_eq!(
            revert_changed(&current, &before, &after),
            json!({"gaps_in": 5, "rounding": 6, "blur": true})
        );
    }

    #[test]
    fn revert_changed_removes_added_leaves_and_empty_parents() {
        // Output names may hold dots; paths are walked by segment, never re-split.
        let before = json!({"outputs": {}});
        let after = json!({"outputs": {"HDMI.A": {"image": "/a.png", "fit": "fill"}}});
        let current = json!({"outputs": {"HDMI.A": {"image": "/a.png", "fit": "fill"}, "DP-1": {"image": "/b.png"}}});
        assert_eq!(
            revert_changed(&current, &before, &after),
            json!({"outputs": {"DP-1": {"image": "/b.png"}}})
        );
        // Keys the entry changed go back even if they changed again since (the conflict
        // prompt covers that); an object left empty goes too, which reads as "nothing set".
        let current = json!({"outputs": {"HDMI.A": {"image": "/c.png", "fit": "fit"}}});
        assert_eq!(revert_changed(&current, &before, &after), json!({}));
    }

    #[test]
    fn revert_changed_restores_removed_leaves() {
        let before = json!({"workspaces": {"2": "master"}, "default": "dwindle"});
        let after = json!({"workspaces": {}, "default": "dwindle"});
        let current = json!({"workspaces": {"3": "scrolling"}, "default": "master"});
        assert_eq!(
            revert_changed(&current, &before, &after),
            json!({"workspaces": {"3": "scrolling", "2": "master"}, "default": "master"})
        );
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
    fn env_json(vars: &[(&str, &str)]) -> Value {
        json!({"vars": vars.iter().map(|(n, v)| json!({"name": n, "value": v})).collect::<Vec<_>>()})
    }

    #[test]
    fn env_summary_names_never_values() {
        let none = env_json(&[]);
        let one = env_json(&[("GDK_BACKEND", "s3cret")]);
        let s = summarize("env", &none, &one, &|_, _| None);
        assert_eq!(s, "Environment variable GDK_BACKEND added");
        let changed = env_json(&[("GDK_BACKEND", "other")]);
        let s = summarize("env", &one, &changed, &|_, _| None);
        assert_eq!(s, "Environment variable GDK_BACKEND changed");
        let s = summarize("env", &changed, &none, &|_, _| None);
        assert_eq!(s, "Environment variable GDK_BACKEND removed");
        let many = env_json(&[
            ("A", "s3cret"),
            ("B", "s3cret"),
            ("C", "s3cret"),
            ("D", "s3cret"),
            ("E", "s3cret"),
        ]);
        let s = summarize("env", &none, &many, &|_, _| None);
        assert_eq!(
            s,
            "Environment variable A added, Environment variable B added, \
             Environment variable C added, and 2 more"
        );
        for line in [&s, &summarize("env", &many, &one, &|_, _| None)] {
            assert!(!line.contains("s3cret"), "{line}");
        }
    }

    #[test]
    fn env_summaries_are_plain() {
        let a = env_json(&[("A", "1"), ("B", "2")]);
        let b = env_json(&[("B", "3"), ("C", "4")]);
        let c = env_json(&[("B", "2"), ("A", "1")]);
        for (x, y) in [(&a, &b), (&b, &a), (&a, &c), (&env_json(&[]), &a)] {
            let s = summarize("env", x, y, &|_, _| None);
            assert!(!s.contains(['{', '[']) && !s.contains("env."), "{s}");
        }
    }

    fn compat_json(sharp: bool, apps: Value) -> Value {
        json!({"sharp_x11": sharp, "electron_wayland": false, "chromium_wayland": false,
            "qt_wayland": false, "steam_scale": false, "apps": apps})
    }

    fn app(id: &str, platform: &str) -> Value {
        json!({"id": id, "platform": platform, "scale": null, "env": [], "args": [],
            "elsewhere": false})
    }

    #[test]
    fn compat_summary() {
        let off = compat_json(false, json!([]));
        let on = compat_json(true, json!([]));
        let s = |b: &Value, a: &Value| summarize("compat", b, a, &|_, _| None);
        assert_eq!(s(&off, &on), "Sharp X11 apps turned on");
        assert_eq!(s(&on, &off), "Sharp X11 apps turned off");
        let mut both = on.clone();
        both["steam_scale"] = json!(true);
        assert_eq!(
            s(&off, &both),
            "Sharp X11 apps turned on, Steam UI scale turned on"
        );
        let code = compat_json(false, json!([app("code.desktop", "x11")]));
        assert_eq!(s(&off, &code), "Fix for code added");
        let changed = compat_json(false, json!([app("code.desktop", "wayland")]));
        assert_eq!(s(&code, &changed), "Fix for code changed");
        assert_eq!(s(&changed, &off), "Fix for code removed");
        let many = compat_json(
            true,
            json!([
                app("a.desktop", "x11"),
                app("b.desktop", "x11"),
                app("c.desktop", "x11")
            ]),
        );
        assert_eq!(
            s(&off, &many),
            "Sharp X11 apps turned on, Fix for a added, Fix for b added, and 1 more"
        );
        let reordered = compat_json(
            true,
            json!([
                app("c.desktop", "x11"),
                app("b.desktop", "x11"),
                app("a.desktop", "x11")
            ]),
        );
        assert_eq!(s(&many, &reordered), "App fixes reordered");
    }

    #[test]
    fn compat_summaries_are_plain() {
        let a = compat_json(
            true,
            json!([app("code.desktop", "x11"), app("x.y.desktop", "auto")]),
        );
        let b = compat_json(false, json!([app("code.desktop", "wayland")]));
        let c = compat_json(false, json!([]));
        let hostile = compat_json(false, json!([app("[x]{y}.desktop", "x11")]));
        for (x, y) in [
            (&a, &b),
            (&b, &a),
            (&c, &a),
            (&a, &c),
            (&c, &c),
            (&c, &hostile),
        ] {
            let s = summarize("compat", x, y, &|_, _| None);
            assert!(!s.is_empty());
            assert!(
                !s.contains(['{', '[']) && !s.contains(".desktop") && !s.contains("compat."),
                "{s}"
            );
        }
    }
}

#[cfg(test)]
mod autostart_tests;
