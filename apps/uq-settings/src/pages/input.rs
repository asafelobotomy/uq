//! The Input page's logic: [`InputState`] reads and writes uqd's `input` section (keyboard,
//! mouse, touchpad and per-device settings) and lists the devices plugged in. Plain Rust, tested
//! against the fake uqd; `crate::input_model` is the thin QML object around it.
//!
//! The section's `mouse` part is the `[mouse]` table `uqctl mouse` also writes, so a change made
//! there shows here at once.

use serde_json::{Map, Value, json};
use tokio::sync::Mutex;
use uq_core::client::DesktopProxy;

use crate::pages::runner::{PageLogic, parse_json, patch_section};
use crate::xkb_lists::{self, XkbLists};

const KEYBOARD_FIELDS: [&str; 4] = ["layouts", "switch_combo", "repeat_rate", "repeat_delay"];
const MOUSE_FIELDS: [&str; 5] = [
    "sensitivity",
    "accel_profile",
    "natural_scroll",
    "left_handed",
    "scroll_factor",
];
const TOUCHPAD_FIELDS: [&str; 4] = [
    "natural_scroll",
    "tap_to_click",
    "disable_while_typing",
    "scroll_factor",
];
/// A device setting's fields are the mouse's.
const DEVICE_FIELDS: [&str; 5] = MOUSE_FIELDS;

/// What the page asks for.
#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    /// One `keyboard` field (`null` = the default); `layouts` is the whole list.
    SetKeyboard {
        field: String,
        value: Value,
    },
    SetMouse {
        field: String,
        value: Value,
    },
    SetTouchpad {
        field: String,
        value: Value,
    },
    /// One field of a device's setting; `null` removes the field, and the device when it was
    /// the last one.
    SetDevice {
        name: String,
        field: String,
        value: Value,
    },
    RemoveDevice {
        name: String,
    },
}

/// What applies when a field is unset: base.lua's value where it sets one (touchpad natural
/// scrolling), else Hyprland's own default. The page seeds "Use default" from this.
pub fn defaults() -> Value {
    json!({
        "keyboard": {"repeat_rate": 25, "repeat_delay": 600},
        "touchpad": {
            "natural_scroll": false,
            "tap_to_click": true,
            "disable_while_typing": true,
            "scroll_factor": 1.0,
        },
    })
}

pub struct InputState {
    conn: zbus::Connection,
    write: Mutex<()>,
    xkb: XkbLists,
}

impl InputState {
    pub fn new(conn: zbus::Connection) -> Self {
        Self::with_xkb(conn, xkb_lists::load())
    }

    pub fn with_xkb(conn: zbus::Connection, xkb: XkbLists) -> Self {
        Self {
            conn,
            write: Mutex::new(()),
            xkb,
        }
    }
}

fn refuse<T>(text: String) -> zbus::Result<T> {
    Err(zbus::Error::Failure(text))
}

/// `section[part][field] = value` (`null` removes it).
fn set_in(
    section: &mut Map<String, Value>,
    part: &str,
    field: String,
    value: Value,
) -> zbus::Result<()> {
    let part = section
        .entry(part)
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| zbus::Error::Failure(format!("the {part} settings aren't an object")))?;
    if value.is_null() {
        part.remove(&field);
    } else {
        part.insert(field, value);
    }
    Ok(())
}

impl PageLogic for InputState {
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
        let known = match &op {
            Op::SetKeyboard { field, .. } => KEYBOARD_FIELDS.contains(&field.as_str()),
            Op::SetMouse { field, value } => {
                // `[mouse]` has no "unset": its fields always hold a value.
                if value.is_null() {
                    return refuse(format!("{field} needs a value"));
                }
                MOUSE_FIELDS.contains(&field.as_str())
            }
            Op::SetTouchpad { field, .. } => TOUCHPAD_FIELDS.contains(&field.as_str()),
            Op::SetDevice { field, .. } => DEVICE_FIELDS.contains(&field.as_str()),
            Op::Refresh | Op::RemoveDevice { .. } => true,
        };
        if !known {
            let field = match &op {
                Op::SetKeyboard { field, .. }
                | Op::SetMouse { field, .. }
                | Op::SetTouchpad { field, .. }
                | Op::SetDevice { field, .. } => field.clone(),
                _ => String::new(),
            };
            return refuse(format!("unknown field {field:?}"));
        }
        let proxy = DesktopProxy::new(&self.conn).await?;
        match op {
            Op::Refresh => Ok(()),
            Op::SetKeyboard { field, value } => {
                patch_section(&proxy, "input", |s| set_in(s, "keyboard", field, value)).await
            }
            Op::SetMouse { field, value } => {
                patch_section(&proxy, "input", |s| set_in(s, "mouse", field, value)).await
            }
            Op::SetTouchpad { field, value } => {
                patch_section(&proxy, "input", |s| set_in(s, "touchpad", field, value)).await
            }
            Op::SetDevice { name, field, value } => {
                patch_section(&proxy, "input", |s| {
                    let devices = s
                        .entry("devices")
                        .or_insert_with(|| Value::Object(Map::new()))
                        .as_object_mut()
                        .ok_or_else(|| {
                            zbus::Error::Failure("the device settings aren't an object".into())
                        })?;
                    set_in(devices, &name, field, value)?;
                    if devices[&name].as_object().is_some_and(Map::is_empty) {
                        devices.remove(&name);
                    }
                    Ok(())
                })
                .await
            }
            Op::RemoveDevice { name } => {
                patch_section(&proxy, "input", |s| {
                    if let Some(d) = s.get_mut("devices").and_then(Value::as_object_mut) {
                        d.remove(&name);
                    }
                    Ok(())
                })
                .await
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let input = parse_json(&proxy.get_section("input").await?)?;
        // The device list is a nicety: without it the page still edits the settings.
        let devices = match proxy.list_input_devices().await {
            Ok(text) => parse_json(&text).unwrap_or_else(|_| json!({})),
            Err(_) => json!({}),
        };
        Ok(json!({
            "input": input,
            "devices": devices,
            "xkb": self.xkb,
            "defaults": defaults(),
        }))
    }
}

#[cfg(test)]
mod tests;
