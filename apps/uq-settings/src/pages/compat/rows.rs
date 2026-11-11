//! The snapshot's rows: the switches with where each comes from, the browsers' flag files, the
//! "Blurry right now" windows and each app fix.

use serde_json::{Value, json};
use uq_core::compat::hypr::SwitchKey;
use uq_core::rules::WindowInfo;

use super::{BLURRY, NOT_SCALED, SHARP_ON, app_for_class};
use crate::desktop_entries::AppEntry;
use crate::pages::startup::rows::origin;

/// The five switches, each `{key, label, description, on, origin}`.
pub(super) fn switches(section: &Value, explained: &Value) -> Vec<Value> {
    SwitchKey::ALL
        .iter()
        .map(|k| {
            json!({"key": k.key(), "label": k.label(), "description": k.description(),
                "on": section[k.key()].as_bool().unwrap_or(false),
                "origin": origin(explained, k.key())})
        })
        .collect()
}

/// Each installed browser's flags file, from the `chromium_wayland` row's detail:
/// `{name, file, message}`, `file` being `ours`, `own_file`, `none` or `error`.
pub(super) fn browsers(explained: &Value) -> Vec<Value> {
    let list = explained["chromium_wayland"]["detail"]["browsers"].as_array();
    list.map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|b| {
            let name = b["name"].as_str().unwrap_or_default();
            let message = match b["file"].as_str() {
                Some("own_file") => json!(format!(
                    "{name} has its own flags file, so uQ left it alone."
                )),
                Some("error") => b["message"].clone(),
                _ => Value::Null,
            };
            json!({"name": name, "file": b["file"], "message": message})
        })
        .collect()
}

/// The heading over the window list.
pub(super) fn heading(scaled: bool, sharp_on: bool) -> &'static str {
    if !scaled {
        NOT_SCALED
    } else if sharp_on {
        SHARP_ON
    } else {
        BLURRY
    }
}

/// A window's class, or its first class when it has none now.
pub(super) fn class_of(w: &WindowInfo) -> &str {
    if w.class.is_empty() {
        &w.initial_class
    } else {
        &w.class
    }
}

/// The windows that run under XWayland, one per class, each with the app it belongs to.
pub(super) fn blurry_windows(
    windows: &[WindowInfo],
    apps: &[AppEntry],
    has_fix: impl Fn(&str) -> bool,
) -> Vec<Value> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for w in windows.iter().filter(|w| w.xwayland) {
        let class = class_of(w);
        if seen.contains(&class) {
            continue;
        }
        seen.push(class);
        let app = app_for_class(class, apps);
        out.push(json!({
            "title": w.title, "class": class,
            "app_id": app.map(|a| &a.id), "app_name": app.map(|a| &a.name),
            "has_fix": app.is_some_and(|a| has_fix(&a.id)),
        }));
    }
    out
}

/// Each fix with its app's name and icon, and what uqd's file sync did about it.
pub(super) fn fixes(section: &Value, explained: &Value, apps: &[AppEntry]) -> Vec<Value> {
    let list = section["apps"].as_array().map_or(&[][..], Vec::as_slice);
    list.iter()
        .map(|f| {
            let id = f["id"].as_str().unwrap_or_default();
            let app = apps.iter().find(|a| a.id == id);
            let name = app.map_or_else(
                || id.strip_suffix(".desktop").unwrap_or(id).to_string(),
                |a| a.name.clone(),
            );
            let origin = origin(explained, &format!("apps.{id}"));
            let detail = &origin["detail"];
            json!({
                "id": id, "name": name, "icon": app.map_or("", |a| a.icon.as_str()),
                "platform": f["platform"], "scale": f["scale"], "env": f["env"],
                "args": f["args"], "elsewhere": f["elsewhere"],
                "override": detail["override"], "message": detail["message"],
                "origin": origin,
            })
        })
        .collect()
}
