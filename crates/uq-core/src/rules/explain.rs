//! `Explain("rules")`: where each rule's live state comes from. Pure: uqd reads `user.lua` and
//! the last rejection and calls this; the app's fake uqd calls it too.

use super::Rules;
use crate::settings::provenance::{find_rule_in_user_lua, find_workspace_rule_in_user_lua};
use serde_json::{Map, Value, json};

/// One row per rule, keyed `window.<id>`, `layer.<id>` or `workspace.<selector>`, each
/// `{source, live, stored, detail}`. `source` is `error` (the last write was rejected),
/// `user_lua` (a line of `user.lua` names the rule) or `uq`. After a rejection there is also a
/// `"rules"` row, so a rejected write that added the only rule still says so.
pub fn explain(
    rules: &Rules,
    user_lua: Option<&str>,
    rejected: Option<&str>,
    user_lua_path: &str,
) -> Value {
    let row = |found: Option<u32>| -> Value {
        let (source, detail) = match (rejected, found) {
            (Some(message), _) => ("error", json!({ "message": message })),
            (None, Some(line)) => ("user_lua", json!({ "line": line, "path": user_lua_path })),
            (None, None) => ("uq", json!({})),
        };
        json!({ "source": source, "live": null, "stored": null, "detail": detail })
    };
    let lua = |f: &dyn Fn(&str) -> Option<u32>| user_lua.and_then(f);
    let mut out = Map::new();
    if rejected.is_some() {
        out.insert("rules".into(), row(None));
    }
    for r in &rules.window {
        let found = lua(&|t| find_rule_in_user_lua(t, &r.id));
        out.insert(format!("window.{}", r.id), row(found));
    }
    for r in &rules.layer {
        let found = lua(&|t| find_rule_in_user_lua(t, &r.id));
        out.insert(format!("layer.{}", r.id), row(found));
    }
    for r in &rules.workspace {
        let found = lua(&|t| find_workspace_rule_in_user_lua(t, &r.workspace));
        out.insert(format!("workspace.{}", r.workspace), row(found));
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Rules {
        serde_json::from_value(json!({
            "window": [{"id": "r-0001", "name": "A", "match": {"class": "a"},
                "effects": {"float": true}}],
            "layer": [{"id": "r-0002", "name": "B", "match": {"namespace": "n"},
                "effects": {"blur": true}}],
            "workspace": [{"workspace": "3", "persistent": true}],
        }))
        .unwrap()
    }

    #[test]
    fn rows_per_rule_and_kind() {
        let v = explain(&rules(), None, None, "/u.lua");
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["layer.r-0002", "window.r-0001", "workspace.3"]);
        for k in keys {
            assert_eq!(v[k]["source"], "uq");
            assert_eq!(v[k]["live"], Value::Null);
            assert_eq!(v[k]["stored"], Value::Null);
        }
    }

    #[test]
    fn user_lua_override_has_line_and_path() {
        let lua = "-- x\nhl.window_rule({ name = \"uq:r-0001\", enabled = false })\n\
            hl.workspace_rule({ workspace = \"3\" })\n";
        let v = explain(&rules(), Some(lua), None, "/home/u/user.lua");
        assert_eq!(v["window.r-0001"]["source"], "user_lua");
        assert_eq!(v["window.r-0001"]["detail"]["line"], 2);
        assert_eq!(v["window.r-0001"]["detail"]["path"], "/home/u/user.lua");
        assert_eq!(v["workspace.3"]["detail"]["line"], 3);
        assert_eq!(v["layer.r-0002"]["source"], "uq");
    }

    #[test]
    fn rejected_marks_every_row_as_error() {
        let lua = "hl.window_rule({ name = \"uq:r-0001\" })\n";
        let v = explain(&rules(), Some(lua), Some("bad"), "/u.lua");
        for k in ["rules", "window.r-0001", "layer.r-0002", "workspace.3"] {
            assert_eq!(v[k]["source"], "error", "{k}");
            assert_eq!(v[k]["detail"], json!({"message": "bad"}));
        }
    }

    #[test]
    fn rejected_adds_a_section_row_even_with_no_rules() {
        let v = explain(&Rules::default(), None, Some("bad"), "/u.lua");
        assert_eq!(
            v,
            json!({"rules": {"source": "error", "live": null, "stored": null,
                "detail": {"message": "bad"}}})
        );
    }
}
