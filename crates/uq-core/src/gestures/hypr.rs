//! The one name table for touchpad gestures: every Hyprland spelling (directions, actions,
//! `hl.gesture` field names, `gestures:` option names) lives here, so a live check that
//! fails is fixed in one row. Checked against Hyprland v0.56.2 and v0.57.0
//! (`LuaBindingsConfigRules.cpp` `hlGesture`, `TrackpadGestures.cpp` `dirForString`,
//! `ConfigValues.cpp`). Hyprland spells every Lua option with `_`, never `-`.

use super::{Action, Direction};

/// A direction as uQ stores it, as Hyprland spells it, and in words.
pub struct DirName {
    pub uq: Direction,
    pub lua: &'static str,
    /// What the person does: "swipe left", "pinch in".
    pub label: &'static str,
}

pub const DIRECTIONS: [DirName; 8] = [
    DirName {
        uq: Direction::Horizontal,
        lua: "horizontal",
        label: "swipe sideways",
    },
    DirName {
        uq: Direction::Vertical,
        lua: "vertical",
        label: "swipe up or down",
    },
    DirName {
        uq: Direction::Left,
        lua: "left",
        label: "swipe left",
    },
    DirName {
        uq: Direction::Right,
        lua: "right",
        label: "swipe right",
    },
    DirName {
        uq: Direction::Up,
        lua: "up",
        label: "swipe up",
    },
    DirName {
        uq: Direction::Down,
        lua: "down",
        label: "swipe down",
    },
    // Hyprland's `dirForString` has `pinchin` and `pinchout`; there is no `pinch_in`.
    DirName {
        uq: Direction::PinchIn,
        lua: "pinchin",
        label: "pinch in",
    },
    DirName {
        uq: Direction::PinchOut,
        lua: "pinchout",
        label: "pinch out",
    },
];

/// How an action is written into `hl.gesture`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionLua {
    /// A string action Hyprland knows: `action = "workspace"`.
    Word(&'static str),
    /// A shell global shortcut (one of `keybinds::hypr::SHELL_NAMES`), run from a function:
    /// `action = function() hl.dispatch(hl.dsp.global("uq:<name>")) end`. At Hyprland 0.57
    /// `hl.dsp.global(…)` is a dispatcher object, which `hl.gesture` refuses as an action; a
    /// function works at both 0.56 and 0.57.
    Global(&'static str),
}

const SWIPES: &[Direction] = &[
    Direction::Horizontal,
    Direction::Vertical,
    Direction::Left,
    Direction::Right,
    Direction::Up,
    Direction::Down,
];
const ANY: &[Direction] = &[
    Direction::Horizontal,
    Direction::Vertical,
    Direction::Left,
    Direction::Right,
    Direction::Up,
    Direction::Down,
    Direction::PinchIn,
    Direction::PinchOut,
];
const SIDEWAYS_OR_UP_DOWN: &[Direction] = &[Direction::Horizontal, Direction::Vertical];

pub struct ActionName {
    pub uq: Action,
    pub lua: ActionLua,
    pub allowed: &'static [Direction],
    pub label: &'static str,
    /// The refusal when the direction doesn't suit the action; empty when any does.
    pub only_with: &'static str,
}

pub const ACTIONS: [ActionName; 9] = [
    ActionName {
        uq: Action::Workspace,
        lua: ActionLua::Word("workspace"),
        allowed: SIDEWAYS_OR_UP_DOWN,
        label: "Switch workspace",
        only_with: "Switching workspace works only with a sideways or up-and-down swipe",
    },
    ActionName {
        uq: Action::Move,
        lua: ActionLua::Word("move"),
        allowed: SWIPES,
        label: "Move the window",
        only_with: "Moving the window works only with a swipe, not a pinch",
    },
    ActionName {
        uq: Action::Resize,
        lua: ActionLua::Word("resize"),
        allowed: SWIPES,
        label: "Resize the window",
        only_with: "Resizing the window works only with a swipe, not a pinch",
    },
    ActionName {
        uq: Action::Special,
        lua: ActionLua::Word("special"),
        allowed: ANY,
        label: "Show or hide the scratch workspace",
        only_with: "",
    },
    ActionName {
        uq: Action::Close,
        lua: ActionLua::Word("close"),
        allowed: ANY,
        label: "Close the window",
        only_with: "",
    },
    ActionName {
        uq: Action::Fullscreen,
        lua: ActionLua::Word("fullscreen"),
        allowed: ANY,
        label: "Make the window full screen",
        only_with: "",
    },
    ActionName {
        uq: Action::Float,
        lua: ActionLua::Word("float"),
        allowed: ANY,
        label: "Float the window",
        only_with: "",
    },
    ActionName {
        uq: Action::Spotlight,
        lua: ActionLua::Global("spotlight"),
        allowed: ANY,
        label: "Open Spotlight",
        only_with: "",
    },
    ActionName {
        uq: Action::QuickSettings,
        lua: ActionLua::Global("quick-settings"),
        allowed: ANY,
        label: "Open quick settings",
        only_with: "",
    },
];

/// The swipe options: uQ's name and the `gestures:` option Hyprland reads.
pub const SWIPE_OPTIONS: [(&str, &str); 4] = [
    ("distance", "workspace_swipe_distance"),
    ("invert", "workspace_swipe_invert"),
    ("create_new", "workspace_swipe_create_new"),
    ("cancel_ratio", "workspace_swipe_cancel_ratio"),
];

/// The fields of an `hl.gesture({ … })` table.
pub struct GestureFields {
    pub fingers: &'static str,
    pub direction: &'static str,
    pub action: &'static str,
    pub mods: &'static str,
    pub workspace_name: &'static str,
}

pub const FIELDS: GestureFields = GestureFields {
    fingers: "fingers",
    direction: "direction",
    action: "action",
    mods: "mods",
    workspace_name: "workspace_name",
};

/// The `hl.gesture` fields Hyprland reads, for the Lua mock: any other key is silently ignored.
pub const ALL_FIELDS: [&str; 9] = [
    "fingers",
    "direction",
    "action",
    "mods",
    "scale",
    "disable_inhibit",
    "workspace_name",
    "mode",
    "zoom_level",
];

pub fn direction(d: Direction) -> &'static DirName {
    DIRECTIONS
        .iter()
        .find(|n| n.uq == d)
        .expect("every direction has a row")
}

pub fn action(a: Action) -> &'static ActionName {
    ACTIONS
        .iter()
        .find(|n| n.uq == a)
        .expect("every action has a row")
}
