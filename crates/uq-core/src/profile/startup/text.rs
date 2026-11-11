//! What the Profiles page, "Back to normal", "Create from history" and Explain say about the
//! startup keys: labels, values, notices, the keys of the current state and of a journal entry,
//! and the rows a profile still holds.

use super::read_leaf;
use crate::compat::hypr::SwitchKey;
use crate::history;
use crate::profile::{ActiveProfile, Key, Profile, holds};
use crate::state::State;
use serde_json::{Value, json};
use std::collections::BTreeSet;

const UNKNOWN_STARTUP: &str = "A startup app that isn't on this computer";
const APPS_NOTICE: &str =
    "Apps you start from now on use this profile's settings; apps already open keep theirs.";
const LOGIN_NOTICE: &str = "Startup apps change next time you log in.";

fn s(v: &str) -> String {
    v.to_string()
}

/// An id without its `.desktop`.
fn stem(id: &str) -> &str {
    id.strip_suffix(".desktop").unwrap_or(id)
}

fn quoted(name: &str) -> String {
    format!("\u{201c}{name}\u{201d}")
}

/// The name of a startup entry: `names`, else an added entry's own name, else the file id
/// without `.desktop` when this computer knows it (or when there's no state to ask).
fn entry_name(
    state: Option<&State>,
    id: &str,
    names: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    if let Some(n) = names(id) {
        return Some(n);
    }
    let Some(st) = state else {
        return Some(stem(id).to_string());
    };
    let a = &st.autostart;
    if let Some(e) = a.added.iter().find(|e| e.id() == id) {
        return Some(e.name().to_string());
    }
    (id.ends_with(".desktop") && a.choice(id).is_some()).then(|| stem(id).to_string())
}

fn label_of(
    state: Option<&State>,
    domain: &str,
    path: &[String],
    names: &dyn Fn(&str) -> Option<String>,
) -> String {
    match (domain, path) {
        ("env", [name]) => format!("Environment variable {name}"),
        ("compat", [k]) if SwitchKey::from_key(k).is_some() => {
            s(SwitchKey::from_key(k).map_or("", SwitchKey::label))
        }
        ("compat" | "autostart", [k]) => history::label(domain, k).unwrap_or_else(|| {
            s(match domain {
                "compat" => "Apps and scaling",
                _ => "Apps that start at login",
            })
        }),
        ("compat", [apps, id, f]) if apps == "apps" => {
            let n = quoted(&names(id).unwrap_or_else(|| s(stem(id))));
            match f.as_str() {
                "platform" => format!("Run on for {n}"),
                "scale" => format!("Size for {n}"),
                _ => format!("A setting for {n}"),
            }
        }
        ("autostart", [id, f]) => match entry_name(state, id, names) {
            None => s(UNKNOWN_STARTUP),
            Some(n) if f == "delay" => format!("Start delay for {}", quoted(&n)),
            Some(n) => format!("Start {} at login", quoted(&n)),
        },
        _ => path.join("."),
    }
}

/// "Environment variable FOO"; the switch's label; "Run on for “Code”", "Size for “Code”";
/// "Start “Syncthing” at login", "Start delay for “Syncthing”". `names` gives an app's or an
/// entry's name; without one, the id without `.desktop`. A startup id this computer doesn't
/// know reads "A startup app that isn't on this computer".
pub fn startup_label(
    state: &State,
    domain: &str,
    path: &[String],
    names: &dyn Fn(&str) -> Option<String>,
) -> String {
    label_of(Some(state), domain, path, names)
}

/// [`startup_label`] with no state and no names: ids stand for names.
pub(in crate::profile) fn fallback_label(domain: &str, path: &[String]) -> String {
    label_of(None, domain, path, &|_| None)
}

/// A startup value as a person reads it, when it differs from the generic text.
pub(in crate::profile) fn startup_value_text(
    domain: &str,
    path: &[String],
    v: &Value,
) -> Option<String> {
    let field = path.last().map(String::as_str);
    match (domain, field, v) {
        ("env", _, Value::Bool(false)) => Some(s("Removed")),
        ("compat", Some("platform"), Value::String(p)) if path.len() == 3 => {
            Some(s(match p.as_str() {
                "wayland" => "Wayland",
                "x11" => "X11",
                _ => "Automatic",
            }))
        }
        ("compat", Some("scale"), Value::Number(n)) if path.len() == 3 => n
            .as_f64()
            .map(|f| format!("{} %", (f * 100.0).round() as i64)),
        ("autostart", Some("delay"), Value::Number(n)) => Some(format!("{n} s")),
        _ => None,
    }
}

/// Every startup key as the state holds it: each variable, each switch, each fix's platform
/// (and size when it has one), each choice's and added entry's `enabled` and `delay` when set.
pub fn startup_keys_of(state: &State) -> Vec<Key> {
    let key = |domain: &str, path: Vec<String>, value: Value| Key {
        domain: s(domain),
        path,
        value,
    };
    let mut out: Vec<Key> = state
        .env
        .vars
        .iter()
        .map(|v| key("env", vec![v.name.clone()], json!(v.value)))
        .collect();
    for sw in SwitchKey::ALL {
        out.push(key(
            "compat",
            vec![s(sw.key())],
            json!(state.compat.switch(sw)),
        ));
    }
    let a = &state.autostart;
    let ids = state.compat.apps.iter().map(|f| ("compat", f.id.as_str()));
    let ids = ids.chain(a.files.iter().map(|c| ("autostart", c.id.as_str())));
    let ids = ids.chain(a.added.iter().map(|e| ("autostart", e.id())));
    for (domain, id) in ids {
        let fields: &[&str] = match domain {
            "compat" => &["platform", "scale"],
            _ => &["enabled", "delay"],
        };
        for f in fields {
            let mut path = vec![s(id), s(f)];
            if domain == "compat" {
                path.insert(0, s("apps"));
            }
            if let Some(v) = read_leaf(state, domain, &path) {
                out.push(key(domain, path, v));
            }
        }
    }
    out
}

/// How many startup apps the profile names that this computer doesn't have (`known` holds the
/// found ids and the added ones).
pub fn missing_startup(known: &BTreeSet<String>, profile: &Profile) -> usize {
    let ids: BTreeSet<String> = profile
        .keys()
        .into_iter()
        .filter(|k| k.domain == "autostart")
        .filter_map(|k| k.path.first().cloned())
        .filter(|id| !known.contains(id))
        .collect();
    ids.len()
}

/// The apply notice for `n` startup apps that were left out.
pub fn missing_startup_notice(n: usize) -> String {
    if n == 1 {
        "1 startup app in this profile isn't on this computer, so it was left out.".into()
    } else {
        format!("{n} startup apps in this profile aren't on this computer, so they were left out.")
    }
}

/// When the profile's startup keys take effect, in plain words.
pub fn startup_notice(profile: &Profile) -> Vec<String> {
    let has = |d: &str| profile.values.contains_key(d);
    let mut out = Vec::new();
    if has("env") || has("compat") {
        out.push(s(APPS_NOTICE));
    }
    if has("autostart") {
        out.push(s(LOGIN_NOTICE));
    }
    out
}

fn list<'a>(v: &'a Value, name: &str) -> &'a [Value] {
    v.get(name)
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
}

fn text<'a>(item: &'a Value, key: &str) -> &'a str {
    item.get(key).and_then(Value::as_str).unwrap_or("")
}

fn find<'a>(side: &'a [Value], key: &str, id: &str) -> Option<&'a Value> {
    side.iter().find(|r| text(r, key) == id)
}

/// The ids on both sides, in order, differ.
fn reordered(before: &[Value], after: &[Value], key: &str) -> bool {
    let common = |side: &[Value], other: &[Value]| -> Vec<String> {
        side.iter()
            .map(|r| s(text(r, key)))
            .filter(|id| find(other, key, id).is_some())
            .collect()
    };
    common(before, after) != common(after, before)
}

/// Every id of both sides, before's first.
fn all_ids(before: &[Value], after: &[Value], key: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in before.iter().chain(after) {
        let id = s(text(r, key));
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

fn field<'a>(item: Option<&'a Value>, f: &str) -> Option<&'a Value> {
    item.and_then(|i| i.get(f)).filter(|v| !v.is_null())
}

/// A journal entry's `env`, `compat` or `autostart` change, split for "Create from history":
/// the profile keys it changed, and one tag for each other change (an added, removed or edited
/// entry, a fix's own variables, options or "also elsewhere", an order).
pub fn startup_keys(
    domain: &str,
    before: &Value,
    after: &Value,
) -> (Vec<(String, Vec<String>)>, Vec<String>) {
    let (mut keys, mut other): (Vec<Vec<String>>, Vec<String>) = (Vec::new(), Vec::new());
    let mut tag = |t: String| {
        if !other.contains(&t) {
            other.push(t);
        }
    };
    match domain {
        "env" => {
            let (b, a) = (list(before, "vars"), list(after, "vars"));
            for name in all_ids(b, a, "name") {
                if field(find(b, "name", &name), "value") != field(find(a, "name", &name), "value")
                {
                    keys.push(vec![name]);
                }
            }
            if reordered(b, a, "name") {
                tag(s("order:vars"));
            }
        }
        "compat" => {
            for sw in SwitchKey::ALL {
                if before.get(sw.key()) != after.get(sw.key()) {
                    keys.push(vec![s(sw.key())]);
                }
            }
            let (b, a) = (list(before, "apps"), list(after, "apps"));
            for id in all_ids(b, a, "id") {
                let (fb, fa) = (find(b, "id", &id), find(a, "id", &id));
                let Some(fa) = fa else {
                    tag(format!("apps:{id}"));
                    continue;
                };
                // A platform that isn't there is automatic.
                let norm = |f: &str, v: Option<&Value>| match (f, v) {
                    ("platform", None) => Some(json!("auto")),
                    (_, v) => v.cloned(),
                };
                for f in ["platform", "scale"] {
                    if norm(f, field(fb, f)) != norm(f, field(Some(fa), f)) {
                        keys.push(vec![s("apps"), id.clone(), s(f)]);
                    }
                }
                if ["env", "args", "elsewhere"]
                    .iter()
                    .any(|f| field(fb, f) != field(Some(fa), f))
                {
                    tag(format!("apps:{id}"));
                }
            }
            if reordered(b, a, "id") {
                tag(s("order:apps"));
            }
        }
        "autostart" => {
            let (b, a) = (list(before, "files"), list(after, "files"));
            for id in all_ids(b, a, "id") {
                let (cb, ca) = (find(b, "id", &id), find(a, "id", &id));
                for f in ["enabled", "delay"] {
                    if field(cb, f) != field(ca, f) {
                        keys.push(vec![id.clone(), s(f)]);
                    }
                }
            }
            let (b, a) = (list(before, "added"), list(after, "added"));
            for id in all_ids(b, a, "id") {
                let (Some(eb), Some(ea)) = (find(b, "id", &id), find(a, "id", &id)) else {
                    tag(format!("added:{id}"));
                    continue;
                };
                for f in ["enabled", "delay"] {
                    if field(Some(eb), f) != field(Some(ea), f) {
                        keys.push(vec![id.clone(), s(f)]);
                    }
                }
                if ["name", "app", "argv"]
                    .iter()
                    .any(|f| field(Some(eb), f) != field(Some(ea), f))
                {
                    tag(format!("added:{id}"));
                }
            }
            if reordered(b, a, "id") {
                tag(s("order:added"));
            }
        }
        _ => {}
    }
    let keys = keys.into_iter().map(|p| (s(domain), p)).collect();
    (keys, other)
}

/// `Explain` rows of `env`, `compat` or `autostart` whose value the active profile still holds
/// become `{source: "profile", detail: {name}}`: `vars.<NAME>`, the switch rows, `apps.<id>`
/// (its platform or size), and the startup rows by id. `user_lua` and `error` rows stay.
pub fn mark_startup(rows: &mut Value, domain: &str, state: &State, active: &ActiveProfile) {
    let held = |path: Vec<String>| {
        read_leaf(state, domain, &path).is_some_and(|v| holds(active, domain, &path, &v))
    };
    let mut marked: Vec<String> = Vec::new();
    match domain {
        "env" => {
            for v in &state.env.vars {
                if held(vec![v.name.clone()]) {
                    marked.push(format!("vars.{}", v.name));
                }
            }
        }
        "compat" => {
            for sw in SwitchKey::ALL {
                if held(vec![s(sw.key())]) {
                    marked.push(s(sw.key()));
                }
            }
            for fix in &state.compat.apps {
                let path = |f: &str| vec![s("apps"), fix.id.clone(), s(f)];
                if held(path("platform")) || held(path("scale")) {
                    marked.push(format!("apps.{}", fix.id));
                }
            }
        }
        "autostart" => {
            let a = &state.autostart;
            let files = a.files.iter().map(|c| c.id.as_str());
            for id in files.chain(a.added.iter().map(|e| e.id())) {
                if held(vec![s(id), s("enabled")]) || held(vec![s(id), s("delay")]) {
                    marked.push(s(id));
                }
            }
        }
        _ => {}
    }
    for key in marked {
        if let Some(row) = rows.get_mut(&key)
            && row["source"] == "uq"
        {
            row["source"] = json!("profile");
            row["detail"] = json!({ "name": active.name });
        }
    }
}
