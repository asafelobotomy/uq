//! The plan a previewed confirmation shows (`uq_api` 3; docs/settings-pages.md).
//!
//! A command with `preview = "<id>"` doesn't run until the person has seen what it would change.
//! Settings runs the preview command (a read-only command of the same page, with the same
//! arguments) and shows its answer under the `confirm` question. The answer is one JSON document:
//!
//! ```json
//! { "summary": "3 caches, 120 MB",
//!   "changes": [ { "group": "Caches", "label": "Firefox", "detail": "~/.cache/mozilla",
//!                  "bytes": 125829120, "reversible": true } ],
//!   "would_refuse": false, "reason": "" }
//! ```
//!
//! Everything is plain text and bounded here, whatever the program printed, so a page can't
//! be handed markup or a wall of text. `would_refuse: true` means Confirm is not offered.

use serde_json::{Map, Value, json};

use crate::desktop_entry::clean_display;

/// At most this many changes are shown; the rest are counted in `more`.
pub const MAX_CHANGES: usize = 500;
pub const MAX_GROUP: usize = 40;
pub const MAX_LABEL: usize = 120;
pub const MAX_DETAIL: usize = 300;
pub const MAX_SUMMARY: usize = 200;
pub const MAX_REASON: usize = 300;

/// A plan with no changes still has a verdict.
fn text(v: Option<&Value>, max: usize) -> String {
    v.and_then(Value::as_str)
        .map(|s| clean_display(s, max).trim().to_string())
        .unwrap_or_default()
}

/// Turn a preview command's output into the plan Settings shows:
/// `{summary, changes: [{group, label, detail, bytes, reversible}], more, would_refuse, reason}`.
/// `bytes` is a whole number or `null`; `reversible` is `true`, `false` or `null` (unknown).
pub fn normalize(output: &Value) -> Result<Value, String> {
    let Some(doc) = output.as_object() else {
        return Err("its answer isn't a plan".into());
    };
    let listed: &[Value] = match doc.get("changes") {
        None => &[],
        Some(Value::Array(a)) => a,
        Some(_) => return Err("its plan's changes aren't a list".into()),
    };
    let would_refuse = doc
        .get("would_refuse")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let reason = text(doc.get("reason"), MAX_REASON);
    let mut changes = Vec::new();
    for item in listed.iter().take(MAX_CHANGES) {
        let Some(c) = item.as_object() else {
            return Err("a change in its plan isn't an object".into());
        };
        let label = text(c.get("label"), MAX_LABEL);
        if label.is_empty() {
            return Err("a change in its plan has no label".into());
        }
        let mut one = Map::new();
        one.insert("group".into(), json!(text(c.get("group"), MAX_GROUP)));
        one.insert("label".into(), json!(label));
        one.insert("detail".into(), json!(text(c.get("detail"), MAX_DETAIL)));
        one.insert(
            "bytes".into(),
            c.get("bytes")
                .and_then(Value::as_u64)
                .map_or(Value::Null, |b| json!(b)),
        );
        one.insert(
            "reversible".into(),
            c.get("reversible")
                .and_then(Value::as_bool)
                .map_or(Value::Null, Value::Bool),
        );
        changes.push(Value::Object(one));
    }
    Ok(json!({
        "summary": text(doc.get("summary"), MAX_SUMMARY),
        "changes": changes,
        "more": listed.len().saturating_sub(MAX_CHANGES),
        "would_refuse": would_refuse,
        "reason": reason,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plan_is_cleaned_and_bounded() {
        let plan = normalize(&json!({
            "summary": "two\nthings",
            "changes": [
                {"group": "Caches", "label": "Firefox", "detail": "~/.cache", "bytes": 5, "reversible": true},
                {"label": "x\u{202e}y", "bytes": -1, "reversible": "yes"}
            ],
            "would_refuse": false
        }))
        .unwrap();
        assert_eq!(plan["summary"], "two things");
        assert_eq!(plan["changes"][0]["bytes"], 5);
        assert_eq!(plan["changes"][0]["reversible"], true);
        assert_eq!(plan["changes"][1]["label"], "x y");
        assert_eq!(plan["changes"][1]["group"], "");
        assert!(plan["changes"][1]["bytes"].is_null());
        assert!(plan["changes"][1]["reversible"].is_null());
        assert_eq!(plan["would_refuse"], false);
    }

    #[test]
    fn too_many_changes_are_counted() {
        let many: Vec<Value> = (0..MAX_CHANGES + 7)
            .map(|i| json!({"label": format!("c{i}")}))
            .collect();
        let plan = normalize(&json!({"changes": many})).unwrap();
        assert_eq!(plan["changes"].as_array().unwrap().len(), MAX_CHANGES);
        assert_eq!(plan["more"], 7);
    }

    #[test]
    fn a_refusal_keeps_its_reason() {
        let plan = normalize(&json!({"would_refuse": true, "reason": "Swap is in use"})).unwrap();
        assert_eq!(plan["would_refuse"], true);
        assert_eq!(plan["reason"], "Swap is in use");
        assert_eq!(plan["changes"], json!([]));
    }

    #[test]
    fn something_that_is_not_a_plan_is_an_error() {
        for bad in [
            json!("text"),
            json!([1]),
            json!({"changes": 3}),
            json!({"changes": [1]}),
            json!({"changes": [{"label": " "}]}),
        ] {
            assert!(normalize(&bad).is_err(), "{bad}");
        }
    }
}
