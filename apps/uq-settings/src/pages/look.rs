//! The Look and layouts page's logic: [`LookState`] talks to uqd (`org.uq.Desktop1`) and is plain
//! Rust, tested against the fake uqd. `crate::look_model` is the thin QML object around it.
//!
//! The page's state is `{look, layouts, defaults, sources, reduce_motion}`; the theme tokens that "Use theme"
//! seeds come from `App.themeJson` in QML, not from here. A `layouts` write replaces the
//! workspace rules, so every change reads the section, edits it and writes it back under the
//! write lock.

use serde_json::{Map, Value, json};
use tokio::sync::Mutex;
use uq_core::client::DesktopProxy;

use crate::pages::runner::{ORIGIN, PageLogic, parse_json, patch_section, sources};

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
    /// Turn reduce motion (`ui.reduce_motion`, a `SetSetting` key) on or off.
    SetReduceMotion { on: bool },
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
            Op::SetReduceMotion { on } => {
                proxy
                    .set_setting("ui.reduce_motion", &json!(on).to_string(), ORIGIN)
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
        let sources = sources(&proxy, &["look", "layouts"]).await;
        let settings = parse_json(&proxy.get_settings().await?)?;
        let reduce_motion = settings["ui"]["reduce_motion"].as_bool().unwrap_or(false);
        Ok(
            json!({ "look": look, "layouts": layouts, "defaults": defaults(), "sources": sources,
                   "reduce_motion": reduce_motion }),
        )
    }
}

#[cfg(test)]
mod tests;
