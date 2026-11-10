//! Which open windows a rule matches, which window is under the cursor, and plain-words text
//! for rules. Pure: the daemon hands in `hyprctl -j` text.

use super::{WindowMatch, WindowRule, check_pattern};
use crate::settings::{SettingError, clean_text_spaced};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_WINDOWS: usize = 500;
const MAX_TEXT: usize = 512;

/// The fields of one open window that rules can match. Also the pick reply's and
/// `ListWindows`' row. No address, no pid.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct WindowInfo {
    pub class: String,
    pub title: String,
    pub initial_class: String,
    pub initial_title: String,
    pub xwayland: bool,
    pub floating: bool,
    pub fullscreen: bool,
    pub pinned: bool,
    pub workspace_id: i64,
    pub workspace_name: String,
    pub tags: Vec<String>,
}

fn text(v: &Value, key: &str) -> String {
    clean_text_spaced(v.get(key).and_then(Value::as_str).unwrap_or(""), MAX_TEXT)
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn info(c: &Value) -> WindowInfo {
    let ws = c.get("workspace");
    WindowInfo {
        class: text(c, "class"),
        title: text(c, "title"),
        initial_class: text(c, "initialClass"),
        initial_title: text(c, "initialTitle"),
        xwayland: flag(c, "xwayland"),
        floating: flag(c, "floating"),
        fullscreen: fullscreen_mode(c) > 0,
        pinned: flag(c, "pinned"),
        workspace_id: ws
            .and_then(|w| w.get("id"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        workspace_name: ws.map(|w| text(w, "name")).unwrap_or_default(),
        tags: c
            .get("tags")
            .and_then(Value::as_array)
            .map(|t| {
                t.iter()
                    .filter_map(Value::as_str)
                    .take(64)
                    .map(|s| clean_text_spaced(s, MAX_TEXT))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn fullscreen_mode(c: &Value) -> i64 {
    match c.get("fullscreen") {
        Some(Value::Bool(b)) => i64::from(*b),
        Some(v) => v.as_i64().unwrap_or(0),
        None => 0,
    }
}

fn array(json: &str) -> Vec<Value> {
    match serde_json::from_str(json) {
        Ok(Value::Array(a)) => a,
        _ => Vec::new(),
    }
}

/// The mapped windows of `hyprctl clients -j`, at most 500. Text is untrusted: control
/// characters become spaces and it is cut to 512 characters.
pub fn windows_from_clients(json: &str) -> Vec<WindowInfo> {
    array(json)
        .iter()
        .filter(|c| flag(c, "mapped"))
        .take(MAX_WINDOWS)
        .map(info)
        .collect()
}

/// Whether a window matches a rule. `IfDialog` is "yes, if it is a dialog": `clients -j` does
/// not say, so a `modal` match can't be decided. `Unknown` is for selectors that can't be
/// evaluated from here.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Yes,
    No,
    IfDialog,
    Unknown,
}

enum Ws {
    Id(i64),
    Name(String),
    /// A bare `special`: the default special workspace.
    DefaultSpecial,
    Unknown,
}

/// A window rule's match, compiled once.
pub struct Compiled {
    class: Option<Regex>,
    title: Option<Regex>,
    initial_class: Option<Regex>,
    initial_title: Option<Regex>,
    tag: Option<String>,
    xwayland: Option<bool>,
    floating: Option<bool>,
    fullscreen: Option<bool>,
    pinned: Option<bool>,
    modal: bool,
    workspace: Option<Ws>,
}

fn compile(key: &str, p: &Option<String>) -> Result<Option<Regex>, SettingError> {
    let Some(p) = p else { return Ok(None) };
    let field = format!("rules.window.match.{key}");
    check_pattern(&field, p)?;
    RegexBuilder::new(&format!("^(?:{p})$"))
        .size_limit(1 << 18)
        .build()
        .map(Some)
        .map_err(|_| {
            crate::settings::invalid(
                &field,
                "this pattern can't be used: this pattern isn't valid",
            )
        })
}

fn workspace(sel: &str) -> Ws {
    if sel.contains('[') {
        Ws::Unknown
    } else if let Ok(id) = sel.parse::<i64>() {
        Ws::Id(id)
    } else if let Some(n) = sel.strip_prefix("name:") {
        Ws::Name(n.to_string())
    } else if sel == "special" {
        Ws::DefaultSpecial
    } else if sel.starts_with("special:") {
        Ws::Name(sel.to_string())
    } else {
        Ws::Unknown
    }
}

fn re(r: &Option<Regex>, s: &str) -> bool {
    r.as_ref().is_none_or(|r| r.is_match(s))
}

fn eq(want: Option<bool>, got: bool) -> bool {
    want.is_none_or(|w| w == got)
}

impl Compiled {
    pub fn new(m: &WindowMatch) -> Result<Compiled, SettingError> {
        Ok(Compiled {
            class: compile("class", &m.class)?,
            title: compile("title", &m.title)?,
            initial_class: compile("initial_class", &m.initial_class)?,
            initial_title: compile("initial_title", &m.initial_title)?,
            tag: m.tag.clone(),
            xwayland: m.xwayland,
            floating: m.floating,
            fullscreen: m.fullscreen,
            pinned: m.pinned,
            modal: m.modal == Some(true),
            workspace: m.workspace.as_deref().map(workspace),
        })
    }

    pub fn test(&self, w: &WindowInfo) -> Verdict {
        let certain = re(&self.class, &w.class)
            && re(&self.title, &w.title)
            && re(&self.initial_class, &w.initial_class)
            && re(&self.initial_title, &w.initial_title)
            && self.tag.as_ref().is_none_or(|t| {
                w.tags
                    .iter()
                    .any(|g| g == t || g.strip_suffix('*') == Some(t))
            })
            && eq(self.xwayland, w.xwayland)
            && eq(self.floating, w.floating)
            && eq(self.fullscreen, w.fullscreen)
            && eq(self.pinned, w.pinned);
        if !certain {
            return Verdict::No;
        }
        let mut verdict = if self.modal {
            Verdict::IfDialog
        } else {
            Verdict::Yes
        };
        match &self.workspace {
            None => {}
            Some(Ws::Unknown) => verdict = Verdict::Unknown,
            Some(Ws::Id(id)) if *id != w.workspace_id => return Verdict::No,
            Some(Ws::Name(n)) if *n != w.workspace_name => return Verdict::No,
            Some(Ws::DefaultSpecial)
                if !matches!(w.workspace_name.as_str(), "special" | "special:special") =>
            {
                return Verdict::No;
            }
            Some(_) => {}
        }
        verdict
    }
}

/// The rules (by index, in list order) that match or might match `w`; disabled ones are
/// included, the caller greys them. A rule whose pattern doesn't compile is left out.
pub fn matching(rules: &[WindowRule], w: &WindowInfo) -> Vec<(usize, Verdict)> {
    rules
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            let v = Compiled::new(&r.matches).ok()?.test(w);
            (v != Verdict::No).then_some((i, v))
        })
        .collect()
}

fn num(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn pair(v: &Value, key: &str) -> Option<(f64, f64)> {
    let a = v.get(key)?.as_array()?;
    Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?))
}

fn ws_id(v: &Value, key: &str) -> i64 {
    v.get(key)
        .and_then(|w| w.get("id"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
}

/// The window under the cursor, from `hyprctl clients -j`, `monitors all -j` and `cursorpos -j`.
/// Only windows that are showing count: on the active (or open special) workspace of a monitor
/// that is on, or pinned. Special-workspace windows come first, then fullscreen, floating, and
/// tiled, each by most recently focused.
pub fn window_at(clients_json: &str, monitors_json: &str, cursor_json: &str) -> Option<WindowInfo> {
    let cursor: Value = serde_json::from_str(cursor_json).ok()?;
    let (cx, cy) = (num(&cursor, "x")?, num(&cursor, "y")?);
    let mut visible = Vec::new();
    let mut special = Vec::new();
    for m in array(monitors_json) {
        if flag(&m, "disabled") {
            continue;
        }
        visible.push(ws_id(&m, "activeWorkspace"));
        let s = ws_id(&m, "specialWorkspace");
        if s != 0 {
            visible.push(s);
            special.push(s);
        }
    }
    array(clients_json)
        .iter()
        .filter(|c| flag(c, "mapped") && !flag(c, "hidden"))
        .filter(|c| flag(c, "pinned") || visible.contains(&ws_id(c, "workspace")))
        .filter(|c| match (pair(c, "at"), pair(c, "size")) {
            (Some((x, y)), Some((w, h))) => cx >= x && cx < x + w && cy >= y && cy < y + h,
            _ => false,
        })
        .min_by_key(|c| {
            let tier = if special.contains(&ws_id(c, "workspace")) {
                0
            } else if fullscreen_mode(c) > 0 {
                1
            } else if flag(c, "floating") {
                2
            } else {
                3
            };
            let focus = c
                .get("focusHistoryID")
                .and_then(Value::as_i64)
                .unwrap_or(i64::MAX);
            (tier, focus)
        })
        .map(info)
}

#[cfg(test)]
mod tests;
