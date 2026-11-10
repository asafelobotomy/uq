//! The list rows of the snapshot: each rule as the page shows it, with its source from
//! `Explain("rules")`.

use serde_json::{Value, json};
use uq_core::rules::{
    Compiled, Rules, Verdict, WindowInfo, effects_text, has_static, layer_effects_text, match_text,
    workspace_text,
};

use super::RulesState;

/// `{source, line}` of a row of `Explain("rules")`.
pub(super) fn source_of(explained: &Value, key: &str) -> (String, Value) {
    let row = &explained[key];
    let source = row["source"].as_str().unwrap_or("uq").to_string();
    (
        source,
        row["detail"].get("line").cloned().unwrap_or(Value::Null),
    )
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
                let (source, line) = source_of(explained, &format!("window.{}", r.id));
                let now = Compiled::new(&r.matches)
                    .map(|c| windows.iter().filter(|w| c.test(w) != Verdict::No).count())
                    .unwrap_or(0);
                json!({"id": r.id, "name": r.name, "enabled": r.enabled,
                    "match_text": match_text(&r.matches),
                    "effects_text": effects_text(&r.effects),
                    "static": has_static(&r.effects),
                    "source": source, "line": line, "matches_now": now})
            })
            .collect();
        let layer: Vec<Value> = rules
            .layer
            .iter()
            .map(|r| {
                let (source, line) = source_of(explained, &format!("layer.{}", r.id));
                json!({"id": r.id, "name": r.name, "enabled": r.enabled,
                    "namespace": r.matches.namespace,
                    "effects_text": layer_effects_text(&r.effects),
                    "source": source, "line": line})
            })
            .collect();
        let workspace: Vec<Value> = rules
            .workspace
            .iter()
            .map(|r| {
                let (source, line) = source_of(explained, &format!("workspace.{}", r.workspace));
                let layout = layouts["workspaces"][&r.workspace].as_str().unwrap_or("");
                json!({"workspace": r.workspace, "layout": layout,
                    "fields_text": workspace_text(r),
                    "source": source, "line": line})
            })
            .collect();
        json!({"window": window, "layer": layer, "workspace": workspace})
    }
}
