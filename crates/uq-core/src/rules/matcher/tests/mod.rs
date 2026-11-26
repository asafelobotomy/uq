use super::*;
use crate::rules::hypr;
use crate::rules::{
    LayerEffects, WindowEffects, WorkspaceRule, effects_text, has_static, layer_effects_text,
    match_text, workspace_text,
};
use serde_json::json;

fn win(class: &str) -> WindowInfo {
    WindowInfo {
        class: class.into(),
        title: "T".into(),
        initial_class: class.into(),
        initial_title: "T".into(),
        workspace_id: 1,
        workspace_name: "1".into(),
        ..Default::default()
    }
}

fn m(f: impl FnOnce(&mut WindowMatch)) -> Compiled {
    let mut x = WindowMatch::default();
    f(&mut x);
    Compiled::new(&x).unwrap()
}

fn client(class: &str, ws: i64, at: [i64; 2], size: [i64; 2], f: impl FnOnce(&mut Value)) -> Value {
    let mut v = json!({
        "mapped": true, "hidden": false, "at": at, "size": size,
        "workspace": {"id": ws, "name": ws.to_string()}, "floating": false,
        "class": class, "title": "t", "initialClass": class, "initialTitle": "t",
        "xwayland": false, "pinned": false, "fullscreen": 0, "tags": [],
        "focusHistoryID": 5
    });
    f(&mut v);
    v
}

fn mons() -> String {
    json!([{"id": 0, "disabled": false, "activeWorkspace": {"id": 1, "name": "1"},
            "specialWorkspace": {"id": 0, "name": ""}}])
    .to_string()
}

const CUR: &str = r#"{"x": 50, "y": 50}"#;

fn at(clients: Vec<Value>, monitors: &str) -> Option<String> {
    window_at(&json!(clients).to_string(), monitors, CUR).map(|w| w.class)
}

mod matching;
mod window_at;
