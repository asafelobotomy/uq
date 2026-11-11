//! `Explain("keybinds")`: where each shortcut's live state comes from. Pure: uqd reads
//! `j/binds`, `user.lua` and the last rejection and calls this; the app's fake uqd calls it too.

use super::combo::{self, Combo};
use super::hypr::{DESCRIPTION_PREFIX, MODIFIERS};
use super::{Keybinds, keys};
use crate::settings::provenance::find_bind_in_user_lua;
use serde_json::{Map, Value, json};

/// How many entries of `hyprctl binds -j` are read.
const MAX_LIVE: usize = 2000;

/// One bind Hyprland has: its keys (when uQ knows them) and the `description` it was given.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveBind {
    pub combo: Option<Combo>,
    pub description: String,
}

/// Parses `hyprctl binds -j`: `[{modmask, key, description, …}]`. `modmask` is Hyprland's own
/// bit set ([`MODIFIERS`]); any other bit, or a key uQ has no name for, gives `combo: None`.
/// Anything that isn't a list gives none; at most 2000 entries are read.
pub fn live_binds(json: &str) -> Vec<LiveBind> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let known: u64 = MODIFIERS.iter().fold(0, |a, m| a | u64::from(m.1));
    items
        .iter()
        .take(MAX_LIVE)
        .map(|b| {
            let mask = b.get("modmask").and_then(Value::as_u64).unwrap_or(0);
            let key = b.get("key").and_then(Value::as_str).unwrap_or("");
            let combo = (mask & !known == 0)
                .then(|| keys::find(key))
                .flatten()
                .map(|(i, _)| Combo {
                    mods: mask as u8,
                    key: i as u16,
                });
            let description = b
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .filter(|c| !c.is_control())
                .take(200)
                .collect();
            LiveBind { combo, description }
        })
        .collect()
}

/// One row per shortcut, keyed `binds.<id>`, each `{source, live, stored, detail}`; `stored` is
/// the shortcut's keys now (canonical), or null when it has none or is off. `source`:
/// - `error`: the last write was rejected (on every row, and on a `"keybinds"` row, so a write
///   that added the only shortcut still says so);
/// - `default`: an untouched default; `uq`: changed or your own;
/// - `user_lua` (`{line, path}`): ours isn't live and a line of `user.lua` explains it;
/// - `runtime`: ours isn't live and nothing in `user.lua` does.
///
/// Whether ours is live is whether a bind with our `description` (`uq:<id>`) has these keys. With
/// `live: None` (Hyprland is down), or when no live bind carries any `uq:` description (a
/// Hyprland that doesn't print it), every row reports from state.
pub fn explain(
    kb: &Keybinds,
    live: Option<&[LiveBind]>,
    user_lua: Option<&str>,
    rejected: Option<&str>,
    user_lua_path: &str,
) -> Value {
    let row = |stored: Option<Combo>, source: &str, detail: Value| -> Value {
        json!({ "source": source, "live": null,
                "stored": stored.map(combo::canonical), "detail": detail })
    };
    let ours = live.filter(|l| {
        l.iter()
            .any(|b| b.description.starts_with(DESCRIPTION_PREFIX))
    });
    let mut out = Map::new();
    if let Some(m) = rejected {
        out.insert(
            "keybinds".into(),
            row(None, "error", json!({ "message": m })),
        );
    }
    for e in kb.effective() {
        let stored = e.combo.filter(|_| e.enabled);
        let touched = !e.is_default
            || kb
                .overrides
                .iter()
                .any(|o| o.id == e.id && (o.combo.is_some() || o.enabled.is_some()));
        let from_state = if touched { "uq" } else { "default" };
        let r = if let Some(m) = rejected {
            row(stored, "error", json!({ "message": m }))
        } else if let (Some(live), Some(c)) = (ours, stored) {
            let tag = format!("{DESCRIPTION_PREFIX}{}", e.id);
            if live
                .iter()
                .any(|b| b.description == tag && b.combo == Some(c))
            {
                row(stored, from_state, json!({}))
            } else if let Some(line) = user_lua.and_then(|t| find_bind_in_user_lua(t, c)) {
                row(
                    stored,
                    "user_lua",
                    json!({ "line": line, "path": user_lua_path }),
                )
            } else {
                row(stored, "runtime", json!({}))
            }
        } else {
            row(stored, from_state, json!({}))
        };
        out.insert(format!("binds.{}", e.id), r);
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live_of(kb: &Keybinds) -> Vec<LiveBind> {
        kb.effective()
            .into_iter()
            .filter(|e| e.enabled)
            .filter_map(|e| {
                Some(LiveBind {
                    combo: Some(e.combo?),
                    description: format!("uq:{}", e.id),
                })
            })
            .collect()
    }

    fn kb(v: Value) -> Keybinds {
        Keybinds::from_section(&v, &Keybinds::default()).unwrap()
    }

    #[test]
    fn default_row_is_default() {
        let k = Keybinds::default();
        let v = explain(&k, Some(&live_of(&k)), None, None, "/u.lua");
        assert_eq!(v["binds.uq.spotlight"]["source"], "default");
        assert_eq!(v["binds.uq.spotlight"]["stored"], "SUPER + Space");
        assert_eq!(v["binds.uq.spotlight"]["live"], Value::Null);
        assert_eq!(v["binds.uq.profile.next"]["stored"], Value::Null);
        assert_eq!(v.as_object().unwrap().len(), 56);
    }

    #[test]
    fn override_live_is_uq() {
        let k = kb(
            json!({"overrides": [{"id": "uq.spotlight", "combo": "SUPER + K"}],
            "custom": [{"name": "T", "combo": "SUPER + F1",
                "action": {"kind": "uq", "name": "terminal"}}]}),
        );
        let id = k.custom[0].id.clone();
        let v = explain(&k, Some(&live_of(&k)), None, None, "/u.lua");
        assert_eq!(v["binds.uq.spotlight"]["source"], "uq");
        assert_eq!(v["binds.uq.spotlight"]["stored"], "SUPER + K");
        assert_eq!(v[format!("binds.{id}")]["source"], "uq");
        assert_eq!(v["binds.uq.close"]["source"], "default");
    }

    #[test]
    fn missing_live_with_user_lua_unbind_is_user_lua() {
        let k = Keybinds::default();
        let live: Vec<LiveBind> = live_of(&k)
            .into_iter()
            .filter(|b| b.description != "uq:uq.close")
            .collect();
        let lua = "-- mine\nlocal x = 1\nhl.unbind(\"WIN + q\")\n";
        let v = explain(&k, Some(&live), Some(lua), None, "/home/u/user.lua");
        let row = &v["binds.uq.close"];
        assert_eq!(row["source"], "user_lua");
        assert_eq!(
            row["detail"],
            json!({"line": 3, "path": "/home/u/user.lua"})
        );
        assert_eq!(v["binds.uq.float"]["source"], "default");
    }

    #[test]
    fn missing_live_without_hit_is_runtime() {
        let k = Keybinds::default();
        let live: Vec<LiveBind> = live_of(&k)
            .into_iter()
            .filter(|b| b.description != "uq:uq.close")
            .collect();
        let v = explain(
            &k,
            Some(&live),
            Some("hl.unbind(\"SUPER + W\")\n"),
            None,
            "/u",
        );
        assert_eq!(v["binds.uq.close"]["source"], "runtime");
        // The same keys live under someone else's description is not ours either.
        let mut other = live_of(&k);
        other
            .iter_mut()
            .find(|b| b.description == "uq:uq.close")
            .unwrap()
            .description = "mine".into();
        let v = explain(&k, Some(&other), None, None, "/u");
        assert_eq!(v["binds.uq.close"]["source"], "runtime");
    }

    #[test]
    fn rejected_marks_every_row_and_the_section() {
        let k = Keybinds::default();
        let v = explain(&k, Some(&live_of(&k)), None, Some("bad"), "/u");
        assert_eq!(v.as_object().unwrap().len(), 57);
        for (key, row) in v.as_object().unwrap() {
            assert_eq!(row["source"], "error", "{key}");
            assert_eq!(row["detail"], json!({"message": "bad"}), "{key}");
        }
        assert_eq!(v["keybinds"]["stored"], Value::Null);
    }

    #[test]
    fn hyprland_down_reports_state() {
        let k = kb(json!({"overrides": [{"id": "uq.spotlight", "combo": "SUPER + K"}]}));
        let v = explain(&k, None, Some("hl.unbind(\"all\")\n"), None, "/u");
        assert_eq!(v["binds.uq.spotlight"]["source"], "uq");
        assert_eq!(v["binds.uq.close"]["source"], "default");
        // So does a Hyprland that prints no `uq:` description at all.
        let none = [LiveBind {
            combo: None,
            description: String::new(),
        }];
        let v = explain(&k, Some(&none), None, None, "/u");
        assert_eq!(v["binds.uq.close"]["source"], "default");
    }

    #[test]
    fn live_binds_parses_hyprctl_json() {
        let json = r#"[
          {"locked": false, "mouse": false, "release": false, "repeat": false,
           "longPress": false, "non_consuming": false, "modmask": 64, "submap": "",
           "submap_universal": "false", "key": "Space", "keycode": 0, "catch_all": false,
           "description": "uq:uq.spotlight", "dispatcher": "global", "arg": "uq:spotlight"},
          {"modmask": 65, "key": "mouse_down", "description": "uq:uq.ws.next"},
          {"modmask": 2, "key": "Q", "description": "capslock"},
          {"modmask": 64, "key": "code:12", "description": ""},
          {"modmask": 64, "key": "q"},
          {"key": 7, "description": 3}
        ]"#;
        let l = live_binds(json);
        assert_eq!(l.len(), 6);
        assert_eq!(l[0].description, "uq:uq.spotlight");
        assert_eq!(combo::canonical(l[0].combo.unwrap()), "SUPER + Space");
        assert_eq!(
            combo::canonical(l[1].combo.unwrap()),
            "SUPER + SHIFT + mouse_down"
        );
        assert_eq!(l[2].combo, None);
        assert_eq!(l[3].combo, None);
        assert_eq!(combo::canonical(l[4].combo.unwrap()), "SUPER + Q");
        assert_eq!(l[4].description, "");
        assert_eq!(l[5].combo, None);
        assert!(live_binds("not json").is_empty());
        assert!(live_binds("{}").is_empty());
        let many = format!("[{}]", vec!["{}"; 2500].join(","));
        assert_eq!(live_binds(&many).len(), 2000);
    }
}
