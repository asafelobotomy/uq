//! `Dispatch`: the closed set of window actions the shell may ask for, mapped to Lua (spec §2.8).
//!
//! The shell never sends Lua. Every action is parsed into an `Action`, validated, and only then
//! turned into `hl.dispatch(...)` statements. Windows are always addressed as `address:0x…`.
//!
//! Hyprland forms confirmed against `LuaBindingsDispatchers.cpp` / `LuaBindingsInternal.cpp` in
//! v0.56.2 and 0.57: the `window` key is read by `pushWindowUpval` (a selector string), `float`
//! and `pin` take `action` toggle/enable/disable (`parseToggleStr`), `fullscreen` takes `mode`
//! fullscreen/maximized, `window.move` takes `workspace` and `follow` (false = silent), `dpms`
//! takes `action` on/off, and `window.tag` takes `tag` where a leading `+` sets, `-` unsets and
//! no prefix toggles (`CTagKeeper::applyTag`). `cursor.move` takes `x` and `y` in global layout
//! coordinates (checked live on 0.57.2).

use serde_json::Value;
use uq_core::lua::lua_string;
use uqd::compositor::CompositorError;
use uqd::hypr::HyprSocket;

/// A window address as Hyprland prints it: `0x` and 1 to 16 lowercase hex digits.
pub fn valid_address(s: &str) -> bool {
    s.strip_prefix("0x").is_some_and(|h| {
        (1..=16).contains(&h.len())
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// A special workspace name: `^[a-z0-9_-]{1,32}$`.
fn valid_special(s: &str) -> bool {
    (1..=32).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsMode {
    Full,
    Maximized,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsTarget {
    Id(i64),
    Special(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    FocusWorkspace {
        id: i64,
    },
    FocusWindow {
        address: String,
        /// Put the pointer back where it was after focusing: Hyprland warps it to the focused
        /// window unless `cursor:no_warps` is set, which would pull it off a screen-edge
        /// indicator mid-gesture.
        keep_pointer: bool,
    },
    Close {
        address: String,
    },
    ToggleFloat {
        address: String,
    },
    Pin {
        address: String,
    },
    Fullscreen {
        address: String,
        mode: FsMode,
    },
    Move {
        address: String,
        workspace: WsTarget,
    },
    Minimise {
        address: String,
        from_ws: i64,
    },
    Restore {
        address: String,
        to_ws: i64,
    },
    Dpms {
        on: bool,
    },
}

fn address(o: &Value) -> Result<String, String> {
    let a = o
        .get("address")
        .and_then(Value::as_str)
        .ok_or("address must be a string")?;
    if valid_address(a) {
        Ok(a.to_string())
    } else {
        Err(format!("invalid window address {a:?}"))
    }
}

/// A real workspace id; special workspaces (negative ids) are not addressed this way.
fn ws_id(o: &Value, key: &str) -> Result<i64, String> {
    match o.get(key).and_then(Value::as_i64) {
        Some(n) if n >= 1 => Ok(n),
        _ => Err(format!("{key} must be a workspace id of 1 or more")),
    }
}

fn ws_target(o: &Value) -> Result<WsTarget, String> {
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
            },
            "dpms" => Action::Dpms {
                on: a
                    .get("on")
                    .and_then(Value::as_bool)
                    .ok_or("on must be a boolean")?,
            },
            other => return Err(format!("unknown action {other}")),
        })
    }

    /// The `return hl.dispatch(…)` statements, in order; each is sent as its own `eval`.
    pub fn to_lua(&self) -> Vec<String> {
        let win = |a: &str| lua_string(&format!("address:{a}"));
        let d = |inner: String| format!("return hl.dispatch({inner})");
        match self {
            Action::FocusWorkspace { id } => {
                vec![d(format!("hl.dsp.focus({{ workspace = {id} }})"))]
            }
            Action::FocusWindow { address, .. } => {
                vec![d(format!("hl.dsp.focus({{ window = {} }})", win(address)))]
            }
            Action::Close { address } => vec![d(format!(
                "hl.dsp.window.close({{ window = {} }})",
                win(address)
            ))],
            Action::ToggleFloat { address } => vec![d(format!(
                "hl.dsp.window.float({{ action = \"toggle\", window = {} }})",
                win(address)
            ))],
            Action::Pin { address } => vec![d(format!(
                "hl.dsp.window.pin({{ window = {} }})",
                win(address)
            ))],
            Action::Fullscreen { address, mode } => {
                let mode = match mode {
                    FsMode::Full => "fullscreen",
                    FsMode::Maximized => "maximized",
                };
                vec![d(format!(
                    "hl.dsp.window.fullscreen({{ mode = {}, window = {} }})",
                    lua_string(mode),
                    win(address)
                ))]
            }
            Action::Move { address, workspace } => vec![move_lua(address, workspace)],
            Action::Minimise { address, from_ws } => vec![
                tag_lua(address, &format!("+uq_ws_{from_ws}")),
                move_lua(address, &WsTarget::Special("minimized".into())),
            ],
            Action::Restore { address, to_ws } => vec![
                move_lua(address, &WsTarget::Id(*to_ws)),
                tag_lua(address, &format!("-uq_ws_{to_ws}")),
            ],
            Action::Dpms { on } => vec![d(format!(
                "hl.dsp.dpms({{ action = {} }})",
                lua_string(if *on { "on" } else { "off" })
            ))],
        }
    }
}

fn move_lua(address: &str, ws: &WsTarget) -> String {
    let ws = match ws {
        WsTarget::Id(n) => n.to_string(),
        WsTarget::Special(s) => lua_string(&format!("special:{s}")),
    };
    format!(
        "return hl.dispatch(hl.dsp.window.move({{ workspace = {ws}, follow = false, window = {} }}))",
        lua_string(&format!("address:{address}"))
    )
}

/// Move the pointer to global layout coordinates `(x, y)`.
fn cursor_move_lua(x: i64, y: i64) -> String {
    format!("return hl.dispatch(hl.dsp.cursor.move({{ x = {x}, y = {y} }}))")
}

/// The pointer position from Hyprland's `j/cursorpos` reply (`{"x": …, "y": …}`).
fn parse_cursorpos(reply: &str) -> Option<(i64, i64)> {
    let v: Value = serde_json::from_str(reply).ok()?;
    let n = |k: &str| {
        let f = v.get(k)?.as_f64()?;
        f.is_finite().then(|| f.round() as i64)
    };
    Some((n("x")?, n("y")?))
}

fn tag_lua(address: &str, tag: &str) -> String {
    format!(
        "return hl.dispatch(hl.dsp.window.tag({{ tag = {}, window = {} }}))",
        lua_string(tag),
        lua_string(&format!("address:{address}"))
    )
}

fn eval_err(e: CompositorError) -> String {
    match e {
        CompositorError::NotRunning => "Hyprland is not running".into(),
        CompositorError::Rejected(msg) => msg,
        e => e.to_string(),
    }
}

/// Send every statement; stop at the first one Hyprland rejects. With `keep_pointer`, the
/// pointer is read first and moved back afterwards (best effort: a failed read just skips it).
pub async fn run(hypr: &HyprSocket, action: &Action) -> Result<(), String> {
    let restore = match action {
        Action::FocusWindow {
            keep_pointer: true, ..
        } => hypr
            .request("j/cursorpos")
            .await
            .ok()
            .and_then(|r| parse_cursorpos(&r)),
        _ => None,
    };
    for lua in action.to_lua() {
        hypr.eval(&lua).await.map_err(eval_err)?;
    }
    if let Some((x, y)) = restore {
        hypr.eval(&cursor_move_lua(x, y)).await.map_err(eval_err)?;
    }
    Ok(())
}

/// The bridge's `Dispatch` call: `[name, {args…}]`.
pub async fn dispatch(args: &[Value]) -> Result<Value, String> {
    let name = args
        .first()
        .and_then(Value::as_str)
        .ok_or("Dispatch takes an action name")?;
    let empty = Value::Object(Default::default());
    let action = Action::from_call(name, args.get(1).unwrap_or(&empty))?;
    run(&HyprSocket::from_env(), &action).await?;
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const A: &str = "0x55d0c0ffee";

    fn lua(name: &str, args: Value) -> Vec<String> {
        Action::from_call(name, &args).unwrap().to_lua()
    }

    #[test]
    fn dispatch_table() {
        let w = r#""address:0x55d0c0ffee""#;
        assert_eq!(
            lua("focus_workspace", json!({"id": 3})),
            ["return hl.dispatch(hl.dsp.focus({ workspace = 3 }))"]
        );
        assert_eq!(
            lua("focus_window", json!({"address": A})),
            [format!(
                "return hl.dispatch(hl.dsp.focus({{ window = {w} }}))"
            )]
        );
        assert_eq!(
            lua("close", json!({"address": A})),
            [format!(
                "return hl.dispatch(hl.dsp.window.close({{ window = {w} }}))"
            )]
        );
        assert_eq!(
            lua("toggle_float", json!({"address": A})),
            [format!(
                "return hl.dispatch(hl.dsp.window.float({{ action = \"toggle\", window = {w} }}))"
            )]
        );
        assert_eq!(
            lua("pin", json!({"address": A})),
            [format!(
                "return hl.dispatch(hl.dsp.window.pin({{ window = {w} }}))"
            )]
        );
        assert_eq!(
            lua("fullscreen", json!({"address": A, "mode": "full"})),
            [format!(
                "return hl.dispatch(hl.dsp.window.fullscreen({{ mode = \"fullscreen\", window = {w} }}))"
            )]
        );
        assert_eq!(
            lua("fullscreen", json!({"address": A, "mode": "maximized"})),
            [format!(
                "return hl.dispatch(hl.dsp.window.fullscreen({{ mode = \"maximized\", window = {w} }}))"
            )]
        );
        assert_eq!(
            lua("move", json!({"address": A, "workspace": 2})),
            [format!(
                "return hl.dispatch(hl.dsp.window.move({{ workspace = 2, follow = false, window = {w} }}))"
            )]
        );
        assert_eq!(
            lua("move", json!({"address": A, "workspace": "scratch"})),
            [format!(
                "return hl.dispatch(hl.dsp.window.move({{ workspace = \"special:scratch\", follow = false, window = {w} }}))"
            )]
        );
        assert_eq!(
            lua("minimise", json!({"address": A, "from_ws": 4})),
            [
                format!(
                    "return hl.dispatch(hl.dsp.window.tag({{ tag = \"+uq_ws_4\", window = {w} }}))"
                ),
                format!(
                    "return hl.dispatch(hl.dsp.window.move({{ workspace = \"special:minimized\", follow = false, window = {w} }}))"
                ),
            ]
        );
        assert_eq!(
            lua("restore", json!({"address": A, "to_ws": 4})),
            [
                format!(
                    "return hl.dispatch(hl.dsp.window.move({{ workspace = 4, follow = false, window = {w} }}))"
                ),
                format!(
                    "return hl.dispatch(hl.dsp.window.tag({{ tag = \"-uq_ws_4\", window = {w} }}))"
                ),
            ]
        );
        assert_eq!(
            lua("dpms", json!({"on": true})),
            ["return hl.dispatch(hl.dsp.dpms({ action = \"on\" }))"]
        );
        assert_eq!(
            lua("dpms", json!({"on": false})),
            ["return hl.dispatch(hl.dsp.dpms({ action = \"off\" }))"]
        );
    }

    #[test]
    fn rejects_bad_address() {
        for bad in [
            "0xZZ",
            "title:foo",
            "0x",
            "0X1",
            "0x0123456789abcdef0",
            "0xAB",
            "",
        ] {
            assert!(
                Action::from_call("close", &json!({"address": bad})).is_err(),
                "{bad}"
            );
        }
        assert!(Action::from_call("close", &json!({})).is_err());
        assert!(valid_address("0x1") && valid_address("0xdeadbeef"));
    }

    #[test]
    fn focus_window_keep_pointer() {
        let w = r#""address:0x55d0c0ffee""#;
        let plain = format!("return hl.dispatch(hl.dsp.focus({{ window = {w} }}))");
        let a = Action::from_call("focus_window", &json!({"address": A, "keep_pointer": true}))
            .unwrap();
        assert_eq!(
            a,
            Action::FocusWindow {
                address: A.into(),
                keep_pointer: true
            }
        );
        assert_eq!(a.to_lua(), [plain]);
        let a = Action::from_call("focus_window", &json!({"address": A})).unwrap();
        assert!(matches!(
            a,
            Action::FocusWindow {
                keep_pointer: false,
                ..
            }
        ));
        assert!(
            Action::from_call("focus_window", &json!({"address": A, "keep_pointer": 1})).is_err()
        );
        assert_eq!(
            cursor_move_lua(-12, 340),
            "return hl.dispatch(hl.dsp.cursor.move({ x = -12, y = 340 }))"
        );
        assert_eq!(parse_cursorpos(r#"{"x": 0, "y": 250}"#), Some((0, 250)));
        assert_eq!(parse_cursorpos(r#"{"x": 10.6, "y": -3.2}"#), Some((11, -3)));
        assert_eq!(parse_cursorpos(r#"{"x": 1}"#), None);
        assert_eq!(parse_cursorpos("0, 250"), None);
    }

    #[test]
    fn rejects_unknown_action() {
        let e = Action::from_call("kill", &json!({"address": A})).unwrap_err();
        assert!(e.contains("unknown action"), "{e}");
    }

    #[test]
    fn special_name_validated() {
        for bad in ["magic\"x", "", "UP", "a b", &"x".repeat(33)] {
            assert!(
                Action::from_call("move", &json!({"address": A, "workspace": bad})).is_err(),
                "{bad}"
            );
        }
        assert!(Action::from_call("move", &json!({"address": A, "workspace": "ok_1-b"})).is_ok());
    }

    #[test]
    fn bad_numbers_rejected() {
        assert!(Action::from_call("focus_workspace", &json!({"id": 0})).is_err());
        assert!(Action::from_call("focus_workspace", &json!({"id": "3"})).is_err());
        assert!(Action::from_call("minimise", &json!({"address": A})).is_err());
        assert!(Action::from_call("fullscreen", &json!({"address": A, "mode": "x"})).is_err());
        assert!(Action::from_call("dpms", &json!({"on": "yes"})).is_err());
    }

    /// Every generated statement must load under Hyprland's Lua. CI runs this with `--ignored`.
    #[test]
    #[ignore = "needs Hyprland; CI runs it with --ignored"]
    fn dispatch_lua_parses_under_hyprland() {
        let all = [
            ("focus_workspace", json!({"id": 3})),
            ("focus_window", json!({"address": A})),
            ("close", json!({"address": A})),
            ("toggle_float", json!({"address": A})),
            ("pin", json!({"address": A})),
            ("fullscreen", json!({"address": A, "mode": "full"})),
            ("fullscreen", json!({"address": A, "mode": "maximized"})),
            ("move", json!({"address": A, "workspace": 2})),
            ("move", json!({"address": A, "workspace": "scratch"})),
            ("minimise", json!({"address": A, "from_ws": 4})),
            ("restore", json!({"address": A, "to_ws": 4})),
            ("dpms", json!({"on": true})),
            ("dpms", json!({"on": false})),
        ];
        let mut body = format!(
            "hl.on(\"hyprland.start\", function() {} end)\n",
            cursor_move_lua(0, 250)
        );
        for (name, args) in &all {
            for stmt in Action::from_call(name, args).unwrap().to_lua() {
                body.push_str(&format!(
                    "hl.on(\"hyprland.start\", function() {stmt} end)\n"
                ));
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("user.lua");
        std::fs::write(&file, body).unwrap();
        let out = std::process::Command::new("Hyprland")
            .arg("--verify-config")
            .arg("-c")
            .arg(&file)
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.status.success(), "{text}");
    }
}
