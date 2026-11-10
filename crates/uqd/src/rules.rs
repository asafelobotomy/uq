//! Window and workspace rules in uqd: the `rules` section's explanation and the window list.
//! Writes go through `SetSection("rules")`, which `commit_section` already renders and verifies.

use crate::compositor::Compositor;
use crate::service::{Desktop, DesktopError};
use uq_core::rules::{self, WindowInfo};

impl Desktop {
    /// `Explain("rules")`: one row per rule, from the pure core function. Reads `user.lua` once.
    pub(crate) fn explain_rules(&self) -> serde_json::Value {
        let text = self.read_user_lua();
        let rejected = self
            .last_rejected
            .as_ref()
            .filter(|(d, _)| d == "rules")
            .map(|(_, m)| m.as_str());
        rules::explain(
            &self.state.rules,
            text.as_deref(),
            rejected,
            &self.paths.user_lua.to_string_lossy(),
        )
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

#[cfg(test)]
mod tests;
