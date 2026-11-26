//! Each action as the Lua Hyprland runs: `hl.dispatch` calls on one window or workspace.

use super::*;

/// The rest of `ColumnWidth` for the (focused) window `address`, given its Hyprland fullscreen
/// state `fs` (0 none, 1 maximized, 2 fullscreen). Full: maximize it unless it already is. Half:
/// un-maximize it, then make its column half the screen. A fullscreen window is left alone.
pub(crate) fn column_lua(address: &str, full: bool, fs: i64) -> Vec<String> {
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

pub(crate) fn move_lua(address: &str, ws: &WsTarget) -> String {
    let ws = match ws {
        WsTarget::Id(n) => n.to_string(),
        WsTarget::Special(s) => lua_string(&format!("special:{s}")),
    };
    format!(
        "return hl.dispatch(hl.dsp.window.move({{ workspace = {ws}, follow = false, window = {} }}))",
        lua_string(&format!("address:{address}"))
    )
}

pub(crate) fn tag_lua(address: &str, tag: &str) -> String {
    format!(
        "return hl.dispatch(hl.dsp.window.tag({{ tag = {}, window = {} }}))",
        lua_string(tag),
        lua_string(&format!("address:{address}"))
    )
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
pub(crate) fn scroll_lua(address: &str, x: i64) -> String {
    format!(
        "  do local ok, w = pcall(hl.get_window, {})\n    \
         local dx = ok and w and w.at and ({x} - w.at.x) or 0\n    \
         if dx >= 1 or dx <= -1 then\n      \
         local m = fail(hl.dispatch(hl.dsp.layout(\"move \" .. math.floor(dx + 0.5)))); if m then return m end\n    \
         end\n  end\n",
        lua_string(&format!("address:{address}"))
    )
}

impl Action {
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
            Action::OwnColumn { address } => vec![
                d(format!("hl.dsp.focus({{ window = {} }})", win(address))),
                d("hl.dsp.layout(\"promote\")".to_string()),
            ],
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
            Action::Dock {
                address,
                x,
                y,
                w,
                h,
            } => vec![
                d(format!(
                    "hl.dsp.window.float({{ action = \"enable\", window = {} }})",
                    win(address)
                )),
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
            // Made Full for the policy, with focus to stay where it is: the window is read and
            // maximized where it is, with no hop of focus (across screens, a hop would show as
            // the other screen flashing as the focused one).
            Action::ColumnWidth {
                full: true,
                keep_focus: true,
                ..
            } => Vec::new(),
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
