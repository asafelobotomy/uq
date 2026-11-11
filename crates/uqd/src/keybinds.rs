//! Keyboard shortcuts in uqd: the `keybinds` section's explanation and the merged view the page
//! shows. Writes go through `SetSection("keybinds")`, which `commit_section` already renders,
//! verifies, reloads and journals.

use crate::service::{Desktop, DesktopError};
use serde_json::Value;
use uq_core::keybinds;

impl Desktop {
    /// `Explain("keybinds")`: one row per shortcut, from the pure core function. Reads
    /// `hyprctl binds -j` once and `user.lua` once; with Hyprland down every row reports from
    /// state.
    pub(crate) async fn explain_keybinds(&self) -> Value {
        let live = match self.comp.binds().await {
            Ok(json) => Some(keybinds::live_binds(&json)),
            Err(e) => {
                tracing::debug!("reading binds: {e}");
                None
            }
        };
        let text = self.read_user_lua();
        let rejected = self
            .last_rejected
            .as_ref()
            .filter(|(d, _)| d == "keybinds")
            .map(|(_, m)| m.as_str());
        keybinds::explain(
            &self.state.keybinds,
            live.as_deref(),
            text.as_deref(),
            rejected,
            &self.paths.user_lua.to_string_lossy(),
        )
    }

    /// `ListBinds`: uQ's shortcuts with your changes, your own, and the `user.lua` binds marked
    /// `-- uq:bind`, with the key and action catalogues.
    pub fn list_binds(&self) -> Result<String, DesktopError> {
        let text = self.read_user_lua();
        Ok(keybinds::list::list(&self.state.keybinds, text.as_deref()).to_string())
    }
}

#[cfg(test)]
mod tests;
