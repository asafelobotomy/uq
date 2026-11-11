//! Window and workspace rules in uqd: the `rules` section's explanation and the window list,
//! and the window rules from apps (`integration_rules`, below). Writes go through
//! `SetSection("rules")`, which `commit_section` already renders and verifies.

use crate::compositor::Compositor;
use crate::service::{Desktop, DesktopError};
use serde_json::{Value, json};
use uq_core::integration::rules::{self as app_rules, AppRule};
use uq_core::profile;
use uq_core::rules::{self, WindowInfo, WindowRule};
use uq_core::state::State;

impl Desktop {
    /// `Explain("rules")`: one row per rule, from the pure core function, with the rules whose
    /// switch the active profile still holds marked as the profile's. Reads `user.lua` once.
    pub(crate) fn explain_rules(&self) -> serde_json::Value {
        let text = self.read_user_lua();
        let rejected = self
            .last_rejected
            .as_ref()
            .filter(|(d, _)| d == "rules")
            .map(|(_, m)| m.as_str());
        let mut rows = rules::explain(
            &self.state.rules,
            text.as_deref(),
            rejected,
            &self.paths.user_lua.to_string_lossy(),
        );
        if let Some(a) = &self.active {
            profile::mark_rules(&mut rows, &self.state.rules, a);
        }
        rows
    }

    /// The open windows from `clients -j`; empty when Hyprland is down or answers badly. Takes
    /// the compositor, not `&self`, so callers need not hold the Desktop lock for the read.
    pub async fn windows_of(comp: &dyn Compositor) -> Vec<WindowInfo> {
        match comp.clients().await {
            Ok(json) => rules::windows_from_clients(&json),
            Err(e) => {
                tracing::debug!("reading windows: {e}");
                Vec::new()
            }
        }
    }

    /// The open windows' rule-relevant fields. Hyprland down is `Ok(vec![])`.
    pub async fn list_windows(&self) -> Result<Vec<WindowInfo>, DesktopError> {
        Ok(Self::windows_of(self.comp.as_ref()).await)
    }
}

/// Rules from apps (Phase 4 spec §3.3): the `integration_rules` domain. Writes go through
/// `SetSection("integration_rules")`: `set_section` refuses an id being turned on that isn't
/// installed and valid ([`Desktop::check_integration_rules`]), then `commit_section` renders,
/// verifies and saves through `persist`, so undo and history work as for every section.
///
/// The packaged files are read again at every render. An id that is on but whose rule is gone
/// (the app was removed, or its file no longer passes the checks) is left out of
/// `generated.lua` and kept in the list, so the rule is back on when the app is.
impl Desktop {
    /// The window rules from apps that `st` turns on and that are installed, ready to render.
    pub(crate) fn app_rules_for(&self, st: &State) -> Vec<WindowRule> {
        if st.integration_rules.enabled.is_empty() {
            return Vec::new();
        }
        let installed = app_rules::installed(&self.paths.integrations_dir);
        app_rules::enabled_rules(&installed, &st.integration_rules)
            .iter()
            .flat_map(AppRule::window_rules)
            .collect()
    }

    /// Every id `next` turns on that `self.state` doesn't must name a valid installed rule. Ids
    /// already on are accepted again (every write resends them), installed or not.
    pub(crate) fn check_integration_rules(&self, next: &State) -> Result<(), DesktopError> {
        let new = next
            .integration_rules
            .turned_on(&self.state.integration_rules);
        if new.is_empty() {
            return Ok(());
        }
        let files =
            app_rules::scan_rules(&self.paths.integrations_dir).map_err(DesktopError::Io)?;
        for id in new {
            let file = files
                .iter()
                .find(|f| f.integration == app_rules::integration_of(id));
            match file.map(|f| &f.rules) {
                Some(Ok(rules)) if rules.iter().any(|r| r.id == id) => {}
                Some(Err(e)) => {
                    return Err(DesktopError::InvalidSetting(format!(
                        "The rules from {} can't be used: {e}",
                        app_rules::integration_of(id)
                    )));
                }
                _ => {
                    return Err(DesktopError::InvalidSetting(format!(
                        "The rule {id} isn't installed."
                    )));
                }
            }
        }
        Ok(())
    }

    /// `ListIntegrationRules`: `{rules, problems, missing}`. `rules`: every valid packaged rule
    /// ([`AppRule::to_json`]) with `enabled`; `problems`: `{integration, path, error}` for each
    /// file that was refused; `missing`: ids that are on but have no installed rule.
    pub fn list_integration_rules(&self) -> Value {
        let files = match app_rules::scan_rules(&self.paths.integrations_dir) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("integration rules: {e}");
                Vec::new()
            }
        };
        let on = &self.state.integration_rules.enabled;
        let mut rules = Vec::new();
        let mut problems = Vec::new();
        for f in files {
            match f.rules {
                Ok(list) => rules.extend(list.iter().map(|r| {
                    let mut v = r.to_json();
                    v["enabled"] = json!(on.contains(&r.id));
                    v
                })),
                Err(e) => problems.push(json!({
                    "integration": f.integration,
                    "path": f.path.to_string_lossy(),
                    "error": e.to_string(),
                })),
            }
        }
        let missing: Vec<&String> = on
            .iter()
            .filter(|id| !rules.iter().any(|r| r["id"] == json!(id)))
            .collect();
        json!({ "rules": rules, "problems": problems, "missing": missing })
    }

    /// `Explain("integration_rules")`: one row per id that is on, `source` `uq` (or `error` with
    /// Hyprland's message when the last write was refused) and `detail.installed`.
    pub(crate) fn explain_integration_rules(&self) -> Value {
        let rejected = self
            .last_rejected
            .as_ref()
            .filter(|(d, _)| d == app_rules::DOMAIN)
            .map(|(_, m)| m.clone());
        let installed = app_rules::installed(&self.paths.integrations_dir);
        let rows: serde_json::Map<String, Value> = self
            .state
            .integration_rules
            .enabled
            .iter()
            .map(|id| {
                let here = installed.iter().any(|r| &r.id == id);
                let row = match &rejected {
                    Some(m) => {
                        json!({"source": "error", "detail": {"message": m, "installed": here}})
                    }
                    None => json!({"source": "uq", "detail": {"installed": here}}),
                };
                (id.clone(), row)
            })
            .collect();
        Value::Object(rows)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod app_tests;
