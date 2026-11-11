//! The Keyboard shortcuts page's logic: [`KeybindsState`] talks to uqd (`org.uq.Desktop1`) and is
//! plain Rust, tested against the fake uqd. `crate::keybinds_model` is the thin QML object
//! around it.
//!
//! The page edits the `keybinds` section as a whole (read it, change one entry, write it back
//! under the write lock, origin `settings`). uqd checks the keys and the conflicts again on
//! every write; [`KeybindsState::check`] runs the same check on a draft as you type, with no
//! D-Bus call, so the editor can warn before Save.

pub mod capture;

use std::path::PathBuf;
use std::sync::Mutex as StdMutex;

use serde_json::{Map, Value, json};
use tokio::sync::Mutex;
use uq_core::client::{self, DesktopProxy};
use uq_core::keybinds::actions::catalogue;
use uq_core::keybinds::{Keybinds, combo, defaults};
use uq_core::settings::SettingError;

use crate::desktop_entries;
use crate::pages::runner::{PageLogic, parse_json, patch_section};

pub use capture::combo_from_qt;

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    /// New keys for one of uQ's shortcuts or one of your own (canonical or aliased text).
    SetCombo {
        id: String,
        combo: String,
    },
    SetEnabled {
        id: String,
        on: bool,
    },
    /// Put one of uQ's shortcuts back to how it came.
    Reset {
        id: String,
    },
    /// Put every one of uQ's shortcuts back; your own stay.
    ResetAll,
    /// An add when `bind` has no id, else a replace of that shortcut of your own.
    SaveCustom {
        bind: Value,
    },
    DeleteCustom {
        id: String,
    },
    /// Pause every shortcut so the next combo reaches the editor (R11).
    PauseCapture,
    ResumeCapture,
}

#[derive(Debug, Default)]
struct Capture {
    seq: u64,
    /// `idle`, `paused` or `unavailable` (Hyprland isn't answering: type the keys instead).
    state: &'static str,
    token: String,
}

#[derive(Debug, Default)]
struct Shown {
    capture: Capture,
    notice_seq: u64,
    notice: String,
}

pub struct KeybindsState {
    conn: zbus::Connection,
    /// Edits read-modify-write the section, so one at a time.
    write: Mutex<()>,
    shown: StdMutex<Shown>,
    /// Where the installed apps are read from, for "Start an app".
    app_dirs: Vec<PathBuf>,
    /// The last `keybinds` section and `ListBinds` read from uqd, for `check`.
    last: StdMutex<(Value, Value)>,
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

fn list_mut<'a>(
    section: &'a mut Map<String, Value>,
    name: &str,
) -> zbus::Result<&'a mut Vec<Value>> {
    section
        .entry(name)
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| failure(format!("the {name} aren't a list")))
}

fn id_of(v: &Value) -> &str {
    v.get("id").and_then(Value::as_str).unwrap_or("")
}

fn gone() -> zbus::Error {
    failure("that shortcut isn't there any more")
}

/// Change the shortcut `id` through `edit`: an override entry for one of uQ's, else the entry of
/// your own. An override left with nothing in it is dropped.
fn edit_bind(
    section: &mut Map<String, Value>,
    id: &str,
    edit: impl FnOnce(&mut Map<String, Value>),
) -> zbus::Result<()> {
    if defaults::default_of(id).is_some() {
        let list = list_mut(section, "overrides")?;
        let at = list.iter().position(|o| id_of(o) == id).unwrap_or_else(|| {
            list.push(json!({ "id": id }));
            list.len() - 1
        });
        let entry = list[at].as_object_mut().ok_or_else(gone)?;
        edit(entry);
        if entry.len() == 1 {
            list.remove(at);
        }
        return Ok(());
    }
    let list = list_mut(section, "custom")?;
    let at = list.iter().position(|b| id_of(b) == id).ok_or_else(gone)?;
    edit(list[at].as_object_mut().ok_or_else(gone)?);
    Ok(())
}

fn text_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

impl KeybindsState {
    pub fn new(conn: zbus::Connection) -> Self {
        Self::with_dirs(conn, desktop_entries::default_dirs())
    }

    pub fn with_dirs(conn: zbus::Connection, app_dirs: Vec<PathBuf>) -> Self {
        Self {
            conn,
            write: Mutex::new(()),
            shown: StdMutex::new(Shown::default()),
            app_dirs,
            last: StdMutex::new((Value::Null, Value::Null)),
        }
    }

    fn notice(&self, text: String) {
        let mut shown = self.shown.lock().unwrap();
        shown.notice_seq += 1;
        shown.notice = text;
    }

    fn set_capture(&self, state: &'static str, token: String) {
        let mut shown = self.shown.lock().unwrap();
        shown.capture = Capture {
            seq: shown.capture.seq + 1,
            state,
            token,
        };
    }

    /// Whether `draft` (`{id?, combo, name?, action?, enabled?, locked?, repeating?}`) could be
    /// saved against the shortcuts read last, and who already uses its keys:
    /// `{valid, error, conflicts: [{id, label}], user_lua: [{line, text}], combo_text}`.
    /// Synchronous and pure: no D-Bus. The same checks uqd makes on a write ("the editor warns as
    /// you type"). A draft with an id changes that shortcut; without one it is a new shortcut of
    /// your own, and a missing name or action is filled in so only the keys are judged.
    pub fn check(&self, draft: &Value) -> Value {
        let (section, list) = self.last.lock().unwrap().clone();
        let text = text_of(draft, "combo").trim().to_string();
        let parsed = combo::parse(&text).ok();
        let combo_text = parsed.map_or(String::new(), combo::text);
        let answer = |valid: bool, error: &str, conflicts: Vec<Value>, user_lua: Vec<Value>| {
            json!({"valid": valid, "error": error, "conflicts": conflicts,
                "user_lua": user_lua, "combo_text": combo_text})
        };
        if text.is_empty() {
            return answer(false, "Choose the keys for this shortcut.", vec![], vec![]);
        }
        let Some(section) = section.as_object().cloned() else {
            return answer(false, "uQ isn't running", vec![], vec![]);
        };
        let current = Keybinds::from_section(&Value::Object(section.clone()), &Keybinds::default())
            .unwrap_or_default();
        let id = id_of(draft).to_string();
        let candidate = match Self::candidate(section, draft, &id) {
            Ok(c) => c,
            Err(e) => return answer(false, &e.to_string(), vec![], vec![]),
        };
        let verdict = Keybinds::from_section(&candidate, &current);
        // Who has the keys now: the others that are on and share them. A draft that is off
        // takes no keys.
        let draft_on = draft.get("enabled").and_then(Value::as_bool) != Some(false);
        let conflicts: Vec<Value> = match parsed {
            Some(c) if draft_on => current
                .effective()
                .into_iter()
                .filter(|e| e.enabled && e.combo == Some(c) && e.id != id)
                .map(|e| json!({"id": e.id, "label": e.label}))
                .collect(),
            _ => Vec::new(),
        };
        let canonical = parsed.map(combo::canonical).unwrap_or_default();
        let user_lua: Vec<Value> = list["user_lua"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| !canonical.is_empty() && text_of(m, "combo") == canonical)
            .map(|m| json!({"line": m["line"], "text": m["text"]}))
            .collect();
        match verdict {
            Ok(_) => answer(true, "", conflicts, user_lua),
            Err(e) => answer(false, &reason(&e), conflicts, user_lua),
        }
    }

    /// The section with `draft` applied, as a write would send it.
    fn candidate(
        mut section: Map<String, Value>,
        draft: &Value,
        id: &str,
    ) -> Result<Value, SettingError> {
        let nope = || SettingError::Invalid {
            key: "keybinds".into(),
            reason: "that shortcut isn't there any more".into(),
        };
        let combo = text_of(draft, "combo").to_string();
        if id.is_empty() {
            let mut bind = draft.as_object().cloned().unwrap_or_default();
            bind.remove("id");
            if text_of(draft, "name").trim().is_empty() {
                bind.insert("name".into(), json!("Preview"));
            }
            if !bind.contains_key("action") {
                let first = catalogue().into_iter().next().map(|(_, a)| a);
                bind.insert(
                    "action".into(),
                    serde_json::to_value(first).unwrap_or(Value::Null),
                );
            }
            list_mut(&mut section, "custom")
                .map_err(|_| nope())?
                .push(Value::Object(bind));
        } else if defaults::default_of(id).is_some() {
            edit_bind(&mut section, id, |o| {
                o.insert("combo".into(), json!(combo));
            })
            .map_err(|_| nope())?;
        } else {
            let list = list_mut(&mut section, "custom").map_err(|_| nope())?;
            let at = list.iter().position(|b| id_of(b) == id).ok_or_else(nope)?;
            if let (Some(old), Some(new)) = (list[at].as_object_mut(), draft.as_object()) {
                for (k, v) in new {
                    old.insert(k.clone(), v.clone());
                }
            }
        }
        Ok(Value::Object(section))
    }

    async fn apply_op(&self, proxy: &DesktopProxy<'_>, op: Op) -> zbus::Result<()> {
        match op {
            Op::Refresh | Op::PauseCapture | Op::ResumeCapture => Ok(()),
            Op::SetCombo { id, combo } => {
                patch_section(proxy, "keybinds", |s| {
                    edit_bind(s, &id, |o| {
                        o.insert("combo".into(), json!(combo));
                    })
                })
                .await
            }
            Op::SetEnabled { id, on } => {
                patch_section(proxy, "keybinds", |s| {
                    edit_bind(s, &id, |o| {
                        if on && defaults::default_of(&id).is_some() {
                            o.remove("enabled");
                        } else {
                            o.insert("enabled".into(), json!(on));
                        }
                    })
                })
                .await
            }
            Op::Reset { id } => {
                if defaults::default_of(&id).is_none() {
                    return Err(failure(
                        "your own shortcuts are changed or deleted, not reset",
                    ));
                }
                patch_section(proxy, "keybinds", |s| {
                    list_mut(s, "overrides")?.retain(|o| id_of(o) != id);
                    Ok(())
                })
                .await
            }
            Op::ResetAll => {
                patch_section(proxy, "keybinds", |s| {
                    s.insert("overrides".into(), json!([]));
                    Ok(())
                })
                .await?;
                self.notice("Put all of uQ's shortcuts back to how they came".into());
                Ok(())
            }
            Op::SaveCustom { bind } => {
                let name = text_of(&bind, "name").trim().to_string();
                let id = id_of(&bind).to_string();
                patch_section(proxy, "keybinds", |s| {
                    let list = list_mut(s, "custom")?;
                    if id.is_empty() {
                        list.push(bind);
                    } else {
                        let at = list.iter().position(|b| id_of(b) == id).ok_or_else(gone)?;
                        list[at] = bind;
                    }
                    Ok(())
                })
                .await?;
                if id.is_empty() {
                    self.notice(format!("Added the shortcut “{name}”"));
                }
                Ok(())
            }
            Op::DeleteCustom { id } => {
                patch_section(proxy, "keybinds", |s| {
                    let list = list_mut(s, "custom")?;
                    let at = list.iter().position(|b| id_of(b) == id).ok_or_else(gone)?;
                    list.remove(at);
                    Ok(())
                })
                .await
            }
        }
    }

    /// Pause, and record what happened. Hyprland not answering is not a refusal: the editor
    /// falls back to typed keys, so the state is `unavailable` and the call is `Ok`.
    async fn pause(&self, proxy: &DesktopProxy<'_>) {
        match proxy.pause_shortcuts().await {
            Ok(token) => self.set_capture("paused", token),
            Err(_) => self.set_capture("unavailable", String::new()),
        }
    }

    /// Resume the pause this page started, if one is in force. A token uqd already dropped (the
    /// 15 s passed) is fine: shortcuts are back either way.
    async fn resume(&self, proxy: &DesktopProxy<'_>) {
        let token = {
            let shown = self.shown.lock().unwrap();
            (shown.capture.state == "paused").then(|| shown.capture.token.clone())
        };
        if let Some(token) = token {
            let _ = proxy.resume_shortcuts(&token).await;
        }
        self.set_capture("idle", String::new());
    }
}

/// The state is the snapshot: `{groups, custom, user_lua, section, capture, catalogue, apps,
/// limits, notice}`.
impl PageLogic for KeybindsState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        match op {
            Op::PauseCapture => {
                self.pause(&proxy).await;
                Ok(())
            }
            Op::ResumeCapture => {
                self.resume(&proxy).await;
                Ok(())
            }
            other => self.apply_op(&proxy, other).await,
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let section = parse_json(&proxy.get_section("keybinds").await?)?;
        let list = parse_json(&proxy.list_binds().await?)?;
        // A failed `Explain` leaves the rows without a source; uqd being gone is the caller's.
        let explained = match proxy.explain("keybinds").await.and_then(|t| parse_json(&t)) {
            Ok(v) => v,
            Err(e) if client::no_owner(&e) => return Err(e),
            Err(_) => json!({}),
        };
        *self.last.lock().unwrap() = (section, list.clone());
        let dirs = self.app_dirs.clone();
        let apps = tokio::task::spawn_blocking(move || desktop_entries::scan(&dirs))
            .await
            .map_err(|e| failure(format!("reading the installed apps failed: {e}")))?;
        let mut view = self.view(&list, &explained);
        // Your own shortcuts carry what the editor needs to change them: the stored entry.
        let stored = self.last.lock().unwrap().0.clone();
        for row in view["custom"].as_array_mut().into_iter().flatten() {
            let id = row["id"].clone();
            row["raw"] = stored["custom"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|b| b["id"] == id)
                .cloned()
                .unwrap_or(Value::Null);
        }
        view["apps"] = apps
            .into_iter()
            .map(|a| json!({"id": a.id, "name": a.name, "icon": a.icon}))
            .collect();
        Ok(view)
    }
}

impl KeybindsState {
    fn row(bind: &Value, explained: &Value) -> Value {
        let id = text_of(bind, "id");
        let e = &explained[format!("binds.{id}")];
        let changed = defaults::default_of(id).is_some() && bind["is_default"] == false;
        let source = e["source"]
            .as_str()
            .unwrap_or(if bind["is_default"] == true {
                "default"
            } else {
                "uq"
            });
        let profile = match e["source"].as_str() {
            Some("profile") => e["detail"]["name"].as_str().unwrap_or(""),
            _ => "",
        };
        json!({"id": id, "label": bind["label"], "combo": bind["combo"],
            "combo_text": bind["combo_text"], "default_combo_text": bind["default_combo_text"],
            "enabled": bind["enabled"], "is_default": bind["is_default"], "changed": changed,
            "can_disable": bind["can_disable"], "action_text": bind["action_text"],
            "source": source, "line": e["detail"].get("line").cloned().unwrap_or(Value::Null),
            "path": e["detail"].get("path").cloned().unwrap_or(Value::Null),
            "profile": profile})
    }

    fn view(&self, list: &Value, explained: &Value) -> Value {
        let binds: Vec<&Value> = list["binds"].as_array().into_iter().flatten().collect();
        let groups: Vec<Value> = list["groups"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|g| g["id"] != "custom")
            .map(|g| {
                let rows: Vec<Value> = binds
                    .iter()
                    .filter(|b| b["group"] == g["id"])
                    .map(|b| Self::row(b, explained))
                    .collect();
                json!({"id": g["id"], "label": g["label"], "binds": rows})
            })
            .collect();
        let custom: Vec<Value> = binds
            .iter()
            .filter(|b| b["custom"] == true)
            .map(|b| {
                let mut row = Self::row(b, explained);
                row["name"] = b["name"].clone();
                row["locked"] = b["locked"].clone();
                row["repeating"] = b["repeating"].clone();
                row
            })
            .collect();
        let user_lua: Vec<Value> = list["user_lua"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|m| json!({"line": m["line"], "combo_text": m["combo_text"], "text": m["text"]}))
            .collect();
        let rejected = &explained["keybinds"];
        let section = if rejected["source"] == "error" {
            json!({"source": "error", "message": rejected["detail"]["message"]})
        } else {
            Value::Null
        };
        let shown = self.shown.lock().unwrap();
        let state = if shown.capture.state.is_empty() {
            "idle"
        } else {
            shown.capture.state
        };
        json!({"groups": groups, "custom": custom, "user_lua": user_lua, "section": section,
            "capture": {"seq": shown.capture.seq, "state": state, "token": shown.capture.token},
            "catalogue": {"keys": list["keys"], "actions": list["actions"]},
            "limits": list["limits"],
            "notice": {"seq": shown.notice_seq, "text": shown.notice}})
    }
}

#[cfg(test)]
mod tests;
