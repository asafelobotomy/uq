//! The Input page's touchpad gestures and swipe options: what the page asks for ([`apply`]) and
//! what it shows about them ([`snapshot_parts`]).
//!
//! Only the gesture ops write uQ's defaults out as a list (turning a default off has to say
//! which one); every other write to the section, `SetSwipe` included, keeps `gestures` as it is,
//! `null` too, so a later change to uQ's defaults still reaches someone who never edited them.
//! Every write is checked here with `InputSettings::from_section` first, so a refusal reads in
//! plain words; uqd checks again.
#![cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 14"))]

use serde_json::{Map, Value, json};
use uq_core::client::{self, DesktopProxy};
use uq_core::gestures::{self, Gesture, hypr};
use uq_core::keybinds::conflict::message;
use uq_core::settings::InputSettings;

use super::Op;
use crate::pages::runner::{ORIGIN, message as failure_text, parse_json, plain_refusal};

const GONE: &str = "That gesture isn't there any more.";

fn refuse<T>(text: impl Into<String>) -> zbus::Result<T> {
    Err(zbus::Error::Failure(text.into()))
}

/// The gestures the section holds, `None` when it holds none of its own (uQ's defaults).
/// A list with an entry this app can't read keeps the entries it can.
pub fn stored(section: &Value) -> Option<Vec<Gesture>> {
    section.get("gestures")?.as_array().map(|items| {
        items
            .iter()
            .filter_map(|g| serde_json::from_value(g.clone()).ok())
            .collect()
    })
}

/// The gestures in force: the stored list, or uQ's defaults.
pub fn effective(section: &Value) -> Vec<Gesture> {
    stored(section).unwrap_or_else(gestures::defaults)
}

/// The editor's live warning: the plain text of the clash `draft` would make when it is saved
/// as gesture `index` (`-1`, or any index past the list, adds it), or `None`. A gesture that
/// is off clashes with nothing.
pub fn draft_clash(list: &[Gesture], index: i32, draft: &Gesture) -> Option<String> {
    if !draft.enabled {
        return None;
    }
    let mut next = list.to_vec();
    let at = match usize::try_from(index) {
        Ok(i) if i < next.len() => {
            next[i] = draft.clone();
            i
        }
        _ => {
            next.push(draft.clone());
            next.len() - 1
        }
    };
    let at = at.to_string();
    gestures::clashes_of(&next)
        .into_iter()
        .find(|c| c.first == at || c.second == at)
        .map(|c| message(&c))
}

/// A refusal from uqd without the key it names; a missing service stays as it is.
fn plain_error(e: zbus::Error) -> zbus::Error {
    if client::no_owner(&e) {
        e
    } else {
        zbus::Error::Failure(plain_refusal(&failure_text(&e)))
    }
}

fn put(section: &mut Map<String, Value>, list: &[Gesture]) {
    section.insert(
        "gestures".into(),
        serde_json::to_value(list).expect("gestures serialise"),
    );
}

/// The checked write of `section`.
fn check(section: &Map<String, Value>) -> zbus::Result<()> {
    match InputSettings::from_section(&Value::Object(section.clone())) {
        Ok(_) => Ok(()),
        Err(e) => refuse(plain_refusal(&e.to_string())),
    }
}

fn edit(section: &mut Map<String, Value>, op: Op) -> zbus::Result<()> {
    let current = effective(&Value::Object(section.clone()));
    match op {
        Op::SaveGesture { index, gesture } => {
            let mut list = current;
            let at = usize::try_from(index).unwrap_or(list.len());
            let g = Gesture::from_value(&format!("input.gestures[{at}]"), &gesture)
                .map_err(|e| zbus::Error::Failure(plain_refusal(&e.to_string())))?;
            if index < 0 {
                list.push(g);
            } else if at < list.len() {
                list[at] = g;
            } else {
                return refuse(GONE);
            }
            put(section, &list);
        }
        Op::RemoveGesture { index } => {
            let mut list = current;
            if index >= list.len() {
                return refuse(GONE);
            }
            list.remove(index);
            put(section, &list);
        }
        Op::ToggleGesture { index, enabled } => {
            let mut list = current;
            let Some(g) = list.get_mut(index) else {
                return refuse(GONE);
            };
            g.enabled = enabled;
            put(section, &list);
        }
        Op::ResetGestures => {
            section.insert("gestures".into(), Value::Null);
        }
        Op::SetSwipe { field, value } => {
            if !hypr::SWIPE_OPTIONS.iter().any(|(name, _)| *name == field) {
                return refuse(format!("unknown field {field:?}"));
            }
            let swipe = section
                .entry("swipe")
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .ok_or_else(|| zbus::Error::Failure("the swipe options aren't an object".into()))?;
            if value.is_null() {
                swipe.remove(&field);
            } else {
                swipe.insert(field, value);
            }
        }
        _ => {}
    }
    check(section)
}

/// Do `op`, one of the ops this file owns, as one `SetSection("input", …)`.
pub(super) async fn apply(proxy: &DesktopProxy<'_>, op: Op) -> zbus::Result<()> {
    let mut section = parse_json(&proxy.get_section("input").await?)?;
    let Some(object) = section.as_object_mut() else {
        return refuse("the input section isn't an object");
    };
    edit(object, op)?;
    proxy
        .set_section("input", &section.to_string(), ORIGIN)
        .await
        .map_err(plain_error)
}

/// "swipe sideways" as "Swipe sideways".
fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn id_of<T: serde::Serialize>(v: T) -> Value {
    serde_json::to_value(v).expect("names serialise")
}

/// What the editor offers: fingers, directions, actions (each with the directions it allows)
/// and modifiers, from the one name table in `uq_core::gestures::hypr`.
pub fn choices() -> Value {
    let directions: Vec<Value> = hypr::DIRECTIONS
        .iter()
        .map(|d| json!({"id": id_of(d.uq), "label": sentence(d.label)}))
        .collect();
    let actions: Vec<Value> = hypr::ACTIONS
        .iter()
        .map(|a| {
            let allowed: Vec<Value> = a.allowed.iter().map(id_of).collect();
            json!({"id": id_of(a.uq), "label": a.label, "directions": allowed})
        })
        .collect();
    json!({
        "fingers": [3, 4, 5],
        "directions": directions,
        "actions": actions,
        "modifiers": [
            {"id": "", "label": "None"},
            {"id": "SUPER", "label": "Super"},
            {"id": "ALT", "label": "Alt"},
            {"id": "CTRL", "label": "Ctrl"},
            {"id": "SHIFT", "label": "Shift"},
        ],
    })
}

/// The page's gestures part of the snapshot. `input` is the section; `explained` is
/// `Explain("input")`, whose `gestures` row says whether `user.lua` has a gesture of its own.
///
/// `gestures_default` is true while the section holds no list (uQ's two gestures are shown);
/// the plain name `defaults` is the page's other defaults.
pub fn snapshot_parts(input: &Value, explained: Option<&Value>) -> Map<String, Value> {
    let list = effective(input);
    let rows: Vec<Value> = list
        .iter()
        .map(|g| {
            json!({
                "label": g.describe(),
                "action_label": g.action_label(),
                "enabled": g.enabled,
                "gesture": g,
            })
        })
        .collect();
    let clashes: Map<String, Value> = gestures::clashes_of(&list)
        .into_iter()
        .map(|c| (c.second.clone(), Value::String(message(&c))))
        .collect();
    let row = explained.and_then(|e| e.get("gestures"));
    let user_lua = row
        .filter(|r| r["source"] == "user_lua")
        .map(|r| json!({"line": r["detail"]["line"], "path": r["detail"]["path"]}));
    let mut out = Map::new();
    out.insert("gestures".into(), Value::Array(rows));
    out.insert("gestures_default".into(), json!(stored(input).is_none()));
    out.insert("clashes".into(), Value::Object(clashes));
    out.insert("choices".into(), choices());
    out.insert(
        "swipe".into(),
        input.get("swipe").cloned().unwrap_or_else(|| json!({})),
    );
    out.insert("user_lua".into(), user_lua.unwrap_or(Value::Null));
    out
}

#[cfg(test)]
#[path = "gestures_tests.rs"]
mod tests;
