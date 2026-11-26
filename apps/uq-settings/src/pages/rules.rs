//! The Window rules page's logic: [`RulesState`] talks to uqd (`org.uq.Desktop1`) and is plain
//! Rust, tested against the fake uqd. `crate::rules_model` is the thin QML object around it.
//!
//! The page edits the `rules` section as a whole (read it, change one rule, write it back under
//! the write lock), shows which open windows a rule matches, and lets the person click a window
//! to fill a rule or ask "why did this window get these rules".

mod apps;
mod fields;
mod rows;
mod save;

use std::sync::Mutex as StdMutex;

use serde_json::{Map, Value, json};
use tokio::sync::Mutex;
use uq_core::client::{self, DesktopProxy};
use uq_core::rules::{
    Compiled, MAX_LAYER_RULES, MAX_NAME_CHARS, MAX_PATTERN_CHARS, MAX_WINDOW_RULES,
    MAX_WORKSPACE_RULES, Rules, TEMPLATES, Verdict, WindowInfo, WindowRule, build_template,
    effects_text, exact_pattern, has_static, matching,
};
use uq_core::settings::SettingError;

use crate::pages::runner::{PageLogic, parse_json, patch_section};

pub use fields::fields;
use rows::source_of;
mod edit;

/// How many window names a reopen note lists.
const REOPEN_NAMES: usize = 3;

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    RefreshWindows,
    /// `kind` is `"window" | "layer" | "workspace"`. An add when `replaces` is `None` (a
    /// workspace that already has a rule is refused), else a replace of the rule with that id
    /// (or workspace).
    Save {
        kind: String,
        rule: Value,
        replaces: Option<String>,
    },
    /// `key` is a rule id, or a workspace selector.
    Delete {
        kind: String,
        key: String,
    },
    Move {
        kind: String,
        key: String,
        to: usize,
    },
    SetEnabled {
        kind: String,
        key: String,
        on: bool,
    },
    FromTemplate {
        template: String,
        app: Option<String>,
        workspace: Option<String>,
        silent: bool,
    },
    /// Let the person click a window: `"match"` (fill a rule) or `"why"`.
    Pick {
        purpose: String,
    },
    /// Turn a rule from an app (`<integration>.<rule>`) on or off.
    SetAppRule {
        id: String,
        on: bool,
    },
}

#[derive(Debug, Default)]
struct Pick {
    seq: u64,
    purpose: String,
    status: String,
    window: Option<WindowInfo>,
}

#[derive(Debug, Default)]
struct Why {
    seq: u64,
    window: Option<WindowInfo>,
}

#[derive(Debug, Default)]
struct Reopen {
    seq: u64,
    names: Vec<String>,
    more: usize,
}

#[derive(Debug, Default)]
struct Shown {
    pick: Pick,
    why: Why,
    reopen: Reopen,
    notice_seq: u64,
    notice: String,
}

pub struct RulesState {
    conn: zbus::Connection,
    /// Edits read-modify-write the section, so one at a time.
    write: Mutex<()>,
    shown: StdMutex<Shown>,
    /// The last window list, for `preview` and `matches_now`.
    windows: StdMutex<Vec<WindowInfo>>,
    /// The last `rules` section read from uqd, for `check`.
    last: StdMutex<Value>,
}

fn failure(text: impl Into<String>) -> zbus::Error {
    zbus::Error::Failure(text.into())
}

/// The plain reason of a refusal, without the key.
fn reason(e: &SettingError) -> String {
    match e {
        SettingError::Invalid { reason, .. } => reason.clone(),
        other => other.to_string(),
    }
}

/// A pattern that matches exactly `text` (an app ID picked from a window).
pub fn exact(text: &str) -> String {
    exact_pattern(text)
}

fn pick_json(p: &Pick) -> Value {
    json!({"seq": p.seq, "purpose": p.purpose, "status": p.status,
        "window": window_json(&p.window),
        "class_pattern": p.window.as_ref().map(|w| exact(&w.class)).unwrap_or_default()})
}

fn verdict_name(v: Verdict) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

fn window_json(w: &Option<WindowInfo>) -> Value {
    serde_json::to_value(w).unwrap_or(Value::Null)
}

/// The key of a rule in its list: the id, or a workspace rule's selector.
fn key_of<'a>(kind: &str, rule: &'a Value) -> &'a str {
    let field = if kind == "workspace" {
        "workspace"
    } else {
        "id"
    };
    rule.get(field).and_then(Value::as_str).unwrap_or("")
}

fn list_mut<'a>(
    section: &'a mut Map<String, Value>,
    kind: &str,
) -> zbus::Result<&'a mut Vec<Value>> {
    if !matches!(kind, "window" | "layer" | "workspace") {
        return Err(failure("that kind of rule doesn't exist"));
    }
    section
        .entry(kind)
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| failure(format!("the {kind} rules aren't a list")))
}

fn position(list: &[Value], kind: &str, key: &str) -> zbus::Result<usize> {
    list.iter()
        .position(|r| key_of(kind, r) == key)
        .ok_or_else(|| failure("that rule isn't there any more"))
}

/// The state is the snapshot described in the plan's Task 9: `{window, layer, workspace,
/// section, windows, why, pick, reopen, templates, fields, limits, notice}`, plus `apps`, the
/// rules from apps (`{rules, problems, missing}`, see `apps.rs`).
impl PageLogic for RulesState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh | Op::RefreshWindows)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        match op {
            Op::Refresh | Op::RefreshWindows => self.refresh_windows(&proxy).await,
            Op::Save {
                kind,
                rule,
                replaces,
            } => self.save(&proxy, &kind, rule, replaces.as_deref()).await,
            Op::Delete { kind, key } => {
                patch_section(&proxy, "rules", |s| {
                    let list = list_mut(s, &kind)?;
                    let i = position(list, &kind, &key)?;
                    list.remove(i);
                    Ok(())
                })
                .await?;
                self.refresh_windows(&proxy).await.ok();
                Ok(())
            }
            Op::Move { kind, key, to } => {
                patch_section(&proxy, "rules", |s| {
                    let list = list_mut(s, &kind)?;
                    let i = position(list, &kind, &key)?;
                    let rule = list.remove(i);
                    list.insert(to.min(list.len()), rule);
                    Ok(())
                })
                .await?;
                self.refresh_windows(&proxy).await.ok();
                Ok(())
            }
            Op::SetEnabled { kind, key, on } => {
                if kind == "workspace" {
                    return Err(failure("a workspace rule can't be switched off"));
                }
                patch_section(&proxy, "rules", |s| {
                    let list = list_mut(s, &kind)?;
                    let i = position(list, &kind, &key)?;
                    list[i]["enabled"] = json!(on);
                    Ok(())
                })
                .await?;
                self.refresh_windows(&proxy).await.ok();
                Ok(())
            }
            Op::FromTemplate {
                template,
                app,
                workspace,
                silent,
            } => {
                self.add_template(
                    &proxy,
                    &template,
                    app.as_deref(),
                    workspace.as_deref(),
                    silent,
                )
                .await
            }
            Op::Pick { purpose } => self.pick(&proxy, &purpose).await,
            Op::SetAppRule { id, on } => {
                patch_section(&proxy, "integration_rules", |s| apps::set(s, &id, on)).await
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        match self.read_all().await {
            Err(e) if client::no_owner(&e) => Ok(self.down()),
            other => other,
        }
    }
}

impl RulesState {
    /// uqd isn't running: no rules, but the pick status (a pick that found no uqd says so) and
    /// a plain error, so the page never fails silently.
    fn down(&self) -> Value {
        let shown = self.shown.lock().unwrap();
        json!({"error": "uQ isn't running", "pick": pick_json(&shown.pick),
            "notice": {"seq": shown.notice_seq, "text": shown.notice}})
    }

    async fn read_all(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let section = parse_json(&proxy.get_section("rules").await?)?;
        let rules: Rules = serde_json::from_value(section.clone())
            .map_err(|_| failure("uQ's settings service sent unreadable rules"))?;
        *self.last.lock().unwrap() = section;
        let layouts = parse_json(&proxy.get_section("layouts").await?)?;
        // A failed `Explain` leaves the rows without a source; uqd being gone is the caller's.
        let explained = match proxy.explain("rules").await.and_then(|t| parse_json(&t)) {
            Ok(v) => v,
            Err(e) if client::no_owner(&e) => return Err(e),
            Err(_) => json!({}),
        };
        let apps = apps::read(&proxy).await?;
        let windows = self.windows.lock().unwrap().clone();
        let mut out = Self::rows(&rules, &windows, &explained, &layouts);
        let rejected = &explained["rules"];
        out["section"] = if rejected["source"] == "error" {
            json!({"source": "error", "message": rejected["detail"]["message"]})
        } else {
            Value::Null
        };
        let shown = self.shown.lock().unwrap();
        let why_rules: Vec<Value> = match &shown.why.window {
            Some(w) => matching(&rules.window, w)
                .into_iter()
                .map(|(i, v)| {
                    let r = &rules.window[i];
                    let (source, line, path) = source_of(&explained, &format!("window.{}", r.id));
                    json!({"id": r.id, "name": r.name, "enabled": r.enabled,
                        "verdict": verdict_name(v),
                        "effects_text": effects_text(&r.effects),
                        "source": source, "line": line, "path": path})
                })
                .collect(),
            None => Vec::new(),
        };
        let templates: Vec<Value> = TEMPLATES
            .iter()
            .map(|t| json!({"id": t.id, "name": t.name, "needs": t.needs}))
            .collect();
        out["windows"] = json!(windows);
        out["why"] = json!({"seq": shown.why.seq, "window": window_json(&shown.why.window),
            "rules": why_rules});
        out["pick"] = pick_json(&shown.pick);
        out["reopen"] = json!({"seq": shown.reopen.seq, "names": shown.reopen.names,
            "more": shown.reopen.more});
        out["templates"] = json!(templates);
        out["fields"] = fields();
        out["limits"] = json!({"window": MAX_WINDOW_RULES, "layer": MAX_LAYER_RULES,
            "workspace": MAX_WORKSPACE_RULES, "name": MAX_NAME_CHARS,
            "pattern": MAX_PATTERN_CHARS});
        out["notice"] = json!({"seq": shown.notice_seq, "text": shown.notice});
        out["apps"] = apps;
        Ok(out)
    }
}

#[cfg(test)]
mod tests;
