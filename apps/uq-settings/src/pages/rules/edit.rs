//! Editing rules: saving, moving, toggling and deleting, through uqd.

use super::*;

impl RulesState {
    pub fn new(conn: zbus::Connection) -> Self {
        Self {
            conn,
            write: Mutex::new(()),
            shown: StdMutex::new(Shown::default()),
            windows: StdMutex::new(Vec::new()),
            last: StdMutex::new(Value::Null),
        }
    }

    /// Whether `draft` (a window rule) can be saved, and which cached windows it matches.
    /// Synchronous and pure: no D-Bus. `{valid, error, matches: [{title, class, verdict}]}`.
    pub fn preview(&self, draft: &Value) -> Value {
        let mut draft = draft.clone();
        if let Some(o) = draft.as_object_mut()
            && o.get("name")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        {
            o.insert("name".into(), json!("Preview"));
        }
        let checked = Rules::from_section(&json!({ "window": [draft] }), &Rules::default());
        let rule = match checked {
            Ok(mut r) if r.window.len() == 1 => r.window.remove(0),
            Ok(_) => return Self::invalid("this isn't a window rule"),
            Err(e) => return Self::invalid(&reason(&e)),
        };
        let compiled = match Compiled::new(&rule.matches) {
            Ok(c) => c,
            Err(e) => return Self::invalid(&reason(&e)),
        };
        let matches: Vec<Value> = self
            .windows
            .lock()
            .unwrap()
            .iter()
            .filter_map(|w| match compiled.test(w) {
                Verdict::No => None,
                v => Some(json!({"title": w.title, "class": w.class, "verdict": verdict_name(v)})),
            })
            .collect();
        json!({"valid": true, "error": "", "matches": matches})
    }

    pub(crate) fn invalid(why: &str) -> Value {
        json!({"valid": false, "error": why, "matches": []})
    }

    pub(crate) async fn refresh_windows(&self, proxy: &DesktopProxy<'_>) -> zbus::Result<()> {
        let list: Vec<WindowInfo> =
            serde_json::from_value(parse_json(&proxy.list_windows().await?)?)
                .map_err(|_| failure("uQ's settings service sent an unreadable window list"))?;
        *self.windows.lock().unwrap() = list;
        Ok(())
    }

    pub(crate) fn notice(&self, text: String) {
        let mut shown = self.shown.lock().unwrap();
        shown.notice_seq += 1;
        shown.notice = text;
    }

    /// After a save: note the open windows a rule with once-only effects matches.
    pub(crate) fn reopen_for(&self, rule: &WindowRule) {
        if !rule.enabled || !has_static(&rule.effects) {
            return;
        }
        let Ok(compiled) = Compiled::new(&rule.matches) else {
            return;
        };
        let names: Vec<String> = self
            .windows
            .lock()
            .unwrap()
            .iter()
            .filter(|w| compiled.test(w) == Verdict::Yes)
            .map(|w| {
                if w.title.is_empty() {
                    w.class.clone()
                } else {
                    w.title.clone()
                }
            })
            .collect();
        if names.is_empty() {
            return;
        }
        let mut shown = self.shown.lock().unwrap();
        shown.reopen = Reopen {
            seq: shown.reopen.seq + 1,
            more: names.len().saturating_sub(REOPEN_NAMES),
            names: names.into_iter().take(REOPEN_NAMES).collect(),
        };
    }

    pub(crate) async fn save(
        &self,
        proxy: &DesktopProxy<'_>,
        kind: &str,
        rule: Value,
        replaces: Option<&str>,
    ) -> zbus::Result<()> {
        let saved = rule.clone();
        patch_section(proxy, "rules", |section| {
            save::put(section, kind, rule, replaces)
        })
        .await?;
        self.refresh_windows(proxy).await.ok();
        if kind == "window"
            && let Ok(r) = serde_json::from_value::<WindowRule>(saved)
        {
            self.reopen_for(&r);
        }
        Ok(())
    }

    pub(crate) async fn add_template(
        &self,
        proxy: &DesktopProxy<'_>,
        template: &str,
        app: Option<&str>,
        workspace: Option<&str>,
        silent: bool,
    ) -> zbus::Result<()> {
        let rule =
            build_template(template, app, workspace, silent).map_err(|e| failure(reason(&e)))?;
        let name = rule.name.clone();
        let value = serde_json::to_value(&rule).map_err(|e| failure(e.to_string()))?;
        patch_section(proxy, "rules", |section| {
            list_mut(section, "window")?.push(value);
            Ok(())
        })
        .await?;
        self.refresh_windows(proxy).await.ok();
        self.reopen_for(&rule);
        self.notice(format!("Added the rule “{name}”"));
        Ok(())
    }

    /// Ask uqd for a click on a window. A failed call still records a pick (F5), so the page
    /// has a status to show, then gives the error.
    pub(crate) async fn pick(&self, proxy: &DesktopProxy<'_>, purpose: &str) -> zbus::Result<()> {
        if !matches!(purpose, "match" | "why") {
            return Err(failure("that isn't something a window can be picked for"));
        }
        let reply = match proxy.pick_window().await {
            Ok(text) => parse_json(&text),
            Err(e) => Err(e),
        };
        let (status, window) = match &reply {
            Ok(v) => (
                v["status"].as_str().unwrap_or("error").to_string(),
                v.get("window")
                    .and_then(|w| serde_json::from_value::<WindowInfo>(w.clone()).ok()),
            ),
            Err(e) if client::no_owner(e) => ("unavailable".to_string(), None),
            Err(_) => ("error".to_string(), None),
        };
        {
            let mut shown = self.shown.lock().unwrap();
            let seq = shown.pick.seq + 1;
            if purpose == "why" && window.is_some() {
                shown.why = Why {
                    seq: shown.why.seq + 1,
                    window: window.clone(),
                };
            }
            shown.pick = Pick {
                seq,
                purpose: purpose.to_string(),
                status,
                window,
            };
        }
        self.refresh_windows(proxy).await.ok();
        reply.map(|_| ())
    }
}
