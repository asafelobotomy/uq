//! `ListBinds`: the merged view the Keyboard shortcuts page shows. Pure, like `explain`: uqd
//! reads `user.lua`; the app's fake uqd calls this too. The key and action catalogues come from
//! the tables, so QML never copies them.

use super::actions::catalogue;
use super::combo::{self, Combo};
use super::defaults::{self, Group, LOCK_RECOVERY};
use super::keys::{Group as KeyGroup, KEYS, KeyKind};
use super::{Keybinds, MAX_CUSTOM, MAX_NAME_CHARS, actions};
use crate::settings::provenance::marked_binds;
use serde_json::{Value, json};

const GROUPS: [Group; 6] = [
    Group::AppsAndDesktop,
    Group::Windows,
    Group::Workspaces,
    Group::SoundAndBrightness,
    Group::Media,
    Group::Profiles,
];

/// The group id of your own shortcuts.
pub const CUSTOM_GROUP: &str = "custom";

fn canon(c: Option<Combo>) -> Value {
    c.map_or(Value::Null, |c| Value::String(combo::canonical(c)))
}

fn words(c: Option<Combo>) -> Value {
    c.map_or(Value::Null, |c| Value::String(combo::text(c)))
}

fn kind_id(k: KeyKind) -> &'static str {
    match k {
        KeyKind::Key => "key",
        KeyKind::Wheel => "wheel",
        KeyKind::Button => "button",
        KeyKind::ModKey => "mod_key",
    }
}

fn key_group_id(g: KeyGroup) -> &'static str {
    match g {
        KeyGroup::Letter => "letter",
        KeyGroup::Digit => "digit",
        KeyGroup::Function => "function",
        KeyGroup::Navigation => "navigation",
        KeyGroup::Editing => "editing",
        KeyGroup::Punctuation => "punctuation",
        KeyGroup::Media => "media",
        KeyGroup::Mouse => "mouse",
    }
}

/// The view, in this order: uQ's defaults (with overrides applied, in table order), then your own
/// shortcuts, then the `user.lua` binds marked `-- uq:bind`.
///
/// `binds[i].is_default` is "exactly as uQ ships it": false once changed, and for your own.
/// `default_combo` is null for your own and for a default with no keys.
pub fn list(kb: &Keybinds, user_lua: Option<&str>) -> Value {
    let mut binds = Vec::new();
    for e in kb.effective() {
        let d = defaults::default_of(&e.id);
        let own = d.and_then(|d| d.combo).and_then(|t| combo::parse(t).ok());
        let c = kb.custom.iter().find(|b| b.id == e.id);
        let changed = kb.overrides.iter().any(|o| o.id == e.id);
        binds.push(json!({
            "id": e.id,
            "label": e.label,
            "group": d.map_or(CUSTOM_GROUP, |d| d.group.id()),
            "combo": canon(e.combo),
            "combo_text": words(e.combo),
            "default_combo": canon(own),
            "default_combo_text": words(own),
            "enabled": e.enabled,
            "is_default": d.is_some() && !changed,
            "custom": c.is_some(),
            "can_disable": e.id != LOCK_RECOVERY,
            "action_text": c.map_or_else(|| e.label.clone(), |b| b.action.text()),
            "locked": e.locked,
            "repeating": e.repeating,
            "name": c.map_or(Value::Null, |b| Value::String(b.name.clone())),
        }));
    }
    let marked: Vec<Value> = user_lua
        .map(marked_binds)
        .unwrap_or_default()
        .into_iter()
        .map(|m| {
            let c = m.combo.as_deref().and_then(|t| combo::parse(t).ok());
            json!({ "line": m.line, "combo": m.combo, "combo_text": words(c), "text": m.text })
        })
        .collect();
    let mut groups: Vec<Value> = GROUPS
        .iter()
        .map(|g| json!({ "id": g.id(), "label": g.label() }))
        .collect();
    groups.push(json!({ "id": CUSTOM_GROUP, "label": "Your own shortcuts" }));
    let keys: Vec<Value> = KEYS
        .iter()
        .map(|k| {
            json!({ "spelling": k.spelling, "label": k.label,
                    "kind": kind_id(k.kind), "group": key_group_id(k.group) })
        })
        .collect();
    let mut acts = json!({ "shell": [], "window": [], "workspace": [], "uq": [] });
    for (group, a) in catalogue() {
        acts[group]
            .as_array_mut()
            .expect("the four groups are lists")
            .push(json!({ "label": a.text(), "action": a,
                         "may_lock": a.may_lock(), "may_repeat": a.may_repeat() }));
    }
    json!({
        "binds": binds,
        "user_lua": marked,
        "groups": groups,
        "keys": keys,
        "actions": acts,
        "limits": { "custom": MAX_CUSTOM, "name": MAX_NAME_CHARS,
                    "command": actions::MAX_COMMAND_CHARS },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb(v: Value) -> Keybinds {
        Keybinds::from_section(&v, &Keybinds::default()).unwrap()
    }

    fn row<'a>(v: &'a Value, id: &str) -> &'a Value {
        v["binds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["id"] == id)
            .unwrap()
    }

    #[test]
    fn lists_defaults_overrides_and_custom_in_order() {
        let k = kb(json!({
            "overrides": [{"id": "uq.spotlight", "combo": "SUPER + K"},
                          {"id": "uq.lock-recovery", "combo": "SUPER + CTRL + R"},
                          {"id": "uq.close", "enabled": false}],
            "custom": [{"name": "Screenshot", "combo": "SUPER + SHIFT + P",
                "action": {"kind": "command", "command": "grim shot.png"}}]
        }));
        let v = list(&k, None);
        let b = v["binds"].as_array().unwrap();
        assert_eq!(b.len(), 56);
        assert_eq!(b[0]["id"], "uq.terminal");
        assert_eq!(b[55]["name"], "Screenshot");
        let s = row(&v, "uq.spotlight");
        assert_eq!(s["combo"], "SUPER + K");
        assert_eq!(s["combo_text"], "Super + K");
        assert_eq!(s["default_combo"], "SUPER + Space");
        assert_eq!(s["default_combo_text"], "Super + Space");
        assert_eq!(s["is_default"], false);
        assert_eq!(s["can_disable"], true);
        assert_eq!(s["group"], "apps_and_desktop");
        assert_eq!(row(&v, "uq.close")["enabled"], false);
        assert_eq!(row(&v, "uq.float")["is_default"], true);
        assert_eq!(row(&v, "uq.lock-recovery")["can_disable"], false);
        assert_eq!(row(&v, "uq.lock-recovery")["locked"], true);
        let c = &b[55];
        assert_eq!(c["group"], "custom");
        assert_eq!(c["custom"], true);
        assert_eq!(c["default_combo"], Value::Null);
        assert_eq!(c["action_text"], "Runs a command");
        assert!(
            !c.to_string().contains("grim"),
            "a command's text isn't listed"
        );
        assert!(c["id"].as_str().unwrap().starts_with("k-"));
    }

    #[test]
    fn marked_user_lua_binds_are_read_only_rows() {
        let lua = "hl.bind(\"SUPER + T\", d) -- uq:bind Terminal two\nhl.bind(x .. \"Q\", d) -- uq:bind\n";
        let v = list(&Keybinds::default(), Some(lua));
        let u = v["user_lua"].as_array().unwrap();
        assert_eq!(u.len(), 2);
        assert_eq!(
            u[0],
            json!({"line": 1, "combo": "SUPER + T", "combo_text": "Super + T",
                   "text": "Terminal two"})
        );
        assert_eq!(u[1]["combo"], Value::Null);
        assert_eq!(u[1]["combo_text"], Value::Null);
        assert_eq!(v["binds"].as_array().unwrap().len(), 55);
    }

    #[test]
    fn catalogues_come_from_the_tables() {
        let v = list(&Keybinds::default(), None);
        assert_eq!(v["keys"].as_array().unwrap().len(), KEYS.len());
        assert_eq!(
            v["keys"][0],
            json!({"spelling": "A", "label": "A", "kind": "key",
                                        "group": "letter"})
        );
        assert_eq!(v["actions"]["shell"].as_array().unwrap().len(), 12);
        let uq = v["actions"]["uq"].as_array().unwrap();
        assert!(
            uq.iter()
                .any(|a| a["action"] == json!({"kind": "uq", "name": "settings"}))
        );
        assert_eq!(v["actions"]["window"][0]["label"], "Closes the window");
        // What the editor offers as switches comes from the same limits the write checks (R7).
        let find = |group: &str, name: &str| {
            v["actions"][group]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["action"]["name"] == name)
                .unwrap()
                .clone()
        };
        assert_eq!(find("shell", "vol-up")["may_lock"], true);
        assert_eq!(find("shell", "vol-up")["may_repeat"], true);
        assert_eq!(find("shell", "spotlight")["may_lock"], false);
        assert_eq!(find("uq", "terminal")["may_repeat"], false);
        assert_eq!(find("uq", "media")["may_lock"], true);
        assert_eq!(
            v["limits"],
            json!({"custom": 100, "name": 100, "command": 1024})
        );
        let ids: Vec<&str> = v["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids.last(), Some(&"custom"));
        assert_eq!(ids.len(), 7);
        // Every default's group is a listed group.
        for b in v["binds"].as_array().unwrap() {
            assert!(ids.contains(&b["group"].as_str().unwrap()), "{b}");
        }
        // No raw key spelling leaks into a label.
        for k in v["keys"].as_array().unwrap() {
            let l = k["label"].as_str().unwrap();
            assert!(
                !l.contains("XF86") && !l.contains("mouse_") && !l.contains("mouse:"),
                "{l}"
            );
        }
    }

    #[test]
    fn profile_next_listed_unbound() {
        let v = list(&Keybinds::default(), None);
        let r = row(&v, "uq.profile.next");
        assert_eq!(r["combo"], Value::Null);
        assert_eq!(r["combo_text"], Value::Null);
        assert_eq!(r["default_combo"], Value::Null);
        assert_eq!(r["enabled"], true);
        assert_eq!(r["is_default"], true);
        assert_eq!(r["group"], "profiles");
    }
}
