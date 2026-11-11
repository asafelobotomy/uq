//! What the Displays page and the shell read about the screens, and saving an arrangement.

use crate::compositor::CompositorError;
use crate::service::{Desktop, DesktopError, Event};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use uq_core::history::Origin;
use uq_core::settings::{LiveOutput, MonitorProfile, Monitors, parse_live, plan};
use uq_core::state::State;

const NO_DESKTOP: &str = "displays can't be read while the desktop isn't running";
const WAITING: &str = "keep or revert the display change first";

/// Whether the stored layout and planner already are what `p` holds for the screens on `live`:
/// saving these screens now would give the same arrangement. A screen with no stored entry
/// counts by its live mode, as it does when saving.
pub(crate) fn applied(stored: &Monitors, p: &MonitorProfile, live: &[LiveOutput]) -> bool {
    let Ok(now) = plan::remember(stored, live, &p.name, None, &BTreeSet::new()) else {
        return false;
    };
    let key = |o: &uq_core::settings::SavedOutput| {
        (
            o.monitor.clone(),
            serde_json::to_string(&o.config).unwrap_or_default(),
        )
    };
    let mut want: Vec<_> = p.outputs.iter().map(key).collect();
    let mut have: Vec<_> = now.outputs.iter().map(key).collect();
    want.sort();
    have.sort();
    want == have
        && p.workspaces
            .as_ref()
            .is_none_or(|w| *w == stored.workspaces)
}

/// `{live, stored, roles, arrangement}`. `roles` names screens by connector (with the stored
/// layout laid over them, as the next reload sees them); `arrangement` is the one saved for the
/// connected set, or `null`. `failed` is the id of an arrangement a switch was skipped for.
pub(crate) fn outputs_json(state: &State, live: &[LiveOutput], failed: Option<&str>) -> Value {
    let stored = &state.settings.monitors;
    let roles = plan::roles(stored, &plan::overlay(live, stored));
    let arrangement = plan::find_arrangement(stored, live).map(|p| {
        json!({
            "id": p.id,
            "name": p.name,
            "applied": applied(stored, p, live),
            "failed": failed == Some(p.id.as_str()),
        })
    });
    json!({
        "live": live,
        "stored": stored,
        "roles": roles,
        "arrangement": arrangement,
    })
}

impl Desktop {
    /// Save the screens connected now, with their stored layout, as an arrangement called
    /// `name` (R8). The same set already saved keeps its id; returns the id.
    pub async fn remember_monitors(
        &mut self,
        name: &str,
        origin: Origin,
    ) -> Result<String, DesktopError> {
        if self.pending.is_some() {
            return Err(DesktopError::InvalidSetting(WAITING.into()));
        }
        let live = match self.comp.monitors().await {
            Ok(j) => parse_live(&j),
            Err(CompositorError::NotRunning) => {
                return Err(DesktopError::InvalidSetting(NO_DESKTOP.into()));
            }
            Err(e) => return Err(DesktopError::Io(e.to_string())),
        };
        let stored = &self.state.settings.monitors;
        let taken: BTreeSet<String> = stored.profiles.iter().map(|p| p.id.clone()).collect();
        let p = plan::remember(stored, &live, name, None, &taken)
            .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
        let id = p.id.clone();
        let mut next = self.state.clone();
        let list = &mut next.settings.monitors.profiles;
        match list.iter_mut().find(|e| e.id == id) {
            Some(slot) => *slot = p,
            None => list.push(p),
        }
        next.settings
            .monitors
            .validate()
            .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
        self.commit_section("monitors", next, origin).await?;
        let _ = self.events.send(Event::MonitorsChanged);
        Ok(id)
    }
}

#[cfg(test)]
mod tests;
