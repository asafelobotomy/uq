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
        /// The window to land after: the last one (rightmost column) on the workspace, so the
        /// restored window joins at the end of the queue. None: where Hyprland puts it.
        after: Option<String>,
    },
    /// Give a floating window this size and put its top-left corner here (layout coordinates,
    /// as Hyprland reports `at`): the shell's "keep windows on screen".
    /// Give windows these sizes where they are, in this order, in one go: a tiled one's column
    /// width (scrolling) or share of its column's height. The shell's resize grips give two
    /// neighbours sizes that add up to what they had, so only the gap between them moves.
    Resize {
        windows: Vec<(String, i64, i64)>,
    },
    /// Set scrolling layout columns to these widths (fractions of the screen, as `colresize`
    /// takes them), in this order, then scroll the layout so window `pin` has its left edge at
    /// `x` again: the shell's resize grips move only the gap between two columns, keeping their
    /// outer edges where they were. `colresize` acts on the focused column, so each window is
    /// focused in turn (the pointer stays put) and the window focused before gets focus back.
    Columns {
        /// Each window and its column's width in ten-thousandths of the screen.
        widths: Vec<(String, u32)>,
        pin: Option<(String, i64)>,
    },
    /// Move the scrolling layout column holding this window `steps` places (negative: left), one
    /// `swapcol` at a time: the Open windows island's tabs dragged into a new order. The window
    /// is focused (the pointer stays put) and keeps focus, as a dragged tab does.
    MoveColumn {
        address: String,
        steps: i64,
    },
    /// Which windows to hide on each workspace, by the window focused there: beside a Half
    /// window alone on its screen page, the next page's windows would show, so the shell hides
    /// them. Hyprland is handed the table and hides and shows windows itself the moment focus
    /// moves (`PAGE_HIDING_LUA`, a `window.active` handler), as a round trip through the shell
    /// would show them for a frame or two. Each workspace: its id, the window its pages follow
    /// when focus is elsewhere, and per window the windows to hide while it is focused. `reset`:
    /// windows to show first (a shell that stopped may have left some hidden).
    PageHiding {
        workspaces: Vec<PageWorkspace>,
        reset: Vec<String>,
    },
    /// Scroll the scrolling layout's view so this window's left edge is at `x`: the shell keeps
    /// the view on a whole screen page. The layout's `move` focuses the window it brings into
    /// view, so the window focused before gets focus back; the pointer stays put.
    Scroll {
        address: String,
        x: i64,
    },
    Place {
        address: String,
        x: i64,
        y: i64,
        w: i64,
        h: i64,
    },
    Dpms {
        on: bool,
    },
    /// Make the scrolling layout column holding this window Full (the screen's width) or Half.
    /// The scrolling layout treats a full-width column as maximized and remembers the width to
    /// go back to, so Full is Hyprland's maximize and Half turns that off and then sets the
    /// column to half (`colresize`, which acts on the focused window's column). The window is
    /// focused first (without moving the pointer) and its state read then (`column_lua`); with
    /// `keep_focus` the window that had focus gets it back afterwards.
    ColumnWidth {
        address: String,
        full: bool,
        keep_focus: bool,
    },
}

/// One workspace of `Action::PageHiding`: its id, the window its pages follow when focus is
/// elsewhere, and per window the windows to hide while it is focused.
type PageWorkspace = (i64, String, Vec<(String, Vec<String>)>);

fn address(o: &Value) -> Result<String, String> {
    address_at(o, "address")
}

fn address_at(o: &Value, key: &str) -> Result<String, String> {
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
fn int_in(o: &Value, key: &str, lo: i64, hi: i64) -> Result<i64, String> {
    match o.get(key).and_then(Value::as_i64) {
        Some(n) if (lo..=hi).contains(&n) => Ok(n),
        _ => Err(format!("{key} must be a whole number from {lo} to {hi}")),
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

    /// The `return hl.dispatch(…)` statements, in order; `script` runs them as its steps.
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
            Action::Resize { windows } => windows
                .iter()
                .map(|(address, w, h)| {
                    d(format!(
                        "hl.dsp.window.resize({{ x = {w}, y = {h}, window = {} }})",
                        win(address)
                    ))
                })
                .collect(),
            Action::Columns { widths, .. } => widths
                .iter()
                .flat_map(|(address, f)| {
                    [
                        d(format!("hl.dsp.focus({{ window = {} }})", win(address))),
                        d(format!(
                            "hl.dsp.layout({})",
                            lua_string(&format!("colresize {}.{:04}", f / 10_000, f % 10_000))
                        )),
                    ]
                })
                .collect(),
            Action::MoveColumn { address, steps } => {
                let swap = d(format!(
                    "hl.dsp.layout({})",
                    lua_string(if *steps < 0 { "swapcol l" } else { "swapcol r" })
                ));
                std::iter::once(d(format!("hl.dsp.focus({{ window = {} }})", win(address))))
                    .chain(std::iter::repeat_n(swap, steps.unsigned_abs() as usize))
                    .collect()
            }
            // All in the script (page_hiding_lua).
            Action::PageHiding { .. } => Vec::new(),
            // All in the script's scroll step.
            Action::Scroll { .. } => Vec::new(),
            Action::Place {
                address,
                x,
                y,
                w,
                h,
            } => vec![
                d(format!(
                    "hl.dsp.window.resize({{ x = {w}, y = {h}, window = {} }})",
                    win(address)
                )),
                d(format!(
                    "hl.dsp.window.move({{ x = {x}, y = {y}, window = {} }})",
                    win(address)
                )),
            ],
            // Focus goes to the window last: a restore is the user choosing it, and the shell's
            // column policy treats it as a new window and focuses it briefly to size it, then
            // gives focus back to whatever had it. Without this the window lost to that.
            // Focusing `after` first makes the scrolling layout put the window next to it: the
            // end of the queue.
            Action::Restore {
                address,
                to_ws,
                after,
            } => {
                let mut steps = Vec::new();
                if let Some(after) = after {
                    steps.push(d(format!("hl.dsp.focus({{ window = {} }})", win(after))));
                }
                steps.push(move_lua(address, &WsTarget::Id(*to_ws)));
                steps.push(tag_lua(address, &format!("-uq_ws_{to_ws}")));
                steps.push(d(format!("hl.dsp.focus({{ window = {} }})", win(address))));
                steps
            }
            // The rest depends on the window's state once focused: see `run` and `column_lua`.
            Action::ColumnWidth { address, .. } => {
                vec![d(format!("hl.dsp.focus({{ window = {} }})", win(address)))]
            }
            Action::Dpms { on } => vec![d(format!(
                "hl.dsp.dpms({{ action = {} }})",
                lua_string(if *on { "on" } else { "off" })
            ))],
        }
    }
}

/// The rest of `ColumnWidth` for the (focused) window `address`, given its Hyprland fullscreen
/// state `fs` (0 none, 1 maximized, 2 fullscreen). Full: maximize it unless it already is. Half:
/// un-maximize it, then make its column half the screen. A fullscreen window is left alone.
fn column_lua(address: &str, full: bool, fs: i64) -> Vec<String> {
    let maximize = format!(
        "return hl.dispatch(hl.dsp.window.fullscreen({{ mode = \"maximized\", window = {} }}))",
        lua_string(&format!("address:{address}"))
    );
    let half = "return hl.dispatch(hl.dsp.layout(\"colresize 0.5\"))".to_string();
    match (full, fs) {
        (_, 2) | (true, 1) => vec![],
        (true, _) => vec![maximize],
        (false, 1) => vec![maximize, half],
        (false, _) => vec![half],
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

/// What the script raises when a step is rejected: Hyprland replies `error: ` and this, then
/// the step's own `level: message` (the same text a lone `eval` of that step replies with).
const STEP_FAILED: &str = "uq-step:";

/// Lua shared by every script: `fail(r)` turns an `hl.dispatch` result into nil (done) or the
/// `level: message` Hyprland would report for it (`{ok = false, level, error, code}`).
const PRELUDE: &str = r#"local function fail(r)
  if type(r) ~= "table" or (r.ok ~= false and r.error == nil) then return nil end
  return tostring(r.level or "error") .. ": " .. tostring(r.error or "dispatch failed")
end
"#;

/// `stmt` (`return hl.dispatch(…)`) as a step of `run` in `script`: stop at a rejection.
fn step(out: &mut String, stmt: &str) {
    let call = stmt.strip_prefix("return ").unwrap_or(stmt);
    out.push_str(&format!(
        "  do local m = fail({call}); if m then return m end end\n"
    ));
}

/// The whole action as one Lua chunk for one `eval`: run the steps in order and stop at the
/// first one Hyprland rejects. With `keep_pointer` (and for ColumnWidth) the pointer never
/// moves: Hyprland's warp to the focused window is switched off (`cursor:no_warps`) just for
/// the steps and always switched back, even when a step failed or raised. When that option
/// can't be read or set, the pointer position is read first and moved back afterwards instead
/// (best effort). ColumnWidth reads the focused window's fullscreen state after focusing it and
/// branches on it (`column_lua`); with `keep_focus` the window focused before gets focus back.
/// Lua (inside `run`) that scrolls the scrolling layout so window `address` has its left edge
/// at `x`, by the layout's `move`; nothing when it is already there or can't be read.
fn scroll_lua(address: &str, x: i64) -> String {
    format!(
        "  do local ok, w = pcall(hl.get_window, {})\n    \
         local dx = ok and w and w.at and ({x} - w.at.x) or 0\n    \
         if dx >= 1 or dx <= -1 then\n      \
         local m = fail(hl.dispatch(hl.dsp.layout(\"move \" .. math.floor(dx + 0.5)))); if m then return m end\n    \
         end\n  end\n",
        lua_string(&format!("address:{address}"))
    )
}

/// Hyprland's side of `PageHiding`, kept in its Lua state as `uq_pages` between calls: the
/// table from the shell, the windows hidden now per workspace, and a `window.active` handler
/// that applies the table when focus moves. A window is hidden by setting its opacity (focused,
/// unfocused and fullscreen: a Full column is maximized) to 0 and turning off the blur behind it
/// (drawn whatever its opacity), at once rather than faded: the
/// window's animations are off just around the change (so its moves still slide), and Hyprland's
/// focus fade (fadeSwitch) is kept off while focused and unfocused windows look the same, when it
/// shows nothing else (else it is as configured: `uq_pages_fade`, read by `run`).
const PAGE_HIDING_LUA: &str = r#"  uq_pages = uq_pages or { hidden = {} }
  local P = uq_pages
  -- Hidden (v "0.00") or shown ("1.00"). A hidden window has no blur either: Hyprland draws the
  -- blur behind a window whatever its opacity, which tints the empty space while it moves.
  function P.set(a, v)
    local w = "address:" .. a
    local blur = v == "0.00" and "1" or "unset"
    for _, kv in ipairs({ { "no_anim", "1" }, { "opacity", v }, { "opacity_inactive", v },
                          { "opacity_fullscreen", v }, { "no_blur", blur }, { "no_anim", "unset" } }) do
      pcall(hl.dispatch, hl.dsp.window.set_prop({ window = w, prop = kv[1], value = kv[2] }))
    end
  end
  -- Hyprland fades a window's opacity when focus moves (fadeSwitch), and it does so after this
  -- handler has run, so a window hidden at a focus change would show for a moment as it fades.
  -- That fade only shows anything when focused and unfocused windows differ in opacity: while
  -- they don't, it is kept off; when they do, it is as configured (`uq_pages_fade`, read by
  -- `run`; ignored while it is off by our hand, which is what Hyprland would report).
  if not P.fade_off then P.fade = uq_pages_fade end
  local function opt(k) local ok, v = pcall(hl.get_config, k) return ok and tonumber(v) or nil end
  local a, i = opt("decoration:active_opacity"), opt("decoration:inactive_opacity")
  local same = a ~= nil and a == i and a == opt("decoration:fullscreen_opacity")
  if same and not P.fade_off then
    if pcall(hl.animation, { leaf = "fadeSwitch", enabled = false }) then P.fade_off = true end
  elseif not same and P.fade_off then
    if P.fade then pcall(hl.animation, P.fade) end
    P.fade_off = false
  end
  function P.apply(ws, list)
    local want, cur = {}, P.hidden[ws] or {}
    for _, a in ipairs(list or {}) do want[a] = true end
    for a in pairs(cur) do if not want[a] then P.set(a, "1.00") end end
    for a in pairs(want) do if not cur[a] then P.set(a, "0.00") end end
    P.hidden[ws] = want
  end
  for _, a in ipairs(data.reset) do P.set(a, "1.00") end
  if #data.reset > 0 then P.hidden = {} end
  local map, listed = {}, {}
  for _, w in ipairs(data.ws) do
    listed[w.id] = true
    for a, hide in pairs(w.map) do map[a] = { ws = w.id, hide = hide } end
  end
  for ws in pairs(P.hidden) do if not listed[ws] then P.apply(ws, {}) end end
  P.map = map
  local ok, act = pcall(hl.get_active_window)
  local active = ok and act and act.address or nil
  for _, w in ipairs(data.ws) do
    local e = (active and map[active] and map[active].ws == w.id) and map[active] or map[w.anchor]
    P.apply(w.id, e and e.hide or {})
  end
  -- A reload builds Hyprland's animations afresh (the fade as configured again).
  if not (P.reload_sub and P.reload_sub:is_active()) then
    P.reload_sub = hl.on("config.reloaded", function() uq_pages.fade_off = false end)
  end
  if not (P.sub and P.sub:is_active()) then
    P.sub = hl.on("window.active", function(win)
      if not win or not win.address then return end
      local e = uq_pages.map and uq_pages.map[win.address]
      if e then pcall(uq_pages.apply, e.ws, e.hide) end
    end)
  end
"#;

/// The `PageHiding` table as Lua (`data`), then `PAGE_HIDING_LUA`.
fn page_hiding_lua(workspaces: &[PageWorkspace], reset: &[String]) -> String {
    let list = |v: &[String]| {
        v.iter()
            .map(|a| lua_string(a))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let ws = workspaces
        .iter()
        .map(|(id, anchor, entries)| {
            let map = entries
                .iter()
                .map(|(a, hide)| format!("[{}] = {{ {} }}", lua_string(a), list(hide)))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{{ id = {id}, anchor = {}, map = {{ {map} }} }}",
                lua_string(anchor)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "  local data = {{ reset = {{ {} }}, ws = {{ {ws} }} }}\n{PAGE_HIDING_LUA}",
        list(reset)
    )
}

fn script(action: &Action) -> String {
    let keep = matches!(
        action,
        Action::FocusWindow {
            keep_pointer: true,
            ..
        } | Action::Restore { .. }
            | Action::ColumnWidth { .. }
            | Action::Columns { .. }
            | Action::MoveColumn { .. }
            | Action::Scroll { .. }
    );
    let mut s = String::from(PRELUDE);
    s.push_str("local unwarp, pos, back = false, nil, nil\n");
    // The layout's `move` focuses the window it brings into view: give focus back after.
    if let Action::Columns { .. } | Action::Scroll { .. } = action {
        s.push_str(
            "do local ok, w = pcall(hl.get_active_window)\n  \
             if ok and w and w.address then back = w.address end end\n",
        );
    }
    if let Action::ColumnWidth {
        address,
        keep_focus: true,
        ..
    } = action
    {
        s.push_str(&format!(
            "do local ok, w = pcall(hl.get_active_window)\n  \
             if ok and w and w.address and w.address ~= {} then back = w.address end end\n",
            lua_string(address)
        ));
    }
    if keep {
        s.push_str(
            r#"do local got, nw = pcall(hl.get_config, "cursor:no_warps")
  if got and nw == false and pcall(hl.config, { cursor = { no_warps = true } }) then
    unwarp = true
  elseif not (got and nw == true) then
    local ok, p = pcall(hl.get_cursor_pos)
    if ok and type(p) == "table" and tonumber(p.x) and tonumber(p.y) then pos = p end
  end
end
"#,
        );
    }
    s.push_str("local function run()\n");
    for stmt in action.to_lua() {
        step(&mut s, &stmt);
    }
    match action {
        // Scroll the layout so the pinned window's left edge is back where it was.
        Action::Columns {
            pin: Some((address, x)),
            ..
        }
        | Action::Scroll { address, x } => s.push_str(&scroll_lua(address, *x)),
        Action::PageHiding { workspaces, reset } => s.push_str(&page_hiding_lua(workspaces, reset)),
        _ => {}
    }
    if let Action::ColumnWidth { address, full, .. } = action {
        // Now focused: its state decides what Full or Half takes. Unreadable counts as 0.
        s.push_str(
            "  local ok, w = pcall(hl.get_active_window)\n  \
             local fs = ok and w and tonumber(w.fullscreen) or 0\n",
        );
        for (cond, fs) in [
            ("if fs == 1 then", 1),
            ("elseif fs == 2 then", 2),
            ("else", 0),
        ] {
            s.push_str(&format!("  {cond}\n"));
            for stmt in column_lua(address, *full, fs) {
                s.push_str("  ");
                step(&mut s, &stmt);
            }
        }
        s.push_str("  end\n");
    }
    s.push_str(
        r#"  if back then
    do local m = fail(hl.dispatch(hl.dsp.focus({ window = "address:" .. back }))); if m then return m end end
  end
end
local ran, res = pcall(run)
if unwarp then
  local ok, e = pcall(hl.config, { cursor = { no_warps = false } })
  if not ok then error(tostring(e), 0) end
end
if not ran then error(tostring(res), 0) end
"#,
    );
    s.push_str(&format!(
        "if res then error({} .. res, 0) end\n",
        lua_string(STEP_FAILED)
    ));
    s.push_str(&format!(
        r#"if pos then
  local function round(v) if v < 0 then return -math.floor(-v + 0.5) end return math.floor(v + 0.5) end
  local m = fail(hl.dispatch(hl.dsp.cursor.move({{ x = round(pos.x), y = round(pos.y) }})))
  if m then error({} .. m, 0) end
end
"#,
        lua_string(STEP_FAILED)
    ));
    s
}

/// The error for a rejected script: a rejected step reads as Hyprland's own reply for it.
fn script_err(e: CompositorError) -> String {
    match e {
        CompositorError::Rejected(msg) => match msg.strip_prefix("error: ") {
            Some(m) if m.starts_with(STEP_FAILED) => m[STEP_FAILED.len()..].to_string(),
            _ => msg,
        },
        e => eval_err(e),
    }
}

/// Run `action` in one `eval` (see `script`).
pub async fn run(hypr: &HyprSocket, action: &Action) -> Result<(), String> {
    let mut lua = script(action);
    if let Action::PageHiding { .. } = action {
        // The focus fade as configured, for the handler to put back after hiding (unread: the
        // handler leaves the fade alone).
        let fade = match hypr.request("j/animations").await {
            Ok(r) => fade_switch(&r),
            Err(_) => None,
        };
        lua = format!("{}{lua}", fade_lua(fade.as_ref()));
    }
    hypr.eval(&lua).await.map_err(script_err)
}

/// The focus fade (`fadeSwitch`) as Hyprland has it, from a `j/animations` reply: its own
/// settings when set, else those of the leaf it takes them from (`fade`, then `global`).
/// `(enabled, speed, bezier)`; None when the reply can't be read.
fn fade_switch(reply: &str) -> Option<(bool, f64, String)> {
    let v: Value = serde_json::from_str(reply).ok()?;
    let leaves = v.get(0)?.as_array()?;
    let find = |name: &str| {
        leaves
            .iter()
            .find(|l| l.get("name").and_then(Value::as_str) == Some(name))
    };
    let leaf = ["fadeSwitch", "fade", "global"]
        .into_iter()
        .filter_map(find)
        .find(|l| l.get("overridden").and_then(Value::as_bool) == Some(true))?;
    let enabled = match leaf.get("enabled")? {
        Value::Bool(b) => *b,
        n => n.as_i64()? != 0,
    };
    let speed = leaf.get("speed")?.as_f64()?;
    let bezier = leaf.get("bezier")?.as_str()?;
    (bezier
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !bezier.is_empty())
    .then(|| (enabled, speed, bezier.to_string()))
}

/// The focus fade for `PAGE_HIDING_LUA` to restore, as a Lua global it reads (`uq_pages_fade`).
fn fade_lua(fade: Option<&(bool, f64, String)>) -> String {
    match fade {
        Some((enabled, speed, bezier)) => format!(
            "uq_pages_fade = {{ leaf = \"fadeSwitch\", enabled = {enabled}, speed = {speed}, bezier = {} }}\n",
            lua_string(bezier)
        ),
        None => "uq_pages_fade = nil\n".to_string(),
    }
}

/// The bridge's `WindowLook` call: how Hyprland draws windows right now, so the shell can draw
/// shapes that match them (the screen-edge indicator's window frame). `{border, rounding,
/// gaps_in, gaps_out: [top, right, bottom, left], active_border, inactive_border, layout}`, the
/// colours as `#AARRGGBB` (the first stop of a gradient), `layout` the default tiling layout
/// (dwindle, master, scrolling…). An option that can't be read is left out.
pub async fn window_look() -> Result<Value, String> {
    window_look_from(&HyprSocket::from_env()).await
}

/// The options `WindowLook` reads: its key, then the Hyprland option.
const LOOK_OPTIONS: [(&str, &str); 7] = [
    ("border", "general:border_size"),
    ("rounding", "decoration:rounding"),
    ("gaps_in", "general:gaps_in"),
    ("gaps_out", "general:gaps_out"),
    ("active_border", "general:col.active_border"),
    ("inactive_border", "general:col.inactive_border"),
    ("layout", "general:layout"),
];

/// Hyprland's separator between the replies of a `[[BATCH]]` request.
const BATCH_SEPARATOR: &str = "\n\n\n";

/// The replies of one `[[BATCH]]` request, one per request in it; `None` if the count is off
/// (an older Hyprland, or a reply that held the separator), so the caller asks one by one.
fn split_batch(reply: &str, n: usize) -> Option<Vec<&str>> {
    let parts: Vec<&str> = reply.split(BATCH_SEPARATOR).collect();
    (parts.len() == n).then_some(parts)
}

/// `WindowLook` against `hypr`: every option in one `[[BATCH]]` request (one connection, one
/// trip through Hyprland's loop instead of seven), falling back to one request per option.
async fn window_look_from(hypr: &HyprSocket) -> Result<Value, String> {
    let batch: Vec<String> = LOOK_OPTIONS
        .iter()
        .map(|(_, opt)| format!("j/getoption {opt}"))
        .collect();
    let batched = hypr.request(&format!("[[BATCH]]{}", batch.join(";"))).await;
    let replies: Vec<Option<String>> = match batched
        .as_deref()
        .ok()
        .and_then(|r| split_batch(r, batch.len()))
    {
        Some(parts) => parts.into_iter().map(|p| Some(p.to_string())).collect(),
        None if matches!(
            batched,
            Err(CompositorError::NotRunning | CompositorError::Timeout(_))
        ) =>
        {
            return Err("Hyprland is not running".into());
        }
        None => {
            let mut one_by_one = Vec::with_capacity(batch.len());
            for req in &batch {
                one_by_one.push(hypr.request(req).await.ok());
            }
            one_by_one
        }
    };
    let mut out = serde_json::Map::new();
    for ((key, _), reply) in LOOK_OPTIONS.iter().zip(replies) {
        if let Some(v) = reply.as_deref().and_then(parse_look_option) {
            out.insert((*key).into(), v);
        }
    }
    if out.is_empty() {
        return Err("Hyprland is not running".into());
    }
    Ok(Value::Object(out))
}

/// One `j/getoption` reply as `WindowLook` gives it: an int; a CSS box (`"4 4 4 4"`, one to four
/// values as in CSS) as `[top, right, bottom, left]`; a short word (a layout name); a gradient's
/// first colour as `#AARRGGBB`.
fn parse_look_option(reply: &str) -> Option<Value> {
    let v: Value = serde_json::from_str(reply).ok()?;
    if let Some(n) = v.get("int").and_then(Value::as_i64) {
        return Some(Value::from(n.clamp(0, 1000)));
    }
    if let Some(css) = v.get("css").and_then(Value::as_str) {
        let n: Vec<i64> = css
            .split_whitespace()
            .map(|p| p.parse::<i64>().ok().map(|x| x.clamp(0, 1000)))
            .collect::<Option<_>>()?;
        let b = match n.as_slice() {
            [a] => [*a, *a, *a, *a],
            [a, b] => [*a, *b, *a, *b],
            [a, b, c] => [*a, *b, *c, *b],
            [a, b, c, d] => [*a, *b, *c, *d],
            _ => return None,
        };
        return Some(Value::from(b.to_vec()));
    }
    if let Some(name) = v.get("str").and_then(Value::as_str) {
        // A layout name: short and plain.
        return (!name.is_empty()
            && name.len() <= 32
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
        .then(|| Value::from(name));
    }
    let g = v.get("gradient").and_then(Value::as_str)?;
    let first = g.split_whitespace().next()?;
    let hex = first.strip_prefix("0x").unwrap_or(first);
    (hex.len() == 8 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| Value::from(format!("#{}", hex.to_ascii_lowercase())))
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
                format!("return hl.dispatch(hl.dsp.focus({{ window = {w} }}))"),
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
    }

    #[test]
    fn place() {
        let w = r#""address:0x55d0c0ffee""#;
        assert_eq!(
            lua(
                "place",
                json!({"address": A, "x": -20, "y": 72, "w": 1880, "h": 990})
            ),
            [
                format!(
                    "return hl.dispatch(hl.dsp.window.resize({{ x = 1880, y = 990, window = {w} }}))"
                ),
                format!(
                    "return hl.dispatch(hl.dsp.window.move({{ x = -20, y = 72, window = {w} }}))"
                ),
            ]
        );
        for bad in [
            json!({"address": A, "x": 0, "y": 0, "w": 0, "h": 10}),
            json!({"address": A, "x": 0, "y": 0, "w": 10}),
            json!({"address": A, "x": 0.5, "y": 0, "w": 10, "h": 10}),
            json!({"address": A, "x": "0", "y": 0, "w": 10, "h": 10}),
            json!({"address": A, "x": 0, "y": 99_999_999, "w": 10, "h": 10}),
            json!({"address": "nope", "x": 0, "y": 0, "w": 10, "h": 10}),
        ] {
            assert!(Action::from_call("place", &bad).is_err(), "{bad}");
        }
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

    #[test]
    fn resize() {
        let w = r#""address:0x55d0c0ffee""#;
        const B: &str = "0x55d0c0ffef";
        let b = r#""address:0x55d0c0ffef""#;
        assert_eq!(
            lua(
                "resize",
                json!({"windows": [{"address": B, "w": 300, "h": 400}, {"address": A, "w": 600, "h": 400}]})
            ),
            [
                format!(
                    "return hl.dispatch(hl.dsp.window.resize({{ x = 300, y = 400, window = {b} }}))"
                ),
                format!(
                    "return hl.dispatch(hl.dsp.window.resize({{ x = 600, y = 400, window = {w} }}))"
                ),
            ],
            "in the order given"
        );
        for bad in [
            json!({"windows": [{"address": A, "w": 0, "h": 400}]}),
            json!({"windows": [{"address": A, "w": 600}]}),
            json!({"windows": []}),
            json!({"address": A, "w": 600, "h": 400}),
        ] {
            assert!(Action::from_call("resize", &bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn move_column() {
        let l = lua("move_column", json!({"address": A, "steps": -2}));
        assert_eq!(l.len(), 3, "focus, then one swap a step");
        assert!(l[0].contains("hl.dsp.focus"));
        assert!(l[1].contains("swapcol l") && l[2].contains("swapcol l"));
        assert!(lua("move_column", json!({"address": A, "steps": 1}))[1].contains("swapcol r"));
        assert!(
            script(&Action::from_call("move_column", &json!({"address": A, "steps": 1})).unwrap())
                .contains("no_warps"),
            "the pointer stays put"
        );
        for bad in [
            json!({"address": A, "steps": 0}),
            json!({"address": A, "steps": 99}),
            json!({"address": A}),
        ] {
            assert!(Action::from_call("move_column", &bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn fade_switch_follows_inheritance() {
        let leaf = |name: &str, o: bool, speed: f64, bezier: &str| json!({"name": name, "overridden": o, "bezier": bezier, "enabled": true, "speed": speed, "style": ""});
        let reply = |leaves: Vec<Value>| json!([leaves, []]).to_string();
        let r = reply(vec![
            leaf("global", true, 10.0, "default"),
            leaf("fade", true, 3.03, "quick"),
            leaf("fadeSwitch", false, 0.0, ""),
        ]);
        assert_eq!(
            fade_switch(&r),
            Some((true, 3.03, "quick".into())),
            "from fade"
        );
        let r = reply(vec![
            leaf("global", true, 10.0, "default"),
            leaf("fade", false, 0.0, ""),
            leaf("fadeSwitch", true, 2.0, "linear"),
        ]);
        assert_eq!(
            fade_switch(&r),
            Some((true, 2.0, "linear".into())),
            "its own"
        );
        let r = reply(vec![leaf("global", true, 10.0, "default")]);
        assert_eq!(
            fade_switch(&r),
            Some((true, 10.0, "default".into())),
            "from global"
        );
        assert_eq!(fade_switch("nope"), None);
        let r = reply(vec![leaf("global", true, 1.0, "a\" .. os.exit()")]);
        assert_eq!(fade_switch(&r), None, "an odd bezier name isn't passed on");
        assert!(
            fade_lua(Some(&(true, 3.03, "quick".into())))
                .contains("speed = 3.03, bezier = \"quick\"")
        );
        assert_eq!(fade_lua(None), "uq_pages_fade = nil\n");
    }

    #[test]
    fn page_hiding() {
        const B: &str = "0x55d0c0ffef";
        let act = Action::from_call(
            "page_hiding",
            &json!({"workspaces": [{"id": 1, "anchor": A,
                                     "windows": [{"address": A, "hide": [B]}, {"address": B, "hide": []}]}],
                    "reset": [A]}),
        )
        .unwrap();
        let s = script(&act);
        assert!(s.contains(&format!("reset = {{ \"{A}\" }}")), "{s}");
        assert!(s.contains(&format!("[\"{A}\"] = {{ \"{B}\" }}")), "{s}");
        assert!(s.contains("hl.on(\"window.active\""), "{s}");
        assert!(
            s.contains("\"opacity_inactive\"") && s.contains("\"no_anim\""),
            "{s}"
        );
        for bad in [
            json!({"workspaces": [{"id": 1, "anchor": A, "windows": []}]}),
            json!({"workspaces": [{"id": 1, "anchor": "nope", "windows": [{"address": A, "hide": []}]}]}),
            json!({"workspaces": [{"id": 1, "anchor": A, "windows": [{"address": A, "hide": ["x"]}]}]}),
            json!({"workspaces": "x"}),
        ] {
            assert!(Action::from_call("page_hiding", &bad).is_err(), "{bad}");
        }
        assert!(
            Action::from_call("page_hiding", &json!({"workspaces": []})).is_ok(),
            "nothing to hide"
        );
    }

    #[test]
    fn scroll() {
        let s = script(&Action::from_call("scroll", &json!({"address": A, "x": 16})).unwrap());
        assert!(
            s.contains("(16 - w.at.x)") && s.contains("\"move \""),
            "{s}"
        );
        assert!(
            s.contains("back = w.address"),
            "focus goes back after the move"
        );
        assert!(s.contains("no_warps = true"), "the pointer stays put");
        for bad in [
            json!({"address": A}),
            json!({"address": A, "x": 99_999_999}),
        ] {
            assert!(Action::from_call("scroll", &bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn columns() {
        const B: &str = "0x55d0c0ffef";
        let a = Action::from_call(
            "columns",
            &json!({"windows": [{"address": A, "fraction": 0.6}, {"address": B, "fraction": 0.4}],
                    "pin": {"address": A, "x": 16}}),
        )
        .unwrap();
        let lua = a.to_lua();
        assert_eq!(lua.len(), 4, "focus and colresize for each");
        assert!(lua[1].contains("colresize 0.6000"), "{}", lua[1]);
        assert!(lua[3].contains("colresize 0.4000"), "{}", lua[3]);
        let s = script(&a);
        assert!(
            s.contains("pcall(hl.get_window, \"address:0x55d0c0ffee\")")
                && s.contains("16 - w.at.x"),
            "{s}"
        );
        assert!(s.contains("back = w.address"), "focus goes back");
        for bad in [
            json!({"windows": [{"address": A, "fraction": 1.5}]}),
            json!({"windows": [{"address": A}]}),
            json!({"windows": []}),
            json!({"windows": [{"address": A, "fraction": 0.5}], "pin": {"address": A}}),
        ] {
            assert!(Action::from_call("columns", &bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn column_width() {
        let w = r#""address:0x55d0c0ffee""#;
        assert_eq!(
            lua("column_width", json!({"address": A, "width": "full"})),
            [format!(
                "return hl.dispatch(hl.dsp.focus({{ window = {w} }}))"
            )]
        );
        let max = format!(
            "return hl.dispatch(hl.dsp.window.fullscreen({{ mode = \"maximized\", window = {w} }}))"
        );
        let half = "return hl.dispatch(hl.dsp.layout(\"colresize 0.5\"))".to_string();
        assert_eq!(
            column_lua(A, true, 0),
            std::slice::from_ref(&max),
            "Full: maximize"
        );
        assert!(column_lua(A, true, 1).is_empty(), "already Full");
        assert_eq!(
            column_lua(A, false, 1),
            [max, half.clone()],
            "Half: un-maximize, then half"
        );
        assert_eq!(column_lua(A, false, 0), [half]);
        assert!(
            column_lua(A, true, 2).is_empty() && column_lua(A, false, 2).is_empty(),
            "fullscreen left alone"
        );
        let a = Action::from_call(
            "column_width",
            &json!({"address": A, "width": "full", "keep_focus": true}),
        )
        .unwrap();
        assert!(matches!(
            a,
            Action::ColumnWidth {
                keep_focus: true,
                ..
            }
        ));
        assert!(Action::from_call("column_width", &json!({"address": A, "width": 1})).is_err());
        assert!(Action::from_call("column_width", &json!({"address": A})).is_err());
        assert!(
            Action::from_call(
                "column_width",
                &json!({"address": A, "width": "full", "keep_focus": "yes"})
            )
            .is_err()
        );
    }

    #[test]
    fn one_script_per_action() {
        let w = r#""address:0x55d0c0ffee""#;
        let plain = Action::from_call("close", &json!({"address": A})).unwrap();
        let s = script(&plain);
        assert!(s.contains(&format!(
            "do local m = fail(hl.dispatch(hl.dsp.window.close({{ window = {w} }}))); if m then return m end end"
        )));
        assert!(!s.contains("hl.get_config"), "no pointer keeping");
        assert!(!s.contains("get_cursor_pos"));
        // Steps keep their order.
        let s =
            script(&Action::from_call("minimise", &json!({"address": A, "from_ws": 4})).unwrap());
        assert!(s.find("uq_ws_4").unwrap() < s.find("special:minimized").unwrap());
        // Restore ends by focusing the window, with the pointer kept still: the column policy
        // would otherwise focus it to size it and give focus back to the window before.
        let s = script(&Action::from_call("restore", &json!({"address": A, "to_ws": 4})).unwrap());
        assert!(s.find("uq_ws_4").unwrap() < s.find("hl.dsp.focus").unwrap());
        assert!(s.contains("no_warps = true"), "the pointer stays put");
        // With `after`, the last window on the workspace is focused first, so the restored
        // window lands behind it.
        let a2 = "0x55d0c0ffe2";
        let s = script(
            &Action::from_call("restore", &json!({"address": A, "to_ws": 4, "after": a2})).unwrap(),
        );
        assert!(s.find(a2).unwrap() < s.find("uq_ws_4").unwrap());
        assert!(Action::from_call("restore", &json!({"address": A, "to_ws": 4, "after": "x"})).is_err());
        // keep_pointer: no_warps switched on, and back after the steps whatever they did.
        let s = script(
            &Action::from_call("focus_window", &json!({"address": A, "keep_pointer": true}))
                .unwrap(),
        );
        let on = s.find("no_warps = true").unwrap();
        let run = s.find("pcall(run)").unwrap();
        let off = s.find("no_warps = false").unwrap();
        let raise = s.find("if not ran then").unwrap();
        assert!(on < run && run < off && off < raise);
        assert!(s.contains("hl.get_cursor_pos") && s.contains("hl.dsp.cursor.move"));
        // ColumnWidth: every fullscreen state gets `column_lua`'s steps, then the refocus.
        let s = script(
            &Action::from_call(
                "column_width",
                &json!({"address": A, "width": "half", "keep_focus": true}),
            )
            .unwrap(),
        );
        assert!(s.contains("if fs == 1 then") && s.contains("elseif fs == 2 then"));
        assert_eq!(s.matches("colresize 0.5").count(), 2, "fs 1 and fs 0");
        assert!(s.contains(r#"w.address ~= "0x55d0c0ffee""#));
    }

    #[test]
    fn rejected_step_reads_as_hyprland_reply() {
        let step = CompositorError::Rejected(
            "error: uq-step:warning: =[C]:-1: hl.focus: window not found".into(),
        );
        assert_eq!(
            script_err(step),
            "warning: =[C]:-1: hl.focus: window not found"
        );
        let lua = CompositorError::Rejected("error: [string \"x\"]:1: boom".into());
        assert_eq!(script_err(lua), "error: [string \"x\"]:1: boom");
        assert_eq!(
            script_err(CompositorError::NotRunning),
            "Hyprland is not running"
        );
    }

    /// Against the live compositor, on a window that doesn't exist (nothing real is touched):
    /// the reply is what the per-statement `eval` gave, and `cursor:no_warps` is as before.
    #[tokio::test]
    #[ignore = "needs a running Hyprland session"]
    async fn script_against_live_hyprland() {
        if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
            return;
        }
        let hypr = HyprSocket::from_env();
        let no_warps = || async {
            let r = hypr.request("j/getoption cursor:no_warps").await.unwrap();
            serde_json::from_str::<Value>(&r).unwrap()["bool"].clone()
        };
        let before = no_warps().await;
        let gone = "0x1";
        let single = hypr
            .eval(&format!(
                "return hl.dispatch(hl.dsp.focus({{ window = {} }}))",
                lua_string(&format!("address:{gone}"))
            ))
            .await
            .map_err(eval_err)
            .unwrap_err();
        for keep_pointer in [false, true] {
            let a = Action::from_call(
                "focus_window",
                &json!({"address": gone, "keep_pointer": keep_pointer}),
            )
            .unwrap();
            assert_eq!(run(&hypr, &a).await.unwrap_err(), single);
            assert_eq!(no_warps().await, before);
        }
    }

    #[test]
    fn look_options() {
        assert_eq!(
            parse_look_option(r#"{"option": "general:border_size", "int": 4, "set": true}"#),
            Some(json!(4))
        );
        assert_eq!(
            parse_look_option(r#"{"option": "general:gaps_out", "css": "4 4 4 4", "set": true}"#),
            Some(json!([4, 4, 4, 4]))
        );
        assert_eq!(
            parse_look_option(r#"{"css": "10 20"}"#),
            Some(json!([10, 20, 10, 20]))
        );
        assert_eq!(
            parse_look_option(r#"{"gradient": "aa625e51 0deg"}"#),
            Some(json!("#aa625e51"))
        );
        assert_eq!(
            parse_look_option(r#"{"gradient": "ffD79921 ff000000 45deg"}"#),
            Some(json!("#ffd79921"))
        );
        assert_eq!(parse_look_option(r#"{"gradient": "red"}"#), None);
        assert_eq!(
            parse_look_option(r#"{"option": "general:layout", "str": "scrolling"}"#),
            Some(json!("scrolling"))
        );
        assert_eq!(parse_look_option(r#"{"str": "a b"}"#), None);
        assert_eq!(parse_look_option(r#"{"css": "a b"}"#), None);
        assert_eq!(parse_look_option("not json"), None);
    }

    /// A fake request socket: answers each connection with `reply(request)` and records the
    /// requests.
    fn fake_hypr(
        reply: fn(&str) -> String,
    ) -> (
        tempfile::TempDir,
        HyprSocket,
        std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("hypr/sig");
        std::fs::create_dir_all(&sock).unwrap();
        let l = tokio::net::UnixListener::bind(sock.join(".socket.sock")).unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                let mut buf = vec![0u8; 8192];
                let n = s.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let _ = s.write_all(reply(&req).as_bytes()).await;
                log.lock().unwrap().push(req);
            }
        });
        let h = HyprSocket::new(Some(dir.path().to_path_buf()), Some("sig".into()));
        (dir, h, seen)
    }

    fn look_reply(opt: &str) -> String {
        match opt {
            "general:border_size" => r#"{"option": "general:border_size", "int": 2, "set": true }"#,
            "general:gaps_out" => r#"{"option": "general:gaps_out", "css": "4 8", "set": true }"#,
            "general:layout" => r#"{"option": "general:layout", "str": "scrolling", "set": true }"#,
            _ => "no such option",
        }
        .to_string()
    }

    fn batch_reply(req: &str) -> String {
        let Some(list) = req.strip_prefix("[[BATCH]]") else {
            return "unknown request".into();
        };
        list.split(';')
            .map(|r| look_reply(r.trim_start_matches("j/getoption ")))
            .collect::<Vec<_>>()
            .join(BATCH_SEPARATOR)
    }

    /// No batch support: one option per request.
    fn single_reply(req: &str) -> String {
        match req.strip_prefix("j/getoption ") {
            Some(opt) => look_reply(opt),
            None => "unknown request".into(),
        }
    }

    #[tokio::test]
    async fn window_look_in_one_batch_or_one_by_one() {
        let want = json!({"border": 2, "gaps_out": [4, 8, 4, 8], "layout": "scrolling"});
        let (_d, h, seen) = fake_hypr(batch_reply);
        assert_eq!(window_look_from(&h).await.unwrap(), want);
        assert_eq!(seen.lock().unwrap().len(), 1, "one connection");

        let (_d, h, seen) = fake_hypr(single_reply);
        assert_eq!(window_look_from(&h).await.unwrap(), want);
        assert_eq!(seen.lock().unwrap().len(), 1 + LOOK_OPTIONS.len());

        let gone = HyprSocket::new(Some("/nonexistent".into()), Some("sig".into()));
        assert_eq!(
            window_look_from(&gone).await.unwrap_err(),
            "Hyprland is not running"
        );
        assert_eq!(split_batch("a\n\n\nb", 2), Some(vec!["a", "b"]));
        assert_eq!(split_batch("a\n\n\nb", 3), None);
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
            ("column_width", json!({"address": A, "width": "full"})),
            (
                "resize",
                json!({"windows": [{"address": A, "w": 600, "h": 400}]}),
            ),
            (
                "columns",
                json!({"windows": [{"address": A, "fraction": 0.5}], "pin": {"address": A, "x": 16}}),
            ),
            ("move_column", json!({"address": A, "steps": -1})),
            ("scroll", json!({"address": A, "x": 16})),
            (
                "page_hiding",
                json!({"workspaces": [{"id": 1, "anchor": A, "windows": [{"address": A, "hide": [A]}]}],
                       "reset": [A]}),
            ),
            ("column_width", json!({"address": A, "width": "half"})),
            (
                "place",
                json!({"address": A, "x": -20, "y": 72, "w": 800, "h": 600}),
            ),
        ];
        let mut body = String::new();
        let mut actions: Vec<Action> = all
            .iter()
            .map(|(name, args)| Action::from_call(name, args).unwrap())
            .collect();
        actions.push(
            Action::from_call("focus_window", &json!({"address": A, "keep_pointer": true}))
                .unwrap(),
        );
        actions.push(
            Action::from_call(
                "column_width",
                &json!({"address": A, "width": "half", "keep_focus": true}),
            )
            .unwrap(),
        );
        for a in &actions {
            body.push_str(&format!(
                "hl.on(\"hyprland.start\", function()\n{}end)\n",
                script(a)
            ));
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
