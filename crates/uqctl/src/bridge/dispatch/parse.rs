//! Reading an action from the shell's call: its name and JSON arguments, checked.

use super::*;

pub(crate) fn address(o: &Value) -> Result<String, String> {
    address_at(o, "address")
}

pub(crate) fn address_at(o: &Value, key: &str) -> Result<String, String> {
    let a = o
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} must be a string"))?;
    if valid_address(a) {
        Ok(a.to_string())
    } else {
        Err(format!("invalid window address {a:?}"))
    }
}

/// An integer `key` within `lo..=hi`.
pub(crate) fn int_in(o: &Value, key: &str, lo: i64, hi: i64) -> Result<i64, String> {
    match o.get(key).and_then(Value::as_i64) {
        Some(n) if (lo..=hi).contains(&n) => Ok(n),
        _ => Err(format!("{key} must be a whole number from {lo} to {hi}")),
    }
}

/// A real workspace id; special workspaces (negative ids) are not addressed this way.
pub(crate) fn ws_id(o: &Value, key: &str) -> Result<i64, String> {
    match o.get(key).and_then(Value::as_i64) {
        Some(n) if n >= 1 => Ok(n),
        _ => Err(format!("{key} must be a workspace id of 1 or more")),
    }
}

pub(crate) fn ws_target(o: &Value) -> Result<WsTarget, String> {
    match o.get("workspace") {
        Some(Value::String(s)) => {
            let name = s.strip_prefix("special:").unwrap_or(s);
            if valid_special(name) {
                Ok(WsTarget::Special(name.to_string()))
            } else {
                Err(format!("invalid special workspace name {s:?}"))
            }
        }
        Some(v) if v.is_i64() => ws_id(o, "workspace").map(WsTarget::Id),
        _ => Err("workspace must be an id or a special workspace name".into()),
    }
}

impl Action {
    pub fn from_call(name: &str, args: &Value) -> Result<Action, String> {
        let a = args;
        Ok(match name {
            "focus_workspace" => Action::FocusWorkspace {
                id: ws_id(a, "id")?,
            },
            "focus_window" => Action::FocusWindow {
                address: address(a)?,
                keep_pointer: match a.get("keep_pointer") {
                    None => false,
                    Some(v) => v.as_bool().ok_or("keep_pointer must be a boolean")?,
                },
            },
            "close" => Action::Close {
                address: address(a)?,
            },
            "toggle_float" => Action::ToggleFloat {
                address: address(a)?,
            },
            "pin" => Action::Pin {
                address: address(a)?,
            },
            "fullscreen" => Action::Fullscreen {
                address: address(a)?,
                mode: match a.get("mode").and_then(Value::as_str) {
                    Some("full") => FsMode::Full,
                    Some("maximized") => FsMode::Maximized,
                    _ => return Err("mode must be \"full\" or \"maximized\"".into()),
                },
            },
            "move" => Action::Move {
                address: address(a)?,
                workspace: ws_target(a)?,
            },
            "minimise" => Action::Minimise {
                address: address(a)?,
                from_ws: ws_id(a, "from_ws")?,
            },
            "restore" => Action::Restore {
                address: address(a)?,
                to_ws: ws_id(a, "to_ws")?,
                after: match a.get("after") {
                    None | Some(Value::Null) => None,
                    Some(_) => Some(address_at(a, "after")?),
                },
            },
            "resize" => Action::Resize {
                windows: match a.get("windows").and_then(Value::as_array) {
                    Some(list) if (1..=4).contains(&list.len()) => list
                        .iter()
                        .map(|w| {
                            Ok((
                                address(w)?,
                                int_in(w, "w", 1, 32_768)?,
                                int_in(w, "h", 1, 32_768)?,
                            ))
                        })
                        .collect::<Result<_, String>>()?,
                    _ => return Err("windows must list 1 to 4 {address, w, h}".into()),
                },
            },
            "columns" => Action::Columns {
                widths: match a.get("windows").and_then(Value::as_array) {
                    Some(list) if (1..=4).contains(&list.len()) => list
                        .iter()
                        .map(|w| {
                            let f = w
                                .get("fraction")
                                .and_then(Value::as_f64)
                                .filter(|f| (0.05..=1.0).contains(f))
                                .ok_or("fraction must be from 0.05 to 1")?;
                            Ok((address(w)?, (f * 10_000.0).round() as u32))
                        })
                        .collect::<Result<_, String>>()?,
                    _ => return Err("windows must list 1 to 4 {address, fraction}".into()),
                },
                pin: match a.get("pin") {
                    None => None,
                    Some(p) => Some((address(p)?, int_in(p, "x", -65_536, 65_536)?)),
                },
            },
            "move_column" => Action::MoveColumn {
                address: address(a)?,
                steps: match int_in(a, "steps", -64, 64)? {
                    0 => return Err("steps must not be 0".into()),
                    n => n,
                },
            },
            "own_column" => Action::OwnColumn {
                address: address(a)?,
            },
            "page_hiding" => Action::PageHiding {
                workspaces: match a.get("workspaces").and_then(Value::as_array) {
                    Some(list) if list.len() <= 32 => list
                        .iter()
                        .map(|w| {
                            let entries = match w.get("windows").and_then(Value::as_array) {
                                Some(e) if (1..=64).contains(&e.len()) => e
                                    .iter()
                                    .map(|e| {
                                        let hide = match e.get("hide").and_then(Value::as_array) {
                                            Some(h) if h.len() <= 64 => h
                                                .iter()
                                                .map(|h| {
                                                    h.as_str()
                                                        .filter(|h| valid_address(h))
                                                        .map(str::to_string)
                                                        .ok_or("hide must list window addresses")
                                                })
                                                .collect::<Result<Vec<_>, _>>()?,
                                            _ => {
                                                return Err(
                                                    "hide must list up to 64 windows".into()
                                                );
                                            }
                                        };
                                        Ok((address(e)?, hide))
                                    })
                                    .collect::<Result<Vec<_>, String>>()?,
                                _ => return Err("windows must list 1 to 64 {address, hide}".into()),
                            };
                            let anchor = w
                                .get("anchor")
                                .and_then(Value::as_str)
                                .filter(|a| valid_address(a))
                                .ok_or("anchor must be a window address")?;
                            Ok((ws_id(w, "id")?, anchor.to_string(), entries))
                        })
                        .collect::<Result<_, String>>()?,
                    _ => return Err("workspaces must list up to 32 workspaces".into()),
                },
                reset: match a.get("reset") {
                    None => Vec::new(),
                    Some(r) => match r.as_array() {
                        Some(r) if r.len() <= 256 => r
                            .iter()
                            .map(|h| {
                                h.as_str()
                                    .filter(|h| valid_address(h))
                                    .map(str::to_string)
                                    .ok_or("reset must list window addresses")
                            })
                            .collect::<Result<_, _>>()?,
                        _ => return Err("reset must list up to 256 windows".into()),
                    },
                },
            },
            "scroll" => Action::Scroll {
                address: address(a)?,
                x: int_in(a, "x", -65_536, 65_536)?,
            },
            "place" => Action::Place {
                address: address(a)?,
                x: int_in(a, "x", -65_536, 65_536)?,
                y: int_in(a, "y", -65_536, 65_536)?,
                w: int_in(a, "w", 1, 32_768)?,
                h: int_in(a, "h", 1, 32_768)?,
            },
            "dock" => Action::Dock {
                address: address(a)?,
                x: int_in(a, "x", -65_536, 65_536)?,
                y: int_in(a, "y", -65_536, 65_536)?,
                w: int_in(a, "w", 1, 32_768)?,
                h: int_in(a, "h", 1, 32_768)?,
            },
            "dpms" => Action::Dpms {
                on: a
                    .get("on")
                    .and_then(Value::as_bool)
                    .ok_or("on must be a boolean")?,
            },
            "column_width" => Action::ColumnWidth {
                address: address(a)?,
                full: match a.get("width").and_then(Value::as_str) {
                    Some("full") => true,
                    Some("half") => false,
                    _ => return Err("width must be \"full\" or \"half\"".into()),
                },
                keep_focus: match a.get("keep_focus") {
                    None => false,
                    Some(v) => v.as_bool().ok_or("keep_focus must be a boolean")?,
                },
            },
            other => return Err(format!("unknown action {other}")),
        })
    }
}
