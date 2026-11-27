//! Restore last session, conditions and commands at logout.

use super::*;

#[test]
fn session_fields_round_trip() {
    let a = ok(json!({
        "restore": true,
        "when": {"syncthing.desktop": "network", "a-1f2e": "power"},
        "logout": [{"name": "Sync notes", "argv": ["rsync", "-a", "notes", "nas:"]}],
    }));
    assert!(a.restore);
    assert_eq!(a.when["syncthing.desktop"], When::Network);
    assert_eq!(a.when["a-1f2e"], When::Power);
    assert_eq!(a.logout.len(), 1);
    assert!(valid_added_id(a.logout[0].id()), "an id is made");
    let back = ok(a.section());
    assert_eq!(back, a);
    let loaded: Autostart = serde_json::from_value(a.section()).unwrap();
    assert_eq!(loaded, a);
}

#[test]
fn session_fields_are_checked() {
    assert!(refused(json!({"restore": "yes"})).contains("on or off"));
    assert!(refused(json!({"when": {"x.desktop": "sunny"}})).contains("network or power"));
    assert!(refused(json!({"when": {"../x": "network"}})).contains("file name"));
    assert!(
        refused(json!({"logout": [{"name": "A", "app": "a.desktop"}]})).contains("only commands")
    );
    assert!(
        refused(json!({"logout": [{"name": "A", "argv": ["x"], "delay": 5}]})).contains("no delay")
    );
    let many: Vec<_> = (0..21)
        .map(|i| json!({"name": format!("c{i}"), "argv": ["x"]}))
        .collect();
    assert!(refused(json!({ "logout": many })).contains("20"));
}

#[test]
fn ids_are_shared_between_added_and_logout() {
    let r = refused(json!({
        "added": [{"id": "a-0001", "name": "A", "argv": ["a"]}],
        "logout": [{"id": "a-0001", "name": "B", "argv": ["b"]}],
    }));
    assert!(r.contains("another entry has this id"), "{r}");
}

#[test]
fn a_bad_condition_is_dropped_at_load_and_the_rest_kept() {
    let v = json!({"when": {"x.desktop": "network", "y.desktop": "nope"}, "restore": true});
    let a: Autostart = serde_json::from_value(v).unwrap();
    assert!(a.restore);
    assert_eq!(a.when.len(), 1);
}

#[test]
fn history_names_the_session_changes() {
    let before = json!({});
    let after = json!({"restore": true, "when": {"syncthing.desktop": "network"},
        "logout": [{"id": "a-0002", "name": "Sync", "argv": ["s"]}]});
    let line = summary(&before, &after);
    assert!(
        line.contains("Startup app syncthing waits for the network"),
        "{line}"
    );
    assert!(line.contains("Restore last session turned on"), "{line}");
    assert!(line.contains("Command at logout Sync added"), "{line}");
    assert_eq!(
        summary(
            &after,
            &json!({"restore": true, "logout": [{"id": "a-0002", "name": "Sync", "argv": ["s"]}]})
        ),
        "Startup app syncthing starts without waiting"
    );
}
