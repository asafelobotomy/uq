//! Whole-section settings (`appearance`, `wallpaper`, `look`, `input`, ...) for
//! `GetSection` / `SetSection`.

use crate::service::{Desktop, DesktopError, Event, change_for};
use uq_core::history::Origin;
use uq_core::settings::{clean_text, renders_section};
use uq_core::state::State;

/// Entries read per device class; Hyprland's reply is untrusted.
const MAX_DEVICES_PER_CLASS: usize = 32;

impl Desktop {
    /// One settings section as JSON (`appearance` or `wallpaper`).
    pub fn get_section(&self, domain: &str) -> Result<String, DesktopError> {
        let v = self
            .state
            .section(domain)
            .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
        serde_json::to_string(&v).map_err(|e| DesktopError::Io(e.to_string()))
    }

    /// Replace a whole section: parse and validate, then `commit_section`. `monitors` takes
    /// everything but the layout (R7): a changed `outputs` is refused, because displays change
    /// only through the apply/confirm path; the main display and the planner re-render and
    /// reload through `write_monitors`, and every `monitors` write waits for a pending display
    /// change (F6).
    pub async fn set_section(
        &mut self,
        domain: &str,
        json: &str,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| DesktopError::InvalidSetting(format!("value is not JSON: {e}")))?;
        let mut next = self.state.clone();
        if domain == "monitors" {
            if self.pending.is_some() {
                return Err(DesktopError::InvalidSetting(
                    "keep or revert the display change first".into(),
                ));
            }
            next.set_section(domain, value)
                .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
            let (old, new) = (&self.state.settings.monitors, &next.settings.monitors);
            if !new.layout_eq(old) {
                return Err(DesktopError::InvalidSetting(
                    "displays change through ApplyMonitors".into(),
                ));
            }
            let primary_changed = new.primary != old.primary;
            if primary_changed || new.workspaces != old.workspaces {
                // These change the rendered config, and `commit_section` never reloads.
                self.write_monitors(next, origin).await?;
            } else {
                self.commit_section(domain, next, origin).await?;
            }
            if primary_changed {
                self.sync_x11_primary_now().await;
            }
            let _ = self.events.send(Event::MonitorsChanged);
            return Ok(());
        }
        next.set_section(domain, value)
            .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
        if domain == "compat" {
            self.check_compat_files(&next)?;
        }
        if domain == "integration_rules" {
            self.check_integration_rules(&next)?;
        }
        self.commit_section(domain, next, origin).await
    }

    /// The single section save path: every section write is saved, applied and announced here.
    /// `appearance` re-renders through the normal commit (verify, reload, theme events); `look`,
    /// `layouts` and `input` render, verify and reload Hyprland only; `monitors` renders and
    /// verifies but applies nothing (only the confirm path calls it, and the config is already
    /// live); `wallpaper` and `shell` just save, since the shell reads them. Then the layout
    /// events, `SectionChanged` and `SettingsChanged` go out.
    ///
    /// The save is `persist(next, origin)`, reached through `commit` (`install`) or directly, so
    /// each accepted write is journalled exactly once.
    pub(crate) async fn commit_section(
        &mut self,
        domain: &str,
        next: State,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        let before = (domain == "layouts").then(|| self.state.layouts.clone());
        if renders_section(domain) || domain == "monitors" {
            let res = self.commit(next, change_for(domain), origin).await;
            self.note_section_commit(domain, &res);
            res?;
        } else {
            self.persist(next, origin)?;
        }
        if let Some(before) = before {
            // The reload applied the rules; tell the shell which workspaces changed layout. A
            // removed rule falls back to the default layout.
            let fallback = self
                .state
                .settings
                .layout_options
                .default
                .unwrap_or(uq_core::lua::Layout::Dwindle);
            let mut changed: Vec<_> = self
                .state
                .layouts
                .iter()
                .filter(|(ws, l)| before.get(*ws) != Some(*l))
                .map(|(ws, l)| (ws.clone(), *l))
                .collect();
            changed.extend(
                before
                    .keys()
                    .filter(|ws| !self.state.layouts.contains_key(*ws))
                    .map(|ws| (ws.clone(), fallback)),
            );
            for (workspace, layout) in changed {
                let _ = self.events.send(Event::LayoutChanged { workspace, layout });
            }
        }
        let section = self.get_section(domain)?;
        let _ = self.events.send(Event::SectionChanged {
            domain: domain.to_string(),
            json: section,
        });
        let _ = self.events.send(Event::SettingsChanged(Box::new(
            self.state.settings.clone(),
        )));
        Ok(())
    }

    /// Input devices Hyprland reports, as `{keyboards, pointers, touch}` JSON for the settings
    /// page. Names are untrusted: control characters are stripped and they are cut to 128
    /// characters. Hyprland down (or unreadable) gives empty lists.
    pub async fn list_input_devices(&self) -> Result<String, DesktopError> {
        let raw = self.comp.devices().await.unwrap_or_default();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
        let names = |key: &str| -> Vec<(String, f64)> {
            v.get(key)
                .and_then(|a| a.as_array())
                .into_iter()
                .flatten()
                .take(MAX_DEVICES_PER_CLASS)
                .filter_map(|d| {
                    let name = clean_text(d.get("name")?.as_str()?, 128);
                    let speed = d.get("defaultSpeed").and_then(|s| s.as_f64());
                    (!name.is_empty())
                        .then_some((name, speed.filter(|s| s.is_finite()).unwrap_or(0.0)))
                })
                .collect()
        };
        let named = |key: &str| -> Vec<serde_json::Value> {
            names(key)
                .into_iter()
                .map(|(name, _)| serde_json::json!({ "name": name }))
                .collect()
        };
        let pointers: Vec<_> = names("mice")
            .into_iter()
            .map(|(name, speed)| serde_json::json!({ "name": name, "default_speed": speed }))
            .collect();
        serde_json::to_string(&serde_json::json!({
            "keyboards": named("keyboards"),
            "pointers": pointers,
            "touch": named("touch"),
        }))
        .map_err(|e| DesktopError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests;
