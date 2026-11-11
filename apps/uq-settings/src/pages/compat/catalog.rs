//! What the fix editor offers, built from uq-core's tables so the page never copies them
//! (slice C's F6), and the preview of a draft fix (F31).

use serde_json::{Value, json};
use uq_core::compat::hypr::{PLATFORM_ENV, SwitchKey};
use uq_core::compat::launch::argv;
use uq_core::compat::{Compat, MAX_APP_ENV, MAX_APPS, MAX_ARG_CHARS, MAX_ARGS, valid_scale};
use uq_core::desktop_entry::normalise_id;

use crate::pages::startup::edit::reason;

/// The plain label of a "Run on" choice.
fn platform_label(key: &str) -> &'static str {
    match key {
        "wayland" => "Wayland",
        "x11" => "X11",
        _ => "Automatic",
    }
}

/// `{switches: [{key, label, description}], platforms: [{key, label}], scales: [1.0 … 3.0],
/// limits}`.
pub fn fields() -> Value {
    let switches: Vec<Value> = SwitchKey::ALL
        .iter()
        .map(|k| json!({"key": k.key(), "label": k.label(), "description": k.description()}))
        .collect();
    let platforms: Vec<Value> = std::iter::once("auto")
        .chain(PLATFORM_ENV.iter().map(|(p, _)| *p))
        .map(|p| json!({"key": p, "label": platform_label(p)}))
        .collect();
    let scales: Vec<f64> = (4..=12)
        .map(|q| f64::from(q) / 4.0)
        .filter(|s| valid_scale(*s))
        .collect();
    json!({
        "switches": switches,
        "platforms": platforms,
        "scales": scales,
        "limits": {"apps": MAX_APPS, "env": MAX_APP_ENV, "args": MAX_ARGS,
            "arg_chars": MAX_ARG_CHARS},
    })
}

/// What a draft fix would add to an app's command, on the stand-in command `app`:
/// `{env: [names], args: [..], problem: text|null}`. A draft uQ would refuse names the reason
/// only. Pure.
pub fn preview(fix: &Value) -> Value {
    let mut draft = fix.clone();
    if let Some(o) = draft.as_object_mut() {
        let id = o.get("id").and_then(Value::as_str).unwrap_or("app");
        let id = normalise_id(id);
        o.insert("id".into(), json!(id));
    }
    let problem = |text: String| json!({"env": [], "args": [], "problem": text});
    let parsed = match Compat::from_section(&json!({ "apps": [draft] })) {
        Ok(c) => c,
        Err(e) => return problem(reason(&e)),
    };
    let Some(fix) = parsed.apps.first() else {
        return problem("This fix doesn't change anything.".into());
    };
    let command = match argv(Some(fix), &fix.id, vec!["app".into()]) {
        Ok(c) => c,
        Err(e) => return problem(e.to_string()),
    };
    let rest = match command.as_slice() {
        [env, dashes, rest @ ..] if env == "env" && dashes == "--" => rest,
        all => all,
    };
    let assignments = rest.iter().take_while(|a| a.contains('=')).count();
    let mut names: Vec<&str> = Vec::new();
    for a in &rest[..assignments] {
        let name = a.split_once('=').map_or(a.as_str(), |(n, _)| n);
        if !names.contains(&name) {
            names.push(name);
        }
    }
    // After the assignments: the stand-in program, then what the fix adds.
    let args = &rest[(assignments + 1).min(rest.len())..];
    json!({"env": names, "args": args, "problem": null})
}
