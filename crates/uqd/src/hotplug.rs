//! Switching display arrangements when screens come and go (R18), moving workspaces back, and
//! telling X11 apps which screen is the main one. The pieces are `uq_core::settings::plan`'s
//! pure functions; this file only holds the order they run in.

use crate::compositor::CompositorError;
use crate::outputs::applied;
use crate::service::{Change, Desktop, Event};
use std::collections::BTreeSet;
use uq_core::history::Origin;
use uq_core::lua::workspace_move_line;
use uq_core::settings::{
    LiveOutput, MonitorId, Monitors, Strategy, clean_text_spaced, counts_in_sets, parse_live, plan,
    stays_visible, valid_output_name,
};

/// What uqd remembers between screen changes.
#[derive(Debug, Default)]
pub(crate) struct HotplugState {
    /// The screens and stored settings the matcher last handled; the same again is no work.
    /// `None` until uqd's first match at startup.
    pub last_set: Option<Handled>,
    /// A screen change came while a display change was waiting: whether any of them was an add.
    pub held: Option<bool>,
    /// Arrangements already reported as unusable for the current set of screens.
    pub notified: BTreeSet<String>,
    /// The connector last given to `xrandr`.
    pub last_primary: Option<String>,
}

/// What the matcher last saw. Arrangements switch only when `ids` changes (R18).
#[derive(Debug, Clone)]
pub(crate) struct Handled {
    /// The connected screens by identity (sorted).
    pub ids: Vec<MonitorId>,
    /// Which connector each screen is on (sorted), so twins swapping ports are noticed.
    pub conns: Vec<(String, MonitorId)>,
    pub stored: Monitors,
}

fn connectors(live: &[LiveOutput]) -> Vec<(String, MonitorId)> {
    let mut v: Vec<(String, MonitorId)> = live
        .iter()
        .filter(|o| counts_in_sets(&o.name))
        .map(|o| (o.name.clone(), MonitorId::of(o)))
        .collect();
    v.sort();
    v
}

const NOT_USED: &str = "Display arrangement not used";

impl Desktop {
    /// The screens changed (`added`: one came). Runs R18's steps in order; returns without
    /// work when a display change is waiting (it runs when that window closes) or when the
    /// screens and settings are what was last handled.
    pub async fn outputs_changed(&mut self, added: bool) {
        if self.pending.is_some() {
            self.hotplug.held = Some(self.hotplug.held.unwrap_or(false) | added);
            return;
        }
        self.hotplug.held = None;
        let live = match self.comp.monitors().await {
            Ok(j) => parse_live(&j),
            Err(e) => {
                tracing::debug!("not matching display arrangements: {e}");
                return;
            }
        };
        let ids = plan::connected_set(&live);
        let conns = connectors(&live);
        let startup = self.hotplug.last_set.is_none();
        let set_changed = match &self.hotplug.last_set {
            Some(h) if h.conns == conns && h.stored == self.state.settings.monitors => return,
            Some(h) => h.ids != ids,
            None => true,
        };
        if set_changed {
            self.hotplug.notified.clear();
        }
        // A saved arrangement replaces the layout only when the screens are not the same ones
        // as last time (a kept, undone or restored layout must survive). At startup, the
        // stored layout already covering every screen means the set did not change while uqd
        // was stopped.
        let may_switch = if startup {
            !covers(&self.state.settings.monitors, &live)
        } else {
            set_changed
        };
        let wrote = may_switch && self.switch_arrangement(&live).await;
        if wrote {
            self.hotplug.notified.clear();
        }
        if !wrote && needs_render(&self.state) {
            // The plan and the main display follow the new screens; nothing in the state changes.
            let state = self.state.clone();
            if let Err(e) = self.commit(state, Change::Reload, Origin::Hotplug).await {
                tracing::warn!("planning workspaces for the new screens: {e}");
            }
        }
        if added {
            self.move_workspaces_back(&live).await;
        }
        self.sync_x11_primary(&live).await;
        self.hotplug.last_set = Some(Handled {
            ids,
            conns,
            stored: self.state.settings.monitors.clone(),
        });
        let _ = self.events.send(Event::MonitorsChanged);
    }

    /// The window of a display change just closed: run a screen change held meanwhile (R9).
    pub(crate) async fn after_window(&mut self) {
        if let Some(added) = self.hotplug.held.take() {
            self.outputs_changed(added).await;
        }
    }

    /// Steps 3 and 4: write the arrangement saved for these screens, unless it is already in
    /// place. A refusal changes nothing and is reported once per set of screens. True when
    /// something was written.
    async fn switch_arrangement(&mut self, live: &[LiveOutput]) -> bool {
        let stored = &self.state.settings.monitors;
        if !stored.auto_profiles {
            return false;
        }
        let Some(p) = plan::find_arrangement(stored, live).cloned() else {
            return false;
        };
        if applied(stored, &p, live) {
            return false;
        }
        let Some(next) = plan::arranged(stored, &p, live) else {
            return false;
        };
        let refusal = if !stays_visible(&next, live) {
            Some("at least one display must stay on".to_string())
        } else if let Err(e) = next.validate() {
            Some(e.to_string())
        } else {
            let mut state = self.state.clone();
            state.settings.monitors = next;
            match self.write_monitors(state, Origin::Hotplug).await {
                Ok(()) => None,
                Err(e) => Some(e.to_string()),
            }
        };
        let Some(reason) = refusal else {
            return true;
        };
        tracing::warn!("display arrangement {:?} not used: {reason}", p.name);
        if self.hotplug.notified.insert(p.id.clone()) {
            let body = format!(
                "“{}” couldn't be used with these screens: {}",
                p.name,
                clean_text_spaced(&reason, 200)
            );
            // Outside the desktop lock: a slow bus must not hold it.
            let notifier = self.notifier.clone();
            tokio::spawn(async move { notifier.notify(NOT_USED, &body).await });
        }
        false
    }

    /// Step 6 (R16): put each planned workspace that exists on another screen back on its own.
    /// Errors are logged, never shown.
    async fn move_workspaces_back(&self, live: &[LiveOutput]) {
        let m = &self.state.settings.monitors;
        if !m.workspaces.move_back || m.workspaces.strategy == Strategy::Off {
            return;
        }
        let wanted = plan::plan(&m.workspaces, &plan::screens(m, live));
        if wanted.is_empty() {
            return;
        }
        let have: Vec<(i64, String)> = match self.comp.workspaces().await {
            Ok(j) => serde_json::from_str::<Vec<serde_json::Value>>(&j)
                .unwrap_or_default()
                .iter()
                .filter_map(|w| {
                    Some((
                        w.get("id")?.as_i64()?,
                        w.get("monitor")?.as_str()?.to_string(),
                    ))
                })
                .collect(),
            Err(e) => {
                tracing::warn!("reading workspaces: {e}");
                return;
            }
        };
        for w in wanted {
            let sits_elsewhere = have
                .iter()
                .any(|(id, mon)| *id == i64::from(w.workspace) && *mon != w.output);
            if !sits_elsewhere || !valid_output_name(&w.output) {
                continue;
            }
            let line = format!(
                "hl.dispatch({})",
                workspace_move_line(w.workspace, &w.output)
            );
            match self.comp.eval(&line).await {
                Ok(()) => {}
                Err(CompositorError::NotRunning) => return,
                Err(e) => tracing::warn!("moving workspace {} back: {e}", w.workspace),
            }
        }
    }

    /// Step 7 (R11): tell X11 apps about an explicitly chosen main display, once per connector.
    pub(crate) async fn sync_x11_primary(&mut self, live: &[LiveOutput]) {
        let m = &self.state.settings.monitors;
        let primary = plan::roles(m, &plan::overlay(live, m)).primary;
        if primary.is_empty() {
            self.hotplug.last_primary = None;
            return;
        }
        if self.hotplug.last_primary.as_deref() == Some(primary.as_str()) {
            return;
        }
        self.hotplug.last_primary = Some(primary.clone());
        if let Err(e) = self.xrandr.set_primary(&primary).await {
            tracing::warn!("setting the X11 main display to {primary}: {e}");
        }
    }

    /// After a commit that may have changed the main display or the layout (Keep, undo,
    /// restore, a main-display change): re-apply the X11 primary, and record the screens so
    /// the burst this change causes is not mistaken for new screens.
    pub(crate) async fn sync_x11_primary_now(&mut self) {
        let Ok(j) = self.comp.monitors().await else {
            return;
        };
        let live = parse_live(&j);
        self.sync_x11_primary(&live).await;
        if self.hotplug.last_set.is_some() {
            self.hotplug.last_set = Some(Handled {
                ids: plan::connected_set(&live),
                conns: connectors(&live),
                stored: self.state.settings.monitors.clone(),
            });
        }
    }
}

/// Whether the stored layout has an entry for every connected screen.
fn covers(m: &Monitors, live: &[LiveOutput]) -> bool {
    live.iter()
        .filter(|o| counts_in_sets(&o.name))
        .all(|o| m.outputs.contains_key(&o.name))
}

/// Whether a render reads the screens (the planner, a chosen main display, scaling compat).
fn needs_render(state: &uq_core::state::State) -> bool {
    let m = &state.settings.monitors;
    m.workspaces.strategy != Strategy::Off
        || m.primary.is_some()
        || state.compat.sharp_x11
        || state.compat.steam_scale
}

#[cfg(test)]
mod tests;
