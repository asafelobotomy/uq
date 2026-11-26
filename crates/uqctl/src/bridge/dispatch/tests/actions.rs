//! Reading actions and their Lua.

use super::*;

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
    let a =
        Action::from_call("focus_window", &json!({"address": A, "keep_pointer": true})).unwrap();
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
    assert!(Action::from_call("focus_window", &json!({"address": A, "keep_pointer": 1})).is_err());
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
            format!("return hl.dispatch(hl.dsp.window.move({{ x = -20, y = 72, window = {w} }}))"),
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
fn dock_floats_then_places() {
    let w = r#""address:0x55d0c0ffee""#;
    assert_eq!(
        lua(
            "dock",
            json!({"address": A, "x": 320, "y": 230, "w": 640, "h": 220})
        ),
        [
            format!(
                "return hl.dispatch(hl.dsp.window.float({{ action = \"enable\", window = {w} }}))"
            ),
            format!(
                "return hl.dispatch(hl.dsp.window.resize({{ x = 640, y = 220, window = {w} }}))"
            ),
            format!("return hl.dispatch(hl.dsp.window.move({{ x = 320, y = 230, window = {w} }}))"),
        ]
    );
    assert!(
        Action::from_call(
            "dock",
            &json!({"address": A, "x": 0, "y": 0, "w": 0, "h": 1})
        )
        .is_err()
    );
    assert!(
        Action::from_call(
            "dock",
            &json!({"address": "nope", "x": 0, "y": 0, "w": 1, "h": 1})
        )
        .is_err()
    );
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
