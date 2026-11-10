//! The one table of Hyprland Lua rule names.
//!
//! Every Lua key and value spelling that uQ's rules use lives here. The validator, the renderer
//! and the Window rules page (through its `fields` catalogue) all read from these tables, so a
//! name Hyprland rejects is fixed by changing one row.
//!
//! The names were read from Hyprland's own source at two tags:
//! - `v0.56.2` (`efb50993780079460b0cbed1363e2166a2de1d9f`)
//! - `v0.57.0` (`0aa6745f705f2c4b0a5004eb019ef76e5bbdb426`)
//!
//! Files read at both tags:
//! - `src/config/lua/bindings/LuaBindingsConfigRules.cpp`
//! - `src/config/lua/bindings/LuaBindingsInternal.hpp`
//! - `src/desktop/rule/Rule.cpp`
//! - `src/desktop/rule/windowRule/WindowRule.cpp`
//! - `src/desktop/rule/windowRule/WindowRuleApplicator.cpp`
//! - `src/desktop/view/Window.cpp`
//! - `src/config/shared/workspace/WorkspaceRuleManager.cpp`
//!
//! Not confirmed from source (live-check and `--verify-config` items): the layer `animation`
//! style names, whether the `tag` effect wants `+name`, whether `gaps_in`/`gaps_out` take a plain
//! integer, and whether named rules survive `hyprctl reload`.

/// The kind of value a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bool,
    /// Text: a pattern, a selector, an output name or a tag name, depending on the field.
    Text,
    /// Two whole numbers, `[a, b]`.
    Pair,
    /// A number with two places; ranges use hundredths.
    Float,
    Int,
    Choice(&'static [&'static str]),
}

/// Whether Hyprland applies an effect only when a window opens or also on reload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    Static,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// The name in uQ's state.
    pub uq: &'static str,
    /// The name in Hyprland's Lua.
    pub lua: &'static str,
    pub kind: Kind,
    /// The plain label the page shows.
    pub label: &'static str,
    /// Lowest value, where one applies (pairs and numbers; floats in hundredths).
    pub min: Option<i64>,
    /// Highest value, where one applies.
    pub max: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effect {
    pub field: Field,
    pub when: When,
}

pub const IDLE_INHIBIT: [&str; 4] = ["none", "always", "focus", "fullscreen"];
pub const LAYER_ANIMATIONS: [&str; 3] = ["fade", "slide", "popin"];
/// 0.56.2's limit (0.57 allows 100).
pub const ROUNDING_MAX: u32 = 20;
/// Added to a `workspace` effect's selector for "silent".
pub const SILENT_SUFFIX: &str = " silent";
/// Put before a tag name in the `tag` effect: set, not toggle.
pub const TAG_SET_PREFIX: &str = "+";
/// Every rendered rule is named `uq:<id>`.
pub const RULE_NAME_PREFIX: &str = "uq:";

const fn f(
    uq: &'static str,
    lua: &'static str,
    kind: Kind,
    label: &'static str,
    min: Option<i64>,
    max: Option<i64>,
) -> Field {
    Field {
        uq,
        lua,
        kind,
        label,
        min,
        max,
    }
}

const fn st(field: Field) -> Effect {
    Effect {
        field,
        when: When::Static,
    }
}

const fn dy(field: Field) -> Effect {
    Effect {
        field,
        when: When::Dynamic,
    }
}

/// Window match keys. Source: `Rule.cpp` (`class`, `title`, `initial_class`, `initial_title`,
/// `float`, `tag`, `xwayland`, `fullscreen`, `pin`, `modal`, `workspace`). `tag` is a tag name,
/// not a pattern (`CTagKeeper::isTagged`).
pub const MATCH_KEYS: [Field; 11] = [
    f("class", "class", Kind::Text, "App ID", None, None),
    f("title", "title", Kind::Text, "Window title", None, None),
    f(
        "initial_class",
        "initial_class",
        Kind::Text,
        "App ID when it opened",
        None,
        None,
    ),
    f(
        "initial_title",
        "initial_title",
        Kind::Text,
        "Title when it opened",
        None,
        None,
    ),
    f("tag", "tag", Kind::Text, "Tag", None, None),
    f(
        "xwayland",
        "xwayland",
        Kind::Bool,
        "Older X11 app",
        None,
        None,
    ),
    f("floating", "float", Kind::Bool, "Floating", None, None),
    f(
        "fullscreen",
        "fullscreen",
        Kind::Bool,
        "Fullscreen",
        None,
        None,
    ),
    f("pinned", "pin", Kind::Bool, "Pinned", None, None),
    f("modal", "modal", Kind::Bool, "Dialog", None, None),
    f(
        "workspace",
        "workspace",
        Kind::Text,
        "Workspace",
        None,
        None,
    ),
];

/// Panel (layer) match keys. Source: `Rule.cpp` (`namespace`).
pub const LAYER_MATCH_KEYS: [Field; 1] = [f(
    "namespace",
    "namespace",
    Kind::Text,
    "Panel name",
    None,
    None,
)];

/// Window effects. Source: `LuaBindingsInternal.hpp` (`WINDOW_RULE_EFFECT_DESCS`); static or
/// dynamic by `WindowRuleApplicator.cpp` (`applyStaticRule`). `silent` is a modifier of
/// `workspace` (`Window.cpp`), rendered with `SILENT_SUFFIX`, so it has no row.
pub const WINDOW_EFFECTS: [Effect; 20] = [
    st(f("float", "float", Kind::Bool, "Float", None, None)),
    st(f("tile", "tile", Kind::Bool, "Tile", None, None)),
    st(f(
        "fullscreen",
        "fullscreen",
        Kind::Bool,
        "Fullscreen",
        None,
        None,
    )),
    st(f(
        "maximize",
        "maximize",
        Kind::Bool,
        "Maximize",
        None,
        None,
    )),
    st(f(
        "workspace",
        "workspace",
        Kind::Text,
        "Open on workspace",
        None,
        None,
    )),
    st(f(
        "monitor",
        "monitor",
        Kind::Text,
        "Open on screen",
        None,
        None,
    )),
    st(f("size", "size", Kind::Pair, "Size", Some(1), Some(16384))),
    st(f(
        "move",
        "move",
        Kind::Pair,
        "Position",
        Some(-16384),
        Some(16384),
    )),
    st(f("center", "center", Kind::Bool, "Center", None, None)),
    st(f(
        "pin",
        "pin",
        Kind::Bool,
        "Keep on top of every workspace",
        None,
        None,
    )),
    st(f(
        "no_initial_focus",
        "no_initial_focus",
        Kind::Bool,
        "Don't focus when it opens",
        None,
        None,
    )),
    dy(f(
        "opacity",
        "opacity",
        Kind::Float,
        "Opacity",
        Some(10),
        Some(100),
    )),
    dy(f("no_blur", "no_blur", Kind::Bool, "No blur", None, None)),
    dy(f(
        "no_shadow",
        "no_shadow",
        Kind::Bool,
        "No shadow",
        None,
        None,
    )),
    dy(f(
        "no_anim",
        "no_anim",
        Kind::Bool,
        "No animations",
        None,
        None,
    )),
    dy(f(
        "rounding",
        "rounding",
        Kind::Int,
        "Corner rounding",
        Some(0),
        Some(ROUNDING_MAX as i64),
    )),
    dy(f(
        "border_size",
        "border_size",
        Kind::Int,
        "Border size",
        Some(0),
        Some(20),
    )),
    dy(f(
        "idle_inhibit",
        "idle_inhibit",
        Kind::Choice(&IDLE_INHIBIT),
        "Keep the screen awake",
        None,
        None,
    )),
    dy(f(
        "keep_aspect_ratio",
        "keep_aspect_ratio",
        Kind::Bool,
        "Keep the shape when resized",
        None,
        None,
    )),
    dy(f("tag", "tag", Kind::Text, "Add a tag", None, None)),
];

/// Panel effects. Source: `LuaBindingsConfigRules.cpp` (`LAYER_RULE_EFFECT_DESCS`).
pub const LAYER_EFFECTS: [Field; 4] = [
    f("blur", "blur", Kind::Bool, "Blur", None, None),
    f(
        "ignore_alpha",
        "ignore_alpha",
        Kind::Float,
        "Blur ignores faint pixels below",
        Some(0),
        Some(100),
    ),
    f(
        "no_anim",
        "no_anim",
        Kind::Bool,
        "No animations",
        None,
        None,
    ),
    f(
        "animation",
        "animation",
        Kind::Choice(&LAYER_ANIMATIONS),
        "Animation style",
        None,
        None,
    ),
];

/// Workspace rule fields. Source: `LuaBindingsConfigRules.cpp` (`WORKSPACE_RULE_FIELDS`). The
/// layout is not here; it comes from `layouts`.
pub const WORKSPACE_FIELDS: [Field; 6] = [
    f("monitor", "monitor", Kind::Text, "Screen", None, None),
    f(
        "default",
        "default",
        Kind::Bool,
        "Default workspace",
        None,
        None,
    ),
    f(
        "persistent",
        "persistent",
        Kind::Bool,
        "Keep when empty",
        None,
        None,
    ),
    f(
        "gaps_in",
        "gaps_in",
        Kind::Int,
        "Gap between windows",
        Some(0),
        Some(100),
    ),
    f(
        "gaps_out",
        "gaps_out",
        Kind::Int,
        "Gap at the screen edge",
        Some(0),
        Some(100),
    ),
    f(
        "border_size",
        "border_size",
        Kind::Int,
        "Border size",
        Some(0),
        Some(20),
    ),
];

pub fn match_key(uq: &str) -> Option<&'static Field> {
    let t: &'static [Field] = &MATCH_KEYS;
    t.iter().find(|k| k.uq == uq)
}

pub fn window_effect(uq: &str) -> Option<&'static Effect> {
    let t: &'static [Effect] = &WINDOW_EFFECTS;
    t.iter().find(|e| e.field.uq == uq)
}

pub fn is_static(uq: &str) -> bool {
    window_effect(uq).is_some_and(|e| e.when == When::Static)
}

#[cfg(test)]
mod tests {
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
}
