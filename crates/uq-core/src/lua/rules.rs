//! Window, panel and workspace rules as Lua calls.
//!
//! Every Lua key comes from `rules::hypr`, every string goes through [`lua_string`], and a rule's
//! name never reaches the output: only `uq:<id>` does. Each call is one line.

use super::{Layout, lua_string, workspace_selector};
use crate::rules::hypr::{
    self, LAYER_EFFECTS, MATCH_KEYS, RULE_NAME_PREFIX, SILENT_SUFFIX, TAG_SET_PREFIX,
    WINDOW_EFFECTS, WORKSPACE_FIELDS,
};
use crate::rules::{
    LayerEffects, LayerRule, WindowEffects, WindowMatch, WindowRule, WorkspaceRule,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

fn pair<T: std::fmt::Display>(p: [T; 2]) -> String {
    format!("{{ {}, {} }}", p[0], p[1])
}

/// A bool as `true` / `false`.
fn lb(b: bool) -> String {
    b.to_string()
}

/// Rounded to two places; `Debug` keeps a `.0` on whole numbers.
fn float2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn match_value(m: &WindowMatch, uq: &str) -> Option<String> {
    let text = |v: &Option<String>| v.as_deref().map(lua_string);
    match uq {
        "class" => text(&m.class),
        "title" => text(&m.title),
        "initial_class" => text(&m.initial_class),
        "initial_title" => text(&m.initial_title),
        "tag" => text(&m.tag),
        "workspace" => m
            .workspace
            .as_deref()
            .map(|w| lua_string(&workspace_selector(w))),
        "xwayland" => m.xwayland.map(lb),
        "floating" => m.floating.map(lb),
        "fullscreen" => m.fullscreen.map(lb),
        "pinned" => m.pinned.map(lb),
        "modal" => m.modal.map(lb),
        _ => None,
    }
}

fn effect_value(e: &WindowEffects, uq: &str) -> Option<String> {
    match uq {
        "float" => e.float.map(lb),
        "tile" => e.tile.map(lb),
        "fullscreen" => e.fullscreen.map(lb),
        "maximize" => e.maximize.map(lb),
        "center" => e.center.map(lb),
        "pin" => e.pin.map(lb),
        "no_initial_focus" => e.no_initial_focus.map(lb),
        "no_blur" => e.no_blur.map(lb),
        "no_shadow" => e.no_shadow.map(lb),
        "no_anim" => e.no_anim.map(lb),
        "keep_aspect_ratio" => e.keep_aspect_ratio.map(lb),
        "workspace" => e.workspace.as_deref().map(|w| {
            let mut s = workspace_selector(w);
            if e.silent == Some(true) {
                s.push_str(SILENT_SUFFIX);
            }
            lua_string(&s)
        }),
        "monitor" => e.monitor.as_deref().map(lua_string),
        "size" => e.size.map(pair),
        "move" => e.position.map(pair),
        "opacity" => e.opacity.map(|o| lua_string(&format!("{:.2}", float2(o)))),
        "rounding" => e.rounding.map(|v| v.to_string()),
        "border_size" => e.border_size.map(|v| v.to_string()),
        "idle_inhibit" => e.idle_inhibit.as_deref().map(lua_string),
        "tag" => e
            .tag
            .as_deref()
            .map(|t| lua_string(&format!("{TAG_SET_PREFIX}{t}"))),
        _ => None,
    }
}

fn layer_value(e: &LayerEffects, uq: &str) -> Option<String> {
    match uq {
        "blur" => e.blur.map(lb),
        "no_anim" => e.no_anim.map(lb),
        "ignore_alpha" => e.ignore_alpha.map(|v| format!("{:?}", float2(v))),
        "animation" => e.animation.as_deref().map(lua_string),
        _ => None,
    }
}

fn head(id: &str, enabled: bool) -> String {
    format!(
        "name = {}, enabled = {}",
        lua_string(&format!("{RULE_NAME_PREFIX}{id}")),
        lb(enabled)
    )
}

/// `hl.window_rule({ name = "uq:<id>", enabled = <bool>, match = { … }, <effect> = <v>, … })`.
pub fn emit_window_rule(r: &WindowRule) -> String {
    let matches: Vec<String> = MATCH_KEYS
        .iter()
        .filter_map(|k| match_value(&r.matches, k.uq).map(|v| format!("{} = {v}", k.lua)))
        .collect();
    let mut out = format!(
        "hl.window_rule({{ {}, match = {{ {} }}",
        head(&r.id, r.enabled),
        matches.join(", ")
    );
    for e in WINDOW_EFFECTS.iter() {
        if let Some(v) = effect_value(&r.effects, e.field.uq) {
            let _ = write!(out, ", {} = {v}", e.field.lua);
        }
    }
    out.push_str(" })");
    out
}

/// `hl.layer_rule({ name = "uq:<id>", enabled = <bool>, match = { namespace = … }, … })`.
pub fn emit_layer_rule(r: &LayerRule) -> String {
    let mut out = format!(
        "hl.layer_rule({{ {}, match = {{ {} = {} }}",
        head(&r.id, r.enabled),
        hypr::LAYER_MATCH_KEYS[0].lua,
        lua_string(&r.matches.namespace)
    );
    for f in LAYER_EFFECTS.iter() {
        if let Some(v) = layer_value(&r.effects, f.uq) {
            let _ = write!(out, ", {} = {v}", f.lua);
        }
    }
    out.push_str(" })");
    out
}

fn workspace_value(r: &WorkspaceRule, uq: &str) -> Option<String> {
    match uq {
        "monitor" => r.monitor.as_deref().map(lua_string),
        "default" => r.default.map(lb),
        "persistent" => r.persistent.map(lb),
        "gaps_in" => r.gaps_in.map(|v| v.to_string()),
        "gaps_out" => r.gaps_out.map(|v| v.to_string()),
        "border_size" => r.border_size.map(|v| v.to_string()),
        _ => None,
    }
}

/// One `hl.workspace_rule` call: the selector, its layout (if any) and the rule's other fields.
/// Hyprland merges calls for the same workspace, but one call per selector keeps the file plain.
pub(super) fn workspace_call(
    ws: &str,
    layout: Option<Layout>,
    rule: Option<&WorkspaceRule>,
) -> String {
    let mut out = format!("hl.workspace_rule({{ workspace = {}", lua_string(ws));
    if let Some(l) = layout {
        let _ = write!(out, ", layout = {}", lua_string(l.as_str()));
    }
    if let Some(r) = rule {
        for f in WORKSPACE_FIELDS.iter() {
            if let Some(v) = workspace_value(r, f.uq) {
                let _ = write!(out, ", {} = {v}", f.lua);
            }
        }
    }
    out.push_str(" })");
    out
}

/// The `-- Workspace rules` block: selectors in `rules` list order (merged with their layout),
/// then layout-only selectors in sorted order. Empty when there is nothing.
pub fn emit_workspace_rules(layouts: &BTreeMap<String, Layout>, rules: &[WorkspaceRule]) -> String {
    let mut lines = Vec::new();
    let mut done = BTreeSet::new();
    for r in rules {
        let sel = workspace_selector(&r.workspace);
        if !done.insert(sel.clone()) {
            continue;
        }
        lines.push(workspace_call(&sel, layouts.get(&sel).copied(), Some(r)));
    }
    for (ws, layout) in layouts {
        if !done.contains(&workspace_selector(ws)) {
            lines.push(workspace_call(ws, Some(*layout), None));
        }
    }
    block("Workspace rules", &lines)
}

fn block(title: &str, lines: &[String]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    format!("\n-- {title}\n{}\n", lines.join("\n"))
}

pub(super) fn emit_window_block(rules: &[WindowRule]) -> String {
    let lines: Vec<String> = rules.iter().map(emit_window_rule).collect();
    block("Window rules", &lines)
}

/// The `-- Rules from apps` block: packaged rules the person turned on, already turned into
/// window rules (`integration::rules::AppRule::window_rules`). Same calls as the person's own.
pub(super) fn emit_app_block(rules: &[WindowRule]) -> String {
    let lines: Vec<String> = rules.iter().map(emit_window_rule).collect();
    block("Rules from apps", &lines)
}

pub(super) fn emit_layer_block(rules: &[LayerRule]) -> String {
    let lines: Vec<String> = rules.iter().map(emit_layer_rule).collect();
    block("Panel rules", &lines)
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
