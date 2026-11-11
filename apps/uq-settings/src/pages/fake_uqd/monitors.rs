//! The fake uqd's main display, bar, planner and saved arrangements: `SetSection("monitors")`
//! as uqd accepts it (R7, F6), `RememberMonitors`, and the `roles` and `arrangement` of
//! `ListOutputs`, from the same core functions uqd calls.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use uq_core::settings::{LiveOutput, MonitorProfile, Monitors, plan};

use super::devices::Displays;
use super::{FakeError, FakeUqd, invalid};

/// What uqd says when a display change is waiting to be kept.
const WAITING: &str = "keep or revert the display change first";

impl Displays {
    /// Whether the stored layout and planner already are what `p` holds for the screens on
    /// `live`: saving these screens now would give the same arrangement (uqd's `applied`).
    fn applied(stored: &Monitors, p: &MonitorProfile, live: &[LiveOutput]) -> bool {
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

    /// `(roles, arrangement)` as uqd's `ListOutputs` has them.
    pub fn roles_and_arrangement(&self, stored: &Monitors) -> (Value, Value) {
        let live = self.live_outputs.lock().unwrap().clone();
        let roles = plan::roles(stored, &plan::overlay(&live, stored));
        let failed = self.failed.lock().unwrap().clone();
        let arrangement = plan::find_arrangement(stored, &live).map(|p| {
            json!({
                "id": p.id,
                "name": p.name,
                "applied": Self::applied(stored, p, &live),
                "failed": failed.as_deref() == Some(p.id.as_str()),
            })
        });
        (json!(roles), json!(arrangement))
    }
}

impl FakeUqd {
    /// `SetSection("monitors")`: everything but the layout is written; a changed layout is
    /// refused, and so is any write while a display change is waiting.
    pub(super) fn set_monitors_section(&self, json: &str, origin: &str) -> Result<(), FakeError> {
        if self.displays.pending.lock().unwrap().is_some() {
            return Err(invalid(WAITING));
        }
        let value: Value =
            serde_json::from_str(json).map_err(|e| invalid(format!("value is not JSON: {e}")))?;
        let old = self.state.lock().unwrap().settings.monitors.clone();
        self.write(origin, |next| {
            next.set_section("monitors", value).map_err(invalid)?;
            let new = &next.settings.monitors;
            if !new.layout_eq(&old) {
                return Err(invalid("displays change through ApplyMonitors"));
            }
            // The main display and the planner change the rendered config, so Hyprland checks it.
            if (new.primary != old.primary || new.workspaces != old.workspaces)
                && let Some(message) = self.verify_error.lock().unwrap().take()
            {
                return Err(FakeError::ConfigInvalid(format!(
                    "Hyprland rejected the new config: {message}"
                )));
            }
            Ok(())
        })?;
        Ok(())
    }

    /// `RememberMonitors`: the screens connected now, with their stored layout, as an
    /// arrangement. The same set already saved keeps its id.
    pub(super) fn remember(&self, name: &str, origin: &str) -> Result<String, FakeError> {
        if self.displays.pending.lock().unwrap().is_some() {
            return Err(invalid(WAITING));
        }
        let live = self.displays.live_outputs.lock().unwrap().clone();
        let stored = self.state.lock().unwrap().settings.monitors.clone();
        let taken: BTreeSet<String> = stored.profiles.iter().map(|p| p.id.clone()).collect();
        let p = plan::remember(&stored, &live, name, None, &taken).map_err(invalid)?;
        let id = p.id.clone();
        self.write(origin, |s| {
            let list = &mut s.settings.monitors.profiles;
            match list.iter_mut().find(|e| e.id == p.id) {
                Some(slot) => *slot = p,
                None => list.push(p),
            }
            s.settings.monitors.validate().map_err(invalid)
        })?;
        Ok(id)
    }
}
