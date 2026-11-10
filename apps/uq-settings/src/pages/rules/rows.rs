//! The list rows of the snapshot: each rule as the page shows it, with its source from
//! `Explain("rules")`.

use serde_json::{Value, json};
use uq_core::rules::{
    Compiled, Rules, Verdict, WindowInfo, effects_text, has_static, layer_effects_text, match_text,
    workspace_text,
};

use super::RulesState;

/// `{source, line, path}` of a row of `Explain("rules")`: the path is `user.lua`'s, for the
/// page's **Open user.lua**.
pub(super) fn source_of(explained: &Value, key: &str) -> (String, Value, Value) {
    let row = &explained[key];
    let source = row["source"].as_str().unwrap_or("uq").to_string();
    let detail = |k: &str| row["detail"].get(k).cloned().unwrap_or(Value::Null);
    (source, detail("line"), detail("path"))
}

/// The message of a row whose last write was rejected (`source` is `error`), or null.
fn message_of(explained: &Value, key: &str) -> Value {
    explained[key]["detail"]
        .get("message")
        .cloned()
        .unwrap_or(Value::Null)
}

/// The name of the profile whose switch the row's rule holds, or `""`.
fn profile_of(explained: &Value, key: &str) -> String {
    let row = &explained[key];
    match row["source"].as_str() {
        Some("profile") => row["detail"]["name"].as_str().unwrap_or("").to_string(),
        _ => String::new(),
    }
}

impl RulesState {
    pub(super) fn rows(
        rules: &Rules,
        windows: &[WindowInfo],
        explained: &Value,
        layouts: &Value,
    ) -> Value {
        let window: Vec<Value> = rules
            .window
            .iter()
            .map(|r| {
                let key = format!("window.{}", r.id);
                let (source, line, path) = source_of(explained, &key);
                let profile = profile_of(explained, &key);
                let now = Compiled::new(&r.matches)
                    .map(|c| windows.iter().filter(|w| c.test(w) != Verdict::No).count())
                    .unwrap_or(0);
                json!({"raw": r, "id": r.id, "name": r.name, "enabled": r.enabled,
                    "match_text": match_text(&r.matches),
                    "effects_text": effects_text(&r.effects),
                    "static": has_static(&r.effects),
                    "source": source, "line": line, "path": path, "profile": profile,
                    "message": message_of(explained, &key), "matches_now": now})
            })
            .collect();
        let layer: Vec<Value> = rules
            .layer
            .iter()
            .map(|r| {
                let key = format!("layer.{}", r.id);
                let (source, line, path) = source_of(explained, &key);
                let profile = profile_of(explained, &key);
                json!({"raw": r, "id": r.id, "name": r.name, "enabled": r.enabled,
                    "namespace": r.matches.namespace,
                    "effects_text": layer_effects_text(&r.effects),
                    "source": source, "line": line, "path": path, "profile": profile,
                    "message": message_of(explained, &key)})
            })
            .collect();
        let workspace: Vec<Value> = rules
            .workspace
            .iter()
            .map(|r| {
                let key = format!("workspace.{}", r.workspace);
                let (source, line, path) = source_of(explained, &key);
                let layout = layouts["workspaces"][&r.workspace].as_str().unwrap_or("");
                json!({"raw": r, "workspace": r.workspace, "layout": layout,
                    "fields_text": workspace_text(r),
                    "source": source, "line": line, "path": path,
                    "message": message_of(explained, &key)})
            })
            .collect();
        json!({"window": window, "layer": layer, "workspace": workspace})
    }
}
