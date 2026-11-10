//! The Look and layouts page's logic: [`LookState`] talks to uqd (`org.uq.Desktop1`) and is plain
//! Rust, tested against the fake uqd. `crate::look_model` is the thin QML object around it.
//!
//! The page's state is `{look, layouts}`; the theme tokens that "Use theme" seeds come from
//! `App.themeJson` in QML, not from here. A `layouts` write replaces the workspace rules, so
//! every change reads the section, edits it and writes it back under the write lock.

use serde_json::{Map, Value, json};
use tokio::sync::Mutex;
use uq_core::client::DesktopProxy;

use crate::pages::runner::{PageLogic, parse_json, patch_section};

/// The `look` section's fields that [`Op::SetLook`] may change.
const LOOK_FIELDS: [&str; 10] = [
    "gaps_in",
    "gaps_out",
    "border_width",
    "rounding",
    "blur",
    "blur_size",
    "blur_passes",
    "shadow",
    "animations",
    "animation_speed",
];

/// The option groups of the `layouts` section, with the fields of each.
const GROUPS: [(&str, &[&str]); 3] = [
    ("dwindle", &["preserve_split", "smart_split", "force_split"]),
    ("master", &["mfact", "new_status", "orientation"]),
    ("scrolling", &["column_width", "fullscreen_on_one_column"]),
];

/// What the page asks for.
#[derive(Debug, Clone)]
pub enum Op {
    /// Just read everything.
    Refresh,
    /// Replace one field of the `look` section (`null` = use the theme).
    SetLook { field: String, value: Value },
    /// Replace one option of the `layouts` section. `group` is "" (then `field` is `default`),
    /// `dwindle`, `master` or `scrolling`.
    SetLayoutOption {
        group: String,
        field: String,
        value: Value,
    },
    /// Give a workspace a layout, or remove its rule (`None`).
    SetWorkspace {
        selector: String,
        layout: Option<String>,
    },
}

/// What applies when a field is `null`: `config/hypr/base.lua`'s value where it sets one, else
/// Hyprland's own default. The page seeds "Use default" from this (state key `defaults`), so
/// there is one place; `defaults_match_base_lua` keeps it true to base.lua.
/// Gaps, border and rounding are not here: they come from the theme's `ui` tokens.
pub fn defaults() -> Value {
    json!({
        "look": {
            "blur": true,
            "blur_size": 3,
            "blur_passes": 1,
            "shadow": true,
            "animations": true,
            "animation_speed": 1.0,
        },
        "layouts": {
            "default": "dwindle",
            "dwindle": {"preserve_split": true, "smart_split": false, "force_split": "follow_mouse"},
            "master": {"mfact": 0.55, "new_status": "slave", "orientation": "left"},
            "scrolling": {"column_width": 0.5, "fullscreen_on_one_column": true},
        },
    })
}

pub struct LookState {
    conn: zbus::Connection,
    /// Writes read-modify-write the section, so one at a time.
    write: Mutex<()>,
}

impl LookState {
    pub fn new(conn: zbus::Connection) -> Self {
        Self {
            conn,
            write: Mutex::new(()),
        }
    }
}

fn refuse<T>(text: String) -> zbus::Result<T> {
    Err(zbus::Error::Failure(text))
}

fn set_layout_option(
    section: &mut Map<String, Value>,
    group: String,
    field: String,
    value: Value,
) -> zbus::Result<()> {
    if group.is_empty() {
        if field != "default" {
            return refuse(format!("unknown field {field:?}"));
        }
        section.insert(field, value);
        return Ok(());
    }
    let Some((_, fields)) = GROUPS.iter().find(|(g, _)| *g == group) else {
        return refuse(format!("unknown group {group:?}"));
    };
    if !fields.contains(&field.as_str()) {
        return refuse(format!("unknown field {field:?}"));
    }
    let entry = section
        .entry(group)
        .or_insert_with(|| Value::Object(Map::new()));
    match entry.as_object_mut() {
        Some(options) => {
            options.insert(field, value);
            Ok(())
        }
        None => refuse("the layout options aren't an object".into()),
    }
}

impl PageLogic for LookState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        // Refused before any call to uqd.
        match &op {
            Op::SetLook { field, .. } if !LOOK_FIELDS.contains(&field.as_str()) => {
                return refuse(format!("unknown field {field:?}"));
            }
            Op::SetLayoutOption {
                group,
                field,
                value,
            } => {
                // Check on a scratch section, so nothing is read for a bad name.
                set_layout_option(&mut Map::new(), group.clone(), field.clone(), value.clone())?;
            }
            _ => {}
        }
        let proxy = DesktopProxy::new(&self.conn).await?;
        match op {
            Op::Refresh => Ok(()),
            Op::SetLook { field, value } => {
                patch_section(&proxy, "look", |section| {
                    section.insert(field, value);
                    Ok(())
                })
                .await
            }
            Op::SetLayoutOption {
                group,
                field,
                value,
            } => {
                patch_section(&proxy, "layouts", |section| {
                    set_layout_option(section, group, field, value)
                })
                .await
            }
            Op::SetWorkspace { selector, layout } => {
                patch_section(&proxy, "layouts", |section| {
                    let rules = section
                        .entry("workspaces")
                        .or_insert_with(|| Value::Object(Map::new()))
                        .as_object_mut()
                        .ok_or_else(|| {
                            zbus::Error::Failure("the workspace rules aren't an object".into())
                        })?;
                    match layout {
                        Some(layout) => rules.insert(selector, Value::String(layout)),
                        None => rules.remove(&selector),
                    };
                    Ok(())
                })
                .await
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let look = parse_json(&proxy.get_section("look").await?)?;
        let layouts = parse_json(&proxy.get_section("layouts").await?)?;
        Ok(json!({ "look": look, "layouts": layouts, "defaults": defaults() }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::fake_uqd::{self, Rig};
    use crate::pages::runner::run;
    use uq_core::state::State;

    /// The value base.lua sets for `key` inside `table = {...}`, as JSON; `None` when it sets none.
    fn lua_value(src: &str, table: &str, key: &str) -> Option<Value> {
        let mut inside = false;
        for line in src.lines().map(str::trim) {
            if !inside {
                inside = line.starts_with(&format!("{table} ")) && line.ends_with('{');
            } else if line.starts_with('}') {
                return None;
            } else if let Some(rest) = line.strip_prefix(key)
                && let Some(v) = rest.trim_start().strip_prefix('=')
            {
                let v = v.split("--").next().unwrap().trim().trim_end_matches(',');
                return serde_json::from_str(v).ok();
            }
        }
        None
    }

    #[test]
    fn defaults_match_base_lua() {
        let base = include_str!("../../../../config/hypr/base.lua");
        let d = defaults();
        // (state path, base.lua table, key): base.lua sets these, so its value is the default.
        for (section, group, field, table, key) in [
            ("look", "", "blur", "blur", "enabled"),
            ("look", "", "blur_size", "blur", "size"),
            ("look", "", "blur_passes", "blur", "passes"),
            ("look", "", "shadow", "shadow", "enabled"),
            ("look", "", "animations", "animations", "enabled"),
            ("layouts", "", "default", "general", "layout"),
            (
                "layouts",
                "dwindle",
                "preserve_split",
                "dwindle",
                "preserve_split",
            ),
            (
                "layouts",
                "scrolling",
                "fullscreen_on_one_column",
                "scrolling",
                "fullscreen_on_one_column",
            ),
        ] {
            let seed = if group.is_empty() {
                &d[section][field]
            } else {
                &d[section][group][field]
            };
            assert_eq!(
                lua_value(base, table, key).as_ref(),
                Some(seed),
                "{section}.{group}.{field} vs base.lua {table}.{key}"
            );
        }
        // base.lua leaves these to Hyprland, so the seed is Hyprland's documented default.
        for (table, key) in [
            ("dwindle", "smart_split"),
            ("dwindle", "force_split"),
            ("master", "mfact"),
            ("master", "new_status"),
            ("master", "orientation"),
            ("scrolling", "column_width"),
        ] {
            assert_eq!(
                lua_value(base, table, key),
                None,
                "base.lua now sets {table}.{key}"
            );
        }
    }

    async fn rig() -> (Rig, LookState) {
        let mut state = State::default();
        state
            .set_section("look", json!({"gaps_in": 4, "blur": true}))
            .unwrap();
        state
            .set_section(
                "layouts",
                json!({"default": "dwindle", "workspaces": {"2": "scrolling"},
                       "master": {"mfact": 0.6}}),
            )
            .unwrap();
        let r = fake_uqd::rig(state).await;
        let page = LookState::new(r.client.clone());
        (r, page)
    }

    fn look(field: &str, value: Value) -> Op {
        Op::SetLook {
            field: field.into(),
            value,
        }
    }

    fn option(group: &str, field: &str, value: Value) -> Op {
        Op::SetLayoutOption {
            group: group.into(),
            field: field.into(),
            value,
        }
    }

    #[tokio::test]
    async fn set_look_keeps_other_fields() {
        let (r, page) = rig().await;
        let out = run(&page, look("gaps_out", json!(12))).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["look"]["gaps_out"], 12);
        assert_eq!(out.state["look"]["gaps_in"], 4);
        assert_eq!(out.state["look"]["blur"], true);
        assert_eq!(out.state["layouts"]["workspaces"]["2"], "scrolling");
        assert_eq!(r.state.lock().unwrap().settings.look.gaps_out, Some(12));

        // "Use theme": null clears one override and leaves the rest.
        let out = run(&page, look("gaps_in", Value::Null)).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["look"]["gaps_in"], Value::Null);
        assert_eq!(out.state["look"]["gaps_out"], 12);

        let out = run(&page, look("animation_speed", json!(1.5))).await;
        assert_eq!(out.state["look"]["animation_speed"], 1.5);
        let out = run(&page, look("rounding", json!(99))).await;
        assert!(out.error.unwrap().contains("rounding"));
        assert_eq!(out.state["look"]["gaps_out"], 12);
    }

    #[tokio::test]
    async fn set_option_in_group() {
        let (_r, page) = rig().await;
        let out = run(&page, option("dwindle", "force_split", json!("left"))).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["layouts"]["dwindle"]["force_split"], "left");
        assert_eq!(
            out.state["layouts"]["master"]["mfact"], 0.6,
            "other groups stay"
        );
        assert_eq!(out.state["layouts"]["workspaces"]["2"], "scrolling");

        let out = run(&page, option("", "default", json!("master"))).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["layouts"]["default"], "master");
        assert_eq!(out.state["layouts"]["dwindle"]["force_split"], "left");

        let out = run(&page, option("master", "mfact", Value::Null)).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["layouts"]["master"]["mfact"], Value::Null);

        let out = run(&page, option("scrolling", "column_width", json!(0.05))).await;
        assert!(out.error.unwrap().contains("column_width"));
    }

    #[tokio::test]
    async fn workspace_add_and_remove() {
        let (r, page) = rig().await;
        let out = run(
            &page,
            Op::SetWorkspace {
                selector: "5".into(),
                layout: Some("master".into()),
            },
        )
        .await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["layouts"]["workspaces"]["5"], "master");
        assert_eq!(out.state["layouts"]["workspaces"]["2"], "scrolling");
        assert_eq!(r.state.lock().unwrap().layouts.len(), 2);

        let out = run(
            &page,
            Op::SetWorkspace {
                selector: "2".into(),
                layout: None,
            },
        )
        .await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["layouts"]["workspaces"], json!({"5": "master"}));
        assert_eq!(out.state["layouts"]["default"], "dwindle", "options stay");

        let out = run(
            &page,
            Op::SetWorkspace {
                selector: "6".into(),
                layout: Some("tiled".into()),
            },
        )
        .await;
        assert!(out.error.is_some());
        assert_eq!(out.state["layouts"]["workspaces"], json!({"5": "master"}));
    }

    #[tokio::test]
    async fn rejected_verify_restores_state() {
        let (r, page) = rig().await;
        let before = run(&page, Op::Refresh).await.state;
        *r.verify_error.lock().unwrap() = Some("bad option".into());
        let out = run(&page, look("gaps_in", json!(9))).await;
        assert_eq!(
            out.error.as_deref(),
            Some("Hyprland rejected the new config: bad option")
        );
        assert_eq!(out.state, before);

        *r.verify_error.lock().unwrap() = Some("bad layout".into());
        let out = run(
            &page,
            Op::SetWorkspace {
                selector: "7".into(),
                layout: Some("master".into()),
            },
        )
        .await;
        assert_eq!(
            out.error.as_deref(),
            Some("Hyprland rejected the new config: bad layout")
        );
        assert_eq!(out.state, before);
    }

    #[tokio::test]
    async fn unknown_field_refused() {
        let (_r, page) = rig().await;
        let before = run(&page, Op::Refresh).await.state;
        for op in [
            look("nope", json!(1)),
            option("", "dwindle", json!(1)),
            option("dwindle", "mfact", json!(0.5)),
            option("tiling", "x", json!(1)),
        ] {
            let out = run(&page, op).await;
            assert!(out.error.as_deref().unwrap().contains("unknown"), "{out:?}");
            assert_eq!(out.state, before);
        }
        let out = run(&page, look("nope", json!(1))).await;
        assert_eq!(out.error.as_deref(), Some("unknown field \"nope\""));
    }
}
