use super::summarize;
use serde_json::{Value, json};

fn s(b: &Value, a: &Value) -> String {
    summarize("autostart", b, a, &|_, _| None)
}

fn files(choices: Value) -> Value {
    json!({"files": choices, "added": []})
}

fn added(entries: Value) -> Value {
    json!({"files": [], "added": entries})
}

fn cmd(id: &str, name: &str, enabled: bool, delay: Value) -> Value {
    json!({"id": id, "name": name, "argv": ["x"], "enabled": enabled, "delay": delay})
}

#[test]
fn autostart_summary() {
    let none = files(json!([]));
    let off = files(json!([{"id": "nm-applet.desktop", "enabled": false, "delay": null}]));
    let on = files(json!([{"id": "nm-applet.desktop", "enabled": true, "delay": null}]));
    assert_eq!(s(&none, &off), "Startup app nm-applet turned off");
    assert_eq!(s(&off, &on), "Startup app nm-applet turned on");
    assert_eq!(
        s(&off, &none),
        "Startup app nm-applet set back to its default"
    );
    let slow = files(json!([{"id": "nm-applet.desktop", "enabled": true, "delay": 30}]));
    assert_eq!(s(&on, &slow), "Start delay for nm-applet set to 30 seconds");
    assert_eq!(s(&slow, &on), "Start delay for nm-applet removed");

    let empty = added(json!([]));
    let sync = added(json!([cmd("a-0001", "Syncthing", true, json!(null))]));
    assert_eq!(s(&empty, &sync), "Startup app Syncthing added");
    assert_eq!(s(&sync, &empty), "Startup app Syncthing removed");
    let sync_off = added(json!([cmd("a-0001", "Syncthing", false, json!(null))]));
    assert_eq!(s(&sync, &sync_off), "Startup app Syncthing turned off");
    assert_eq!(s(&sync_off, &sync), "Startup app Syncthing turned on");
    let sync_slow = added(json!([cmd("a-0001", "Syncthing", true, json!(5))]));
    assert_eq!(
        s(&sync, &sync_slow),
        "Start delay for Syncthing set to 5 seconds"
    );
    let mut renamed = sync.clone();
    renamed["added"][0]["argv"] = json!(["y"]);
    assert_eq!(s(&sync, &renamed), "Startup app Syncthing changed");

    let many = added(json!([
        cmd("a-0001", "A", true, json!(null)),
        cmd("a-0002", "B", true, json!(null)),
        cmd("a-0003", "C", true, json!(null)),
        cmd("a-0004", "D", true, json!(null))
    ]));
    assert_eq!(
        s(&empty, &many),
        "Startup app A added, Startup app B added, Startup app C added, and 1 more"
    );
    let reordered = added(json!([
        cmd("a-0002", "B", true, json!(null)),
        cmd("a-0001", "A", true, json!(null)),
        cmd("a-0003", "C", true, json!(null)),
        cmd("a-0004", "D", true, json!(null))
    ]));
    assert_eq!(s(&many, &reordered), "Startup apps reordered");
    assert_eq!(s(&many, &many), "Startup apps");
}

#[test]
fn autostart_summaries_are_plain() {
    let a = json!({
        "files": [{"id": "nm-applet.desktop", "enabled": false, "delay": 3},
                  {"id": "../evil", "enabled": false, "delay": null}],
        "added": [cmd("a-0001", "Sync\u{202E}\nthing", true, json!(null)),
                  {"id": "a-0002", "name": "App", "app": "x.desktop", "enabled": true,
                   "delay": null}]
    });
    let b = files(json!([{"id": "blueman.desktop", "enabled": true, "delay": null}]));
    let c = json!({});
    let hostile = files(json!([{"id": "[x]{y}.desktop", "enabled": false, "delay": null}]));
    for (x, y) in [
        (&a, &b),
        (&b, &a),
        (&c, &a),
        (&a, &c),
        (&c, &c),
        (&c, &hostile),
    ] {
        let line = s(x, y);
        assert!(!line.is_empty());
        assert!(
            !line.contains(['{', '[', '\n', '\u{202E}'])
                && !line.contains(".desktop")
                && !line.contains("autostart.")
                && !line.contains("argv"),
            "{line}"
        );
    }
}
