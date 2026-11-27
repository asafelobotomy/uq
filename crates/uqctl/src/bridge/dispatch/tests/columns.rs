//! Columns, resizing and scrolling.

use super::*;

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
fn own_column() {
    let l = lua("own_column", json!({"address": A}));
    assert_eq!(l.len(), 2, "focus, then promote");
    assert!(l[0].contains("hl.dsp.focus"));
    assert!(l[1].contains("hl.dsp.layout(\"promote\")"));
    assert!(
        script(&Action::from_call("own_column", &json!({"address": A})).unwrap())
            .contains("no_warps"),
        "the pointer stays put"
    );
    assert!(Action::from_call("own_column", &json!({})).is_err());
    assert!(Action::from_call("own_column", &json!({"address": "nope"})).is_err());
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
        s.contains("pcall(hl.get_window, \"address:0x55d0c0ffee\")") && s.contains("16 - w.at.x"),
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
