//! Putting an edited rule into its list, and checking a draft before it is sent.
//!
//! The editor says what a save means: an add (`replaces` is `None`) or a replace of the rule it
//! was opened on (`replaces` is that rule's id, or its workspace). A new workspace rule never
//! replaces one: `web` and `name:web` are the same workspace, and a second rule for it is
//! refused with a plain message.

use serde_json::{Map, Value, json};
use uq_core::rules::Rules;
use uq_core::settings::rule_selector;

use super::{RulesState, failure, key_of, list_mut, reason};

/// A workspace as uqd stores it, or the text as given when it can't be one (uqd then says why).
fn selector_of(s: &str) -> String {
    rule_selector(s).unwrap_or_else(|_| s.to_string())
}

fn same_key(kind: &str, a: &str, b: &str) -> bool {
    if kind == "workspace" {
        selector_of(a) == selector_of(b)
    } else {
        a == b
    }
}

/// Add `rule` at the end of its list (`replaces` is `None`), or put it in place of the rule
/// `replaces` names. Refuses a workspace that another rule already has.
pub(super) fn put(
    section: &mut Map<String, Value>,
    kind: &str,
    rule: Value,
    replaces: Option<&str>,
) -> zbus::Result<()> {
    let list = list_mut(section, kind)?;
    let at = match replaces {
        None => None,
        Some(old) => Some(
            list.iter()
                .position(|r| same_key(kind, key_of(kind, r), old))
                .ok_or_else(|| failure("that rule isn't there any more"))?,
        ),
    };
    if kind == "workspace" {
        let key = key_of(kind, &rule);
        let clash = list
            .iter()
            .enumerate()
            .any(|(i, r)| Some(i) != at && same_key(kind, key_of(kind, r), key));
        if clash {
            let shown = selector_of(key);
            return Err(failure(format!(
                "Workspace {} already has a rule; edit that one.",
                shown.strip_prefix("name:").unwrap_or(&shown)
            )));
        }
    }
    match at {
        Some(i) => list[i] = rule,
        None => list.push(rule),
    }
    Ok(())
}

/// The text of a refusal made here (not by uqd).
fn plain(e: &zbus::Error) -> String {
    match e {
        zbus::Error::Failure(text) => text.clone(),
        other => other.to_string(),
    }
}

impl RulesState {
    /// Whether `draft` can be saved as `put` would save it, against the rules last read from
    /// uqd: `{valid, problems: [plain text]}`. Synchronous and pure, so the editor can say what
    /// is wrong (a panel or workspace rule has no live preview) and keep the draft. uqd still
    /// checks the write; this is only an early warning.
    pub fn check(&self, kind: &str, draft: &Value, replaces: Option<String>) -> Value {
        let mut section = self.last.lock().unwrap().clone();
        let Some(obj) = section.as_object_mut() else {
            return json!({"valid": false, "problems": ["the rules haven't been read yet"]});
        };
        let problem = match put(obj, kind, draft.clone(), replaces.as_deref()) {
            Err(e) => Some(plain(&e)),
            Ok(()) => Rules::from_section(&section, &Rules::default())
                .err()
                .map(|e| reason(&e)),
        };
        match problem {
            Some(p) => json!({"valid": false, "problems": [p]}),
            None => json!({"valid": true, "problems": []}),
        }
    }
}
