//! The `fields` catalogue (F6): every rule field the editor offers, built from `rules::hypr`
//! so the page never copies the name table.

use serde_json::{Value, json};
use uq_core::rules::hypr::{self, Field, Kind, When};

/// The editor's kind name for a field. The text match keys that hold a pattern say `pattern`.
fn kind_name(f: &Field, pattern: bool) -> &'static str {
    match f.kind {
        Kind::Bool => "bool",
        Kind::Text if pattern => "pattern",
        Kind::Text => "text",
        Kind::Pair => "pair",
        Kind::Float => "float",
        Kind::Int => "int",
        Kind::Choice(_) => "choice",
    }
}

fn item(f: &Field, pattern: bool, is_static: bool) -> Value {
    let choices: Vec<&str> = match f.kind {
        Kind::Choice(c) => c.to_vec(),
        _ => Vec::new(),
    };
    json!({
        "key": f.uq,
        "label": f.label,
        "kind": kind_name(f, pattern),
        "static": is_static,
        "min": f.min,
        "max": f.max,
        "choices": choices,
        "lua": f.lua,
    })
}

/// `{match, layer_match, window_effects, layer_effects, workspace}`, each a list of
/// `{key, label, kind, static, min, max, choices, lua}`.
pub fn fields() -> Value {
    let is_pattern = |f: &Field| {
        matches!(
            f.uq,
            "class" | "title" | "initial_class" | "initial_title" | "namespace"
        )
    };
    let plain =
        |fs: &[Field]| -> Vec<Value> { fs.iter().map(|f| item(f, is_pattern(f), false)).collect() };
    let window_effects: Vec<Value> = hypr::WINDOW_EFFECTS
        .iter()
        .map(|e| item(&e.field, false, e.when == When::Static))
        .collect();
    json!({
        "match": plain(&hypr::MATCH_KEYS),
        "layer_match": plain(&hypr::LAYER_MATCH_KEYS),
        "window_effects": window_effects,
        "layer_effects": plain(&hypr::LAYER_EFFECTS),
        "workspace": plain(&hypr::WORKSPACE_FIELDS),
    })
}
