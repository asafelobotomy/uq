use super::*;
use std::collections::HashSet;

fn unique(fields: &[Field]) {
    let uq: HashSet<_> = fields.iter().map(|f| f.uq).collect();
    let lua: HashSet<_> = fields.iter().map(|f| f.lua).collect();
    assert_eq!(uq.len(), fields.len(), "repeated uq name");
    assert_eq!(lua.len(), fields.len(), "repeated lua name");
}

fn effects(when: When) -> Vec<&'static str> {
    WINDOW_EFFECTS
        .iter()
        .filter(|e| e.when == when)
        .map(|e| e.field.uq)
        .collect()
}

#[test]
fn tables_have_unique_names() {
    unique(&MATCH_KEYS);
    unique(&LAYER_MATCH_KEYS);
    let w: Vec<Field> = WINDOW_EFFECTS.iter().map(|e| e.field).collect();
    unique(&w);
    unique(&LAYER_EFFECTS);
    unique(&WORKSPACE_FIELDS);
}

#[test]
fn spec_effect_lists_are_covered() {
    let stat = [
        "float",
        "tile",
        "fullscreen",
        "maximize",
        "workspace",
        "monitor",
        "size",
        "move",
        "center",
        "pin",
        "no_initial_focus",
    ];
    let dynamic = [
        "opacity",
        "no_blur",
        "no_shadow",
        "no_anim",
        "rounding",
        "border_size",
        "idle_inhibit",
        "keep_aspect_ratio",
        "tag",
    ];
    let layer = ["blur", "ignore_alpha", "no_anim", "animation"];
    assert_eq!(effects(When::Static), stat);
    assert_eq!(effects(When::Dynamic), dynamic);
    let l: Vec<_> = LAYER_EFFECTS.iter().map(|f| f.uq).collect();
    assert_eq!(l, layer);
    assert!(window_effect("silent").is_none());
    assert!(is_static("move") && !is_static("opacity") && !is_static("nope"));
}

#[test]
fn spec_match_names_map_to_lua() {
    let spec = [
        "class",
        "title",
        "initial_class",
        "initial_title",
        "tag",
        "xwayland",
        "floating",
        "fullscreen",
        "pinned",
        "workspace",
    ];
    for uq in spec {
        assert!(match_key(uq).is_some(), "{uq}");
    }
    for k in MATCH_KEYS {
        let want = match k.uq {
            "floating" => "float",
            "pinned" => "pin",
            other => other,
        };
        assert_eq!(k.lua, want);
    }
    // The plan's one extra key.
    assert_eq!(MATCH_KEYS.len(), spec.len() + 1);
    assert_eq!(match_key("modal").unwrap().lua, "modal");
}

#[test]
fn labels_are_plain() {
    let mut all: Vec<Field> = MATCH_KEYS.to_vec();
    all.extend(LAYER_MATCH_KEYS);
    all.extend(WINDOW_EFFECTS.iter().map(|e| e.field));
    all.extend(LAYER_EFFECTS);
    all.extend(WORKSPACE_FIELDS);
    for f in all {
        assert!(!f.label.is_empty());
        let l = f.label.to_lowercase();
        for bad in ["regex", "xwayland", "namespace", "class", "hl."] {
            assert!(!l.contains(bad), "{} holds {bad}", f.label);
        }
    }
}
