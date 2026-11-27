//! Which window is at a point.

use super::*;

#[test]
fn window_at_prefers_floating_over_tiled() {
    let tiled = client("tiled", 1, [0, 0], [200, 200], |v| {
        v["focusHistoryID"] = json!(0)
    });
    let float = client("float", 1, [20, 20], [100, 100], |v| {
        v["floating"] = json!(true);
        v["focusHistoryID"] = json!(3);
    });
    let float2 = client("float2", 1, [20, 20], [100, 100], |v| {
        v["floating"] = json!(true);
        v["focusHistoryID"] = json!(1);
    });
    assert_eq!(
        at(vec![tiled.clone(), float.clone()], &mons()).as_deref(),
        Some("float")
    );
    assert_eq!(
        at(vec![tiled, float, float2], &mons()).as_deref(),
        Some("float2")
    );
}

#[test]
fn window_at_fullscreen_beats_floating() {
    let fs = client("fs", 1, [0, 0], [200, 200], |v| v["fullscreen"] = json!(1));
    let float = client("float", 1, [20, 20], [100, 100], |v| {
        v["floating"] = json!(true)
    });
    assert_eq!(at(vec![float, fs], &mons()).as_deref(), Some("fs"));
}

#[test]
fn window_at_edges_and_misses() {
    let c = client("a", 1, [50, 50], [10, 10], |_| {});
    assert_eq!(at(vec![c.clone()], &mons()).as_deref(), Some("a"));
    let c2 = client("a", 1, [40, 40], [10, 10], |_| {});
    assert_eq!(at(vec![c2], &mons()), None);
    let mut hidden = c.clone();
    hidden["hidden"] = json!(true);
    assert_eq!(at(vec![hidden], &mons()), None);
    let mut unmapped = c;
    unmapped["mapped"] = json!(false);
    assert_eq!(at(vec![unmapped], &mons()), None);
}

#[test]
fn window_at_ignores_other_workspaces() {
    let other = client("other", 2, [0, 0], [200, 200], |_| {});
    assert_eq!(at(vec![other.clone()], &mons()), None);
    let pinned = client("pin", 2, [0, 0], [200, 200], |v| {
        v["pinned"] = json!(true);
        v["floating"] = json!(true);
    });
    assert_eq!(at(vec![other, pinned], &mons()).as_deref(), Some("pin"));
}

#[test]
fn window_at_sees_special_workspace() {
    let monitors = json!([{"id": 0, "disabled": false, "activeWorkspace": {"id": 1, "name": "1"},
        "specialWorkspace": {"id": -98, "name": "special:magic"}}])
    .to_string();
    let tiled = client("tiled", 1, [0, 0], [200, 200], |_| {});
    let sp = client("sp", -98, [0, 0], [200, 200], |v| {
        v["focusHistoryID"] = json!(9)
    });
    let float = client("float", 1, [20, 20], [100, 100], |v| {
        v["floating"] = json!(true)
    });
    assert_eq!(
        at(vec![tiled, float, sp.clone()], &monitors).as_deref(),
        Some("sp")
    );
    // the special workspace is closed
    assert_eq!(at(vec![sp], &mons()), None);
}

#[test]
fn window_at_none_on_empty_desktop() {
    assert_eq!(at(vec![], &mons()), None);
    assert!(window_at("[]", "[]", CUR).is_none());
    assert!(window_at("junk", "junk", "junk").is_none());
    assert!(window_at("[]", &mons(), "{}").is_none());
}

#[test]
fn window_at_skips_disabled_monitors() {
    let monitors = json!([
        {"id": 0, "disabled": false, "activeWorkspace": {"id": 1, "name": "1"},
         "specialWorkspace": {"id": 0, "name": ""}},
        {"id": 1, "disabled": true, "activeWorkspace": {"id": 7, "name": "7"},
         "specialWorkspace": {"id": 0, "name": ""}}
    ])
    .to_string();
    let stale = client("stale", 7, [0, 0], [200, 200], |_| {});
    assert_eq!(at(vec![stale], &monitors), None);
}
