use super::hypr::{ACTIONS, ActionLua, DIRECTIONS, FIELDS, SWIPE_OPTIONS};
use super::*;
use crate::keybinds::hypr::SHELL_NAMES;
use serde_json::json;

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

fn off(mut g: Gesture) -> Gesture {
    g.enabled = false;
    g
}

#[test]
fn no_lua_name_has_a_hyphen() {
    // Hyprland spells every Lua option and field with `_`; a `-` is silently another name.
    let mut names: Vec<&str> = DIRECTIONS.iter().map(|d| d.lua).collect();
    names.extend(SWIPE_OPTIONS.iter().map(|(_, lua)| *lua));
    names.extend([
        FIELDS.fingers,
        FIELDS.direction,
        FIELDS.action,
        FIELDS.mods,
        FIELDS.workspace_name,
    ]);
    names.extend(hypr::ALL_FIELDS);
    for a in &ACTIONS {
        if let ActionLua::Word(w) = a.lua {
            names.push(w);
        }
    }
    for n in names {
        assert!(!n.contains('-'), "{n}");
    }
    // The uQ names of the swipe options match their JSON keys.
    let keys: Vec<&str> = SWIPE_OPTIONS.iter().map(|(uq, _)| *uq).collect();
    assert_eq!(keys, SwipeOptions::FIELDS);
}

#[test]
fn every_shell_action_names_a_registered_shortcut() {
    let mut globals = 0;
    for a in &ACTIONS {
        if let ActionLua::Global(name) = a.lua {
            globals += 1;
            assert!(SHELL_NAMES.contains(&name), "{name} isn't a shell shortcut");
        }
    }
    assert_eq!(globals, 2);
    // Every direction and action has exactly one row, and the serde names agree.
    for (i, d) in DIRECTIONS.iter().enumerate() {
        assert!(DIRECTIONS.iter().skip(i + 1).all(|o| o.uq != d.uq));
    }
    assert_eq!(
        serde_json::to_value(Action::QuickSettings).unwrap(),
        json!("uq:quick-settings")
    );
    assert_eq!(
        serde_json::to_value(Direction::PinchIn).unwrap(),
        json!("pinch_in")
    );
}

#[test]
fn pinch_renders_as_hyprland_spells_it() {
    let lua = |d: Direction| hypr::direction(d).lua;
    assert_eq!(lua(Direction::PinchIn), "pinchin");
    assert_eq!(lua(Direction::PinchOut), "pinchout");
    assert_eq!(lua(Direction::Horizontal), "horizontal");
}

#[test]
fn conflicts_follow_hyprland() {
    let sw = |d, a| g(3, d, a);
    // An axis and one of its directions clash, in both orders.
    for pair in [
        [
            sw(Direction::Horizontal, Action::Workspace),
            sw(Direction::Left, Action::Close),
        ],
        [
            sw(Direction::Left, Action::Close),
            sw(Direction::Horizontal, Action::Workspace),
        ],
        [
            sw(Direction::Horizontal, Action::Workspace),
            sw(Direction::Horizontal, Action::Close),
        ],
    ] {
        assert!(validate(&pair).is_err(), "{pair:?}");
    }
    assert!(
        validate(&[
            sw(Direction::Left, Action::Close),
            sw(Direction::Right, Action::Float)
        ])
        .is_ok()
    );
    // A modifier or another finger count is another trigger.
    let mut with_super = g(4, Direction::Up, Action::Close);
    with_super.modifier = Mods::parse("SUPER").unwrap();
    assert!(validate(&[g(4, Direction::Up, Action::Spotlight), with_super.clone()]).is_ok());
    assert!(
        validate(&[
            g(4, Direction::Up, Action::Spotlight),
            g(3, Direction::Up, Action::Close)
        ])
        .is_ok()
    );
    let mut super_shift = with_super.clone();
    super_shift.modifier = Mods::parse("SUPER SHIFT").unwrap();
    assert!(validate(&[with_super.clone(), super_shift]).is_ok());
    // Pinching in and out are different.
    assert!(
        validate(&[
            sw(Direction::PinchIn, Action::Close),
            sw(Direction::PinchOut, Action::Float)
        ])
        .is_ok()
    );
    assert!(
        validate(&[
            sw(Direction::PinchIn, Action::Close),
            sw(Direction::PinchIn, Action::Float)
        ])
        .is_err()
    );
    // A disabled duplicate never conflicts.
    assert!(
        validate(&[
            sw(Direction::Left, Action::Close),
            off(sw(Direction::Left, Action::Float))
        ])
        .is_ok()
    );
    assert!(
        validate(&[
            off(sw(Direction::Left, Action::Close)),
            sw(Direction::Left, Action::Float)
        ])
        .is_ok()
    );
    // The message names both actions and the shared trigger, later one first; the key is the
    // later gesture's direction.
    let list = [
        sw(Direction::Horizontal, Action::Workspace),
        sw(Direction::Left, Action::Close),
    ];
    let err = validate(&list).unwrap_err();
    let SettingError::Invalid { key, reason } = err else {
        panic!()
    };
    assert_eq!(key, "input.gestures[1].direction");
    assert_eq!(
        reason,
        "“Close the window” and “Switch workspace” both use 3 fingers, swipe left."
    );
    // A horizontal swipe meets left and right once each, but is one conflict per pair.
    let three = [
        sw(Direction::Left, Action::Close),
        sw(Direction::Right, Action::Float),
        sw(Direction::Horizontal, Action::Workspace),
    ];
    let clashes = clashes_of(&three);
    assert_eq!(clashes.len(), 2);
    assert_eq!(clashes[0].combo_text, "3 fingers, swipe left");
    assert_eq!(
        clashes_of(&[
            sw(Direction::Horizontal, Action::Close),
            sw(Direction::Horizontal, Action::Float)
        ])
        .len(),
        1
    );
}

#[test]
fn direction_must_suit_action() {
    let err = validate(&[g(3, Direction::Up, Action::Workspace)]).unwrap_err();
    assert_eq!(
        err.to_string(),
        "invalid value for input.gestures[0].direction: Switching workspace works only with a sideways or up-and-down swipe"
    );
    assert!(validate(&[g(3, Direction::Vertical, Action::Workspace)]).is_ok());
    assert!(validate(&[g(3, Direction::PinchIn, Action::Move)]).is_err());
    assert!(validate(&[g(3, Direction::Left, Action::Resize)]).is_ok());
    assert!(validate(&[g(3, Direction::PinchOut, Action::Special)]).is_ok());
}

#[test]
fn fingers_three_to_five() {
    for n in [2, 6, 0, 9] {
        assert!(
            validate(&[g(n, Direction::Left, Action::Close)]).is_err(),
            "{n}"
        );
    }
    for n in 3..=5 {
        assert!(
            validate(&[g(n, Direction::Left, Action::Close)]).is_ok(),
            "{n}"
        );
    }
}

#[test]
fn too_many_gestures() {
    // 24 distinct triggers are fine; the 25th is refused for the count.
    let mut list = Vec::new();
    'all: for fingers in 3..=5 {
        for d in [
            Direction::Left,
            Direction::Right,
            Direction::Up,
            Direction::Down,
            Direction::PinchIn,
            Direction::PinchOut,
        ] {
            for m in ["", "SUPER", "ALT"] {
                if list.len() == 25 {
                    break 'all;
                }
                let mut x = g(fingers, d, Action::Close);
                x.modifier = Mods::parse(m).unwrap();
                list.push(x);
            }
        }
    }
    assert!(validate(&list[..24]).is_ok());
    let err = validate(&list[..25]).unwrap_err().to_string();
    assert!(err.contains("at most 24"), "{err}");
}

#[test]
fn special_workspace_name_refused_or_escaped() {
    let special = |name: &str| {
        let mut x = g(3, Direction::Down, Action::Special);
        x.workspace = Some(name.into());
        x
    };
    for bad in ["a b", "x\"y", "", &"a".repeat(33), "é"] {
        assert!(validate(&[special(bad)]).is_err(), "{bad:?}");
    }
    assert!(validate(&[special("magic")]).is_ok());
    assert!(validate(&[special("a-b_9")]).is_ok());
    // Only the scratch gesture takes a name.
    let mut close = g(3, Direction::Down, Action::Close);
    close.workspace = Some("magic".into());
    assert!(validate(&[close]).is_err());
    assert_eq!(
        g(3, Direction::Down, Action::Special).special_name(),
        "magic"
    );
}

#[test]
fn defaults_are_the_two_spec_gestures() {
    let d = defaults();
    assert_eq!(d.len(), 2);
    assert_eq!(
        (d[0].fingers, d[0].direction, d[0].action),
        (3, Direction::Horizontal, Action::Workspace)
    );
    assert_eq!(
        (d[1].fingers, d[1].direction, d[1].action),
        (4, Direction::Up, Action::Spotlight)
    );
    assert!(d.iter().all(|g| g.enabled));
    assert!(validate(&d).is_ok());
}

#[test]
fn null_means_defaults_empty_means_none() {
    assert_eq!(effective(&None).as_ref(), defaults().as_slice());
    assert!(effective(&Some(Vec::new())).is_empty());
    let one = Some(vec![g(5, Direction::Left, Action::Close)]);
    assert_eq!(effective(&one).len(), 1);
}

#[test]
fn swipe_ranges() {
    let ok = |s: SwipeOptions| s.validate().is_ok();
    assert!(ok(SwipeOptions::default()));
    assert!(ok(SwipeOptions {
        distance: Some(0),
        ..Default::default()
    }));
    assert!(ok(SwipeOptions {
        distance: Some(2000),
        ..Default::default()
    }));
    assert!(!ok(SwipeOptions {
        distance: Some(2001),
        ..Default::default()
    }));
    assert!(!ok(SwipeOptions {
        cancel_ratio: Some(1.01),
        ..Default::default()
    }));
    assert!(!ok(SwipeOptions {
        cancel_ratio: Some(-0.1),
        ..Default::default()
    }));
    assert!(!ok(SwipeOptions {
        cancel_ratio: Some(f64::NAN),
        ..Default::default()
    }));
    let r = SwipeOptions {
        cancel_ratio: Some(0.555),
        ..Default::default()
    }
    .normalised();
    assert_eq!(r.cancel_ratio, Some(0.56));
}

#[test]
fn modifiers_are_strict() {
    assert_eq!(
        Mods::parse("super shift").unwrap().to_string(),
        "SUPER SHIFT"
    );
    assert_eq!(Mods::parse("ALT + CTRL").unwrap().to_string(), "CTRL ALT");
    assert_eq!(Mods::parse("WIN").unwrap(), Mods::parse("SUPER").unwrap());
    assert_eq!(Mods::parse("").unwrap(), Mods::default());
    for bad in ["CAPS", "HYPER", "SUPERR", "SUPER,ALT"] {
        assert!(Mods::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn describe_is_plain() {
    assert_eq!(
        g(3, Direction::Horizontal, Action::Workspace).describe(),
        "3 fingers, swipe sideways"
    );
    assert_eq!(
        g(3, Direction::PinchIn, Action::Close).describe(),
        "3 fingers, pinch in"
    );
    let mut x = g(4, Direction::Left, Action::Close);
    x.modifier = Mods::parse("SUPER").unwrap();
    assert_eq!(x.describe(), "SUPER + 4 fingers, swipe left");
}

#[test]
fn strict_entries_refuse_what_the_loader_would_drop() {
    let at = |v: Value| Gesture::from_value("input.gestures[0]", &v);
    assert!(at(json!({"fingers": 3, "direction": "left", "action": "close"})).is_ok());
    for bad in [
        json!({"fingers": 3, "direction": "left", "action": "close", "extra": 1}),
        json!({"fingers": "3", "direction": "left", "action": "close"}),
        json!({"fingers": 3, "direction": "diagonal", "action": "close"}),
        json!({"fingers": 3, "direction": "left", "action": "reboot"}),
        json!({"fingers": 3, "direction": "left", "action": "close", "modifier": "CAPS"}),
        json!({"fingers": 3, "direction": "left", "action": "close", "enabled": "no"}),
        json!({"fingers": 3, "direction": "left"}),
        json!(7),
    ] {
        assert!(at(bad.clone()).is_err(), "{bad}");
    }
    let g = at(json!({"fingers": 3, "direction": "left", "action": "close", "modifier": "super"}))
        .unwrap();
    assert_eq!(g.modifier.to_string(), "SUPER");
    assert!(g.enabled, "enabled defaults to on");
}

#[test]
fn summary_lines_are_plain() {
    let defaults_json = json!({});
    let with = |v: Value| json!({ "gestures": v });
    assert_eq!(
        summary(&defaults_json, &defaults_json),
        Vec::<String>::new()
    );
    let added = with(json!([
        {"fingers": 3, "direction": "horizontal", "action": "workspace", "enabled": true},
        {"fingers": 4, "direction": "up", "action": "uq:spotlight", "enabled": true},
        {"fingers": 3, "direction": "down", "action": "special", "enabled": true}]));
    assert_eq!(
        summary(&defaults_json, &added),
        ["Touchpad gesture added: 3 fingers, swipe down"]
    );
    let removed = with(json!([
        {"fingers": 3, "direction": "horizontal", "action": "workspace", "enabled": true}]));
    assert_eq!(
        summary(&defaults_json, &removed),
        ["Touchpad gesture removed: 4 fingers, swipe up"]
    );
    let off = with(json!([
        {"fingers": 3, "direction": "horizontal", "action": "workspace", "enabled": true},
        {"fingers": 4, "direction": "up", "action": "uq:spotlight", "enabled": false}]));
    assert_eq!(
        summary(&defaults_json, &off),
        ["Touchpad gesture turned off: 4 fingers, swipe up"]
    );
    assert_eq!(
        summary(&off, &defaults_json),
        ["Touchpad gestures back to uQ's defaults"]
    );
    for line in summary(&defaults_json, &added)
        .iter()
        .chain(&summary(&defaults_json, &off))
    {
        assert!(!line.contains(['{', '[', '"']), "{line}");
        assert!(
            !line.contains("uq:") && !line.contains("pinch_in"),
            "{line}"
        );
    }
}
