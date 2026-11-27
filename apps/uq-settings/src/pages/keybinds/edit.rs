//! Editing shortcuts: rebinding, resetting and custom shortcuts, through uqd.

use super::*;

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

    pub(crate) fn notice(&self, text: String) {
        let mut shown = self.shown.lock().unwrap();
        shown.notice_seq += 1;
        shown.notice = text;
    }

    pub(crate) fn set_capture(&self, state: &'static str, token: String) {
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
    pub(crate) fn candidate(
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
                bind.insert("name".into(), json!("This shortcut"));
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

    pub(crate) async fn apply_op(&self, proxy: &DesktopProxy<'_>, op: Op) -> zbus::Result<()> {
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
    pub(crate) async fn pause(&self, proxy: &DesktopProxy<'_>) {
        match proxy.pause_shortcuts().await {
            Ok(token) => self.set_capture("paused", token),
            Err(_) => self.set_capture("unavailable", String::new()),
        }
    }

    /// Resume the pause this page started, if one is in force. A token uqd already dropped (the
    /// 15 s passed) is fine: shortcuts are back either way.
    pub(crate) async fn resume(&self, proxy: &DesktopProxy<'_>) {
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
