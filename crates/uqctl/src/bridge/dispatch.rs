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
mod lua;
mod parse;
use lua::*;
mod script;
pub use script::*;
mod look;
pub use look::*;

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
    /// Give this window a scrolling layout column of its own (`promote`): a window moved with
    /// the pointer that Hyprland dropped into another window's column, where the shell showed it
    /// would get a place of its own. The window is focused (the pointer stays put).
    OwnColumn {
        address: String,
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
    /// [`Action::Place`] for a window that must float there: the terminal look=see>do_ keeps
    /// docked under its pill, made floating first in case it opened tiled.
    Dock {
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
mod tests;
