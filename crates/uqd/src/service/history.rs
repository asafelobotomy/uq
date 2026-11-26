//! History: undo, restore points and the journal.

use super::*;

/// How a conflict message names a journal domain.
pub(crate) fn domain_label(domain: &str) -> &'static str {
    match domain {
        "theme" => "The theme",
        "schedule" => "The theme schedule",
        "appearance" => "Appearance",
        "look" => "Look",
        "layouts" => "Layouts",
        "rules" => "Window rules",
        "keybinds" => "Keyboard shortcuts",
        "monitors" => "Displays",
        "input" => "Input",
        "env" => "The list of environment variables",
        "compat" => "Apps and scaling",
        "autostart" => "The list of startup apps",
        "integration_rules" => "Rules from apps",
        "wallpaper" => "Wallpaper",
        "shell" => "Shell",
        "settings" => "A setting",
        _ => "This setting",
    }
}

impl Desktop {
    /// Ids of the entries the latest `persist` journalled (empty if none).
    pub(crate) fn last_entry_ids(&self) -> &[String] {
        &self.last_entry_ids
    }

    /// The journal, newest first, at most `limit` (capped at [`MAX_ENTRIES`]) entries.
    pub fn history(&self, limit: usize) -> Vec<Entry> {
        self.journal.list(limit.min(MAX_ENTRIES))
    }

    /// Write entry `id`'s `before` back through the normal write path, recorded with origin
    /// `restore`. Only the keys the entry changed are written back. Refused with `Conflict` when a newer entry changed the same domain, unless
    /// `force`. [`Self::last_entry_ids`] then holds what the undo itself journalled.
    pub async fn undo(&mut self, id: &str, force: bool) -> Result<(), DesktopError> {
        let entries = self.journal.list(usize::MAX);
        let Some(pos) = entries.iter().position(|e| e.id == id) else {
            return Err(DesktopError::InvalidArgument(format!(
                "no history entry {id:?}"
            )));
        };
        let entry = &entries[pos];
        if !force && entries[..pos].iter().any(|e| e.domain == entry.domain) {
            return Err(DesktopError::Conflict(format!(
                "{} was changed again after this. Undo anyway?",
                domain_label(&entry.domain)
            )));
        }
        let damaged = |e: serde_json::Error| {
            DesktopError::InvalidArgument(format!("history entry {id:?} is damaged: {e}"))
        };
        let before = entry.before.clone();
        match entry.domain.as_str() {
            "theme" => {
                let name: String =
                    serde_json::from_value(before["theme"].clone()).map_err(damaged)?;
                let variant: Variant =
                    serde_json::from_value(before["variant"].clone()).map_err(damaged)?;
                self.set_theme(&name, variant, Origin::Restore).await?;
            }
            "schedule" => {
                let schedule: Schedule = serde_json::from_value(before).map_err(damaged)?;
                self.set_schedule(schedule, Origin::Restore).await?;
            }
            "settings" => {
                let after = &entry.after;
                let values: Vec<(String, serde_json::Value)> = before
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter(|(k, v)| after.get(k.as_str()) != Some(*v))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                self.set_settings(values, Origin::Restore).await?;
            }
            domain => {
                // Only the keys this entry changed go back; later changes to other keys of the
                // section stay.
                let domain = domain.to_string();
                let current = self
                    .state
                    .section(&domain)
                    .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
                let value = revert_changed(&current, &before, &entry.after);
                if domain == "monitors" {
                    // Displays never take SetSection; an undo goes back to a layout the user
                    // already kept, so it is saved and reloaded without a revert window.
                    let mut next = self.state.clone();
                    next.set_section(&domain, value)
                        .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
                    self.write_monitors(next, Origin::Restore).await?;
                } else {
                    self.set_section(&domain, &value.to_string(), Origin::Restore)
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// Remember a section write Hyprland rejected, so `explain` reports it. Any later commit
    /// that installs a verified config clears it (see `commit`).
    pub(crate) fn note_section_commit(&mut self, domain: &str, res: &Result<(), DesktopError>) {
        if let Err(DesktopError::ConfigInvalid(msg)) = res {
            self.last_rejected = Some((domain.to_string(), msg.clone()));
        }
    }

    /// Restore points, newest first.
    pub fn restore_points(&self) -> Vec<RestorePoint> {
        self.restore.list()
    }

    /// Save the current state as a manual restore point; returns its id.
    pub fn save_restore_point(&self, name: &str) -> Result<String, DesktopError> {
        self.restore
            .save(&self.state, name, PointKind::Manual, Utc::now().timestamp())
            .map(|p| p.id)
            .map_err(io_err("saving restore point"))
    }

    pub fn delete_restore_point(&self, id: &str) -> Result<(), DesktopError> {
        self.restore.delete(id).map_err(|e| match e.kind() {
            std::io::ErrorKind::InvalidInput => DesktopError::InvalidArgument(e.to_string()),
            _ => DesktopError::Io(format!("deleting restore point: {e}")),
        })
    }

    /// Bring back every domain of restore point `id` as one write, keeping today's launch
    /// counts. A damaged point or one whose theme is gone changes nothing; otherwise an
    /// automatic point of the current state is saved first. Returns plain notices (the power
    /// mode of a profile the restore ended).
    pub async fn restore(&mut self, id: &str) -> Result<Vec<String>, DesktopError> {
        let mut next = self.restore.load(id).map_err(|e| match e.kind() {
            std::io::ErrorKind::InvalidData => {
                DesktopError::InvalidArgument("restore point is damaged".into())
            }
            std::io::ErrorKind::InvalidInput | std::io::ErrorKind::NotFound => {
                DesktopError::InvalidArgument(e.to_string())
            }
            _ => DesktopError::Io(format!("reading restore point: {e}")),
        })?;
        if self.themes.get(&next.theme, next.variant).is_none() {
            return Err(DesktopError::UnknownTheme(next.theme));
        }
        if let Err(e) = self.restore.save(
            &self.state,
            "Before restoring a restore point",
            PointKind::Auto(AutoReason::BeforeRestore),
            Utc::now().timestamp(),
        ) {
            tracing::warn!("[STATE-005] saving before-restore point: {e}");
        }
        next.launch_counts = self.state.launch_counts.clone();
        // Whether Welcome ran is not part of a restore point's settings.
        next.welcome = self.state.welcome;
        self.commit(next, Change::Theme, Origin::Restore).await?;
        self.sync_x11_primary_now().await;
        self.announce_all();
        // The restored settings aren't the profile's any more, and neither is its power mode,
        // which a restore point doesn't hold: put it back as "Back to normal" does (K2: a
        // failure is a notice, never an error).
        let mut notices = Vec::new();
        if let Some(a) = self.active.take() {
            if let Some(pm) = &a.power {
                self.power_back(pm, &mut Vec::new(), &mut notices).await;
            }
            self.forget_active();
        }
        Ok(notices)
    }

    /// Keep a restore point of the state a new uQ version found, once per version.
    pub(crate) fn save_new_version_point(&self) {
        let version = env!("CARGO_PKG_VERSION");
        if self
            .restore
            .last_auto(AutoReason::NewVersion)
            .is_some_and(|p| p.name == version)
        {
            return;
        }
        if let Err(e) = self.restore.save(
            &self.state,
            version,
            PointKind::Auto(AutoReason::NewVersion),
            Utc::now().timestamp(),
        ) {
            tracing::warn!("[STATE-005] saving new-version restore point: {e}");
        }
    }
}
