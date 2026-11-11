//! The fake uqd's open windows and window picker: a window list, a queue of scripted pick
//! replies, and a switch that makes `PickWindow` fail as uqd does when one is already pending.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use uq_core::rules::WindowInfo;

use super::FakeError;

#[derive(Clone, Default)]
pub struct FakeWindows {
    pub windows: Arc<Mutex<Vec<WindowInfo>>>,
    /// Replies for `PickWindow`, oldest first: `{status, window?}`. With none left, the pick is
    /// `cancelled`.
    pub picks: Arc<Mutex<VecDeque<Value>>>,
    /// When set, `PickWindow` fails with `Conflict`.
    pub conflict: Arc<AtomicBool>,
    /// What `ListIntegrationRules` lists: `{rules, problems}` (each rule as
    /// `AppRule::to_json`); `enabled` and `missing` come from the state.
    pub app_rules: Arc<Mutex<Value>>,
}

impl FakeWindows {
    pub fn set(&self, windows: Vec<WindowInfo>) {
        *self.windows.lock().unwrap() = windows;
    }

    /// Queue the next pick's reply.
    pub fn script(&self, reply: Value) {
        self.picks.lock().unwrap().push_back(reply);
    }

    /// Queue a pick that clicked `window`.
    pub fn script_picked(&self, window: &WindowInfo) {
        self.script(json!({"status": "picked", "window": window}));
    }

    /// Make every `PickWindow` fail with `Conflict`.
    pub fn fail_picks(&self) {
        self.conflict.store(true, Ordering::SeqCst);
    }

    /// `ListWindows`.
    pub fn list(&self) -> String {
        serde_json::to_string(&*self.windows.lock().unwrap()).unwrap()
    }

    /// Install packaged rules (`rules.toml` text per integration) for `ListIntegrationRules`,
    /// checked as uqd checks them: a refused file is listed under `problems`.
    pub fn install_rules(&self, files: &[(&str, &str)]) {
        let (mut rules, mut problems) = (Vec::new(), Vec::new());
        for (name, text) in files {
            match uq_core::integration::rules::parse_rules(text, name) {
                Ok(list) => rules.extend(list.iter().map(|r| r.to_json())),
                Err(e) => problems.push(json!({"integration": name,
                    "path": format!("/usr/share/uq/integrations/{name}/rules.toml"),
                    "error": e.to_string()})),
            }
        }
        *self.app_rules.lock().unwrap() = json!({"rules": rules, "problems": problems});
    }

    /// `ListIntegrationRules` for the ids that are on.
    pub fn list_app_rules(&self, on: &[String]) -> String {
        let v = self.app_rules.lock().unwrap().clone();
        let mut rules: Vec<Value> = v["rules"].as_array().cloned().unwrap_or_default();
        for r in &mut rules {
            r["enabled"] = json!(on.iter().any(|id| r["id"] == json!(id)));
        }
        let missing: Vec<&String> = on
            .iter()
            .filter(|id| !rules.iter().any(|r| r["id"] == json!(id)))
            .collect();
        json!({"rules": rules, "problems": v.get("problems").cloned().unwrap_or(json!([])),
            "missing": missing})
        .to_string()
    }

    /// `PickWindow`.
    pub fn pick(&self) -> Result<String, FakeError> {
        if self.conflict.load(Ordering::SeqCst) {
            return Err(FakeError::Conflict("Already choosing a window.".into()));
        }
        let reply = self
            .picks
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| json!({"status": "cancelled"}));
        Ok(reply.to_string())
    }
}
