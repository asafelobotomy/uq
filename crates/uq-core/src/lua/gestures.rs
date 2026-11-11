//! The touchpad gesture lines of `generated.lua`: one `hl.gesture({ … })` per enabled gesture
//! and the `gestures` option table inside `hl.config`. Every name comes from
//! `gestures::hypr`; the only free text is a special workspace's name, which passes its
//! charset rule and `lua_string`.

use super::lua_string;
use crate::gestures::hypr::{self, ActionLua};
use crate::gestures::{self, Action, Gesture, SwipeOptions};
use crate::settings::InputSettings;
use std::fmt::Write as _;

/// One `hl.gesture({ … })` call. A shell action is a function that dispatches the global
/// shortcut: at Hyprland 0.57 `hl.dsp.global(…)` is an object `hl.gesture` refuses as an
/// action, while a function works at 0.56 and 0.57.
pub fn emit_gesture(g: &Gesture) -> String {
    let f = &hypr::FIELDS;
    let a = hypr::action(g.action);
    let action = match a.lua {
        ActionLua::Word(w) => lua_string(w),
        ActionLua::Global(name) => format!(
            "function() hl.dispatch(hl.dsp.global({})) end",
            lua_string(&format!("uq:{name}"))
        ),
    };
    let mut out = format!(
        "hl.gesture({{ {} = {}, {} = {}, {} = {action}",
        f.fingers,
        g.fingers,
        f.direction,
        lua_string(hypr::direction(g.direction).lua),
        f.action,
    );
    if g.action == Action::Special {
        let _ = write!(
            out,
            ", {} = {}",
            f.workspace_name,
            lua_string(g.special_name())
        );
    }
    if !g.modifier.is_empty() {
        let _ = write!(
            out,
            ", {} = {}",
            f.mods,
            lua_string(&g.modifier.to_string())
        );
    }
    out.push_str(" })");
    out
}

/// The `-- Touchpad gestures` block: enabled gestures in list order (uQ's own two when the
/// list was never set). Nothing when none is on.
pub fn emit_gestures(i: &InputSettings) -> String {
    let lines: Vec<String> = gestures::effective(&i.gestures)
        .iter()
        .filter(|g| g.enabled)
        .map(emit_gesture)
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!("\n-- Touchpad gestures\n{}\n", lines.join("\n"))
}

/// The `gestures = { … }` table inside the main `hl.config`: only the options that are set.
pub(super) fn emit_swipe(out: &mut String, s: &SwipeOptions) {
    let lua = |uq: &str| {
        hypr::SWIPE_OPTIONS
            .iter()
            .find(|(n, _)| *n == uq)
            .map_or("", |(_, l)| *l)
    };
    let mut f = Vec::new();
    if let Some(d) = s.distance {
        f.push(format!("{} = {d}", lua("distance")));
    }
    if let Some(b) = s.invert {
        f.push(format!("{} = {b}", lua("invert")));
    }
    if let Some(b) = s.create_new {
        f.push(format!("{} = {b}", lua("create_new")));
    }
    if let Some(r) = s.cancel_ratio {
        f.push(format!("{} = {r:?}", lua("cancel_ratio")));
    }
    if !f.is_empty() {
        let _ = writeln!(out, "    gestures = {{ {} }},", f.join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gestures::{Direction, Mods};

    fn g(fingers: u8, direction: Direction, action: Action) -> Gesture {
        Gesture {
            fingers,
            direction,
            action,
            modifier: Mods::default(),
            workspace: None,
            enabled: true,
        }
    }

    #[test]
    fn render_shapes() {
        assert_eq!(
            emit_gesture(&g(3, Direction::Horizontal, Action::Workspace)),
            r#"hl.gesture({ fingers = 3, direction = "horizontal", action = "workspace" })"#
        );
        // A shell action is a function: a bare `hl.dsp.global(…)` fails at Hyprland 0.57.
        assert_eq!(
            emit_gesture(&g(4, Direction::Up, Action::Spotlight)),
            r#"hl.gesture({ fingers = 4, direction = "up", action = function() hl.dispatch(hl.dsp.global("uq:spotlight")) end })"#
        );
        assert_eq!(
            emit_gesture(&g(4, Direction::Left, Action::QuickSettings)),
            r#"hl.gesture({ fingers = 4, direction = "left", action = function() hl.dispatch(hl.dsp.global("uq:quick-settings")) end })"#
        );
        let mut special = g(3, Direction::Down, Action::Special);
        assert_eq!(
            emit_gesture(&special),
            r#"hl.gesture({ fingers = 3, direction = "down", action = "special", workspace_name = "magic" })"#
        );
        special.workspace = Some("my-notes".into());
        assert!(emit_gesture(&special).contains(r#"workspace_name = "my-notes""#));
        let mut pinch = g(4, Direction::PinchIn, Action::Float);
        pinch.modifier = Mods::parse("SHIFT SUPER").unwrap();
        assert_eq!(
            emit_gesture(&pinch),
            r#"hl.gesture({ fingers = 4, direction = "pinchin", action = "float", mods = "SUPER SHIFT" })"#
        );
        // `float` and `fullscreen` render without `mode`.
        assert!(!emit_gesture(&g(3, Direction::Up, Action::Fullscreen)).contains("mode"));
    }

    #[test]
    fn block_lists_enabled_gestures_in_order() {
        // Never set: uQ's own two.
        let out = emit_gestures(&InputSettings::default());
        assert!(out.starts_with("\n-- Touchpad gestures\n"), "{out}");
        assert_eq!(out.matches("hl.gesture(").count(), 2);
        assert!(out.find("horizontal") < out.find("uq:spotlight"));
        // A disabled gesture renders nothing; nothing enabled renders no block.
        let mut off = g(3, Direction::Left, Action::Close);
        off.enabled = false;
        let mut i = InputSettings {
            gestures: Some(vec![off.clone(), g(5, Direction::Right, Action::Float)]),
            ..InputSettings::default()
        };
        let out = emit_gestures(&i);
        assert_eq!(out.matches("hl.gesture(").count(), 1, "{out}");
        assert!(out.contains(r#"direction = "right""#));
        i.gestures = Some(vec![off]);
        assert_eq!(emit_gestures(&i), "");
        i.gestures = Some(Vec::new());
        assert_eq!(emit_gestures(&i), "");
    }

    #[test]
    fn swipe_table_has_only_set_keys() {
        let mut out = String::new();
        emit_swipe(&mut out, &SwipeOptions::default());
        assert_eq!(out, "");
        emit_swipe(
            &mut out,
            &SwipeOptions {
                distance: Some(400),
                invert: Some(false),
                create_new: Some(true),
                cancel_ratio: Some(0.4),
            },
        );
        assert_eq!(
            out,
            "    gestures = { workspace_swipe_distance = 400, workspace_swipe_invert = false, workspace_swipe_create_new = true, workspace_swipe_cancel_ratio = 0.4 },\n"
        );
        let mut one = String::new();
        emit_swipe(
            &mut one,
            &SwipeOptions {
                cancel_ratio: Some(1.0),
                ..SwipeOptions::default()
            },
        );
        assert_eq!(
            one,
            "    gestures = { workspace_swipe_cancel_ratio = 1.0 },\n"
        );
    }
}
