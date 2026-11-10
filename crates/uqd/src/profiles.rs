//! Settings profiles (spec §4): keep them, and apply or turn one off as one validated write.
//!
//! Applying saves a restore point and the active file first, then commits the profile's keys
//! with origin `profile:<id>`; only then is the power mode changed. A failed commit puts the
//! active file back, so the settings, the active profile and its file stay as they were.

use crate::service::{Change, Desktop, DesktopError, Event, io_err};
use chrono::Utc;
use serde::Serialize;
use std::io::ErrorKind;
use uq_core::history::{AutoReason, Origin, PointKind};
use uq_core::profile::{
    self, ActiveProfile, Kept, MAX_PROFILES, PowerMemo, Profile, ProfileError, clear_active,
    key_label, power_notice, save_active, turn_off,
};

/// One profile as `ListProfiles` shows it.
#[derive(Debug, Serialize)]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    /// How many settings it changes, not counting the power mode.
    pub keys: usize,
    pub power: Option<String>,
    pub active: bool,
}

/// What an apply or "Back to normal" did: the profile now on (`""` for none), the keys left
/// alone because they were changed by hand, and plain notices (the power mode).
#[derive(Debug, Default, PartialEq, Serialize)]
pub struct ProfileReport {
    pub active: String,
    pub kept: Vec<Kept>,
    pub notices: Vec<String>,
}

fn refused(e: ProfileError) -> DesktopError {
    DesktopError::InvalidSetting(e.to_string())
}

fn no_profile(id: &str) -> DesktopError {
    DesktopError::InvalidArgument(format!("no profile {id:?}"))
}

/// The power mode, reported as kept when it was changed by hand.
fn power_kept() -> Kept {
    let path = vec!["profile".to_string()];
    Kept {
        label: key_label("power", &path),
        domain: "power".into(),
        path,
    }
}

impl Desktop {
    /// Every profile, by name.
    pub fn list_profiles(&self) -> Vec<ProfileSummary> {
        let active = self.active.as_ref().map(|a| a.id.as_str());
        self.profiles
            .list()
            .into_iter()
            .map(|p| ProfileSummary {
                keys: p.keys().iter().filter(|k| k.domain != "power").count(),
                power: p.power().map(str::to_string),
                active: active == Some(p.id.as_str()),
                id: p.id,
                name: p.name,
                icon: p.icon,
            })
            .collect()
    }

    /// The profile's file text.
    pub fn get_profile(&self, id: &str) -> Result<String, DesktopError> {
        self.load_profile(id).map(|p| p.to_toml())
    }

    /// Save a profile from its file text; returns its id. An existing id is replaced only when
    /// `replace` is set. Changes no setting, even for the active profile.
    pub fn set_profile(&mut self, toml: &str, replace: bool) -> Result<String, DesktopError> {
        let p = Profile::parse(toml).map_err(refused)?;
        if p.values
            .get("appearance")
            .is_some_and(|a| a.get("theme").is_some())
        {
            let (next, _) = profile::apply(&self.state, &p, 0).map_err(refused)?;
            if self.themes.get(&next.theme, next.variant).is_none() {
                return Err(DesktopError::UnknownTheme(next.theme));
            }
        }
        let exists = self
            .paths
            .profiles_dir
            .join(format!("{}.toml", p.id))
            .exists();
        if exists && !replace {
            let name = self.profiles.load(&p.id).map_or(p.id.clone(), |e| e.name);
            return Err(DesktopError::Conflict(format!(
                "A profile called “{name}” already exists. Replace it?"
            )));
        }
        if !exists && self.profiles.list().len() >= MAX_PROFILES {
            return Err(DesktopError::InvalidArgument(format!(
                "You can keep up to {MAX_PROFILES} profiles."
            )));
        }
        self.profiles
            .save(&p)
            .map_err(io_err("saving the profile"))?;
        self.send_profile_changed();
        Ok(p.id)
    }

    /// Delete a profile that isn't on.
    pub fn delete_profile(&mut self, id: &str) -> Result<(), DesktopError> {
        if self.active.as_ref().is_some_and(|a| a.id == id) {
            return Err(DesktopError::InvalidArgument(
                "Turn this profile off before deleting it.".into(),
            ));
        }
        self.profiles.delete(id).map_err(|e| match e.kind() {
            ErrorKind::NotFound | ErrorKind::InvalidInput => no_profile(id),
            _ => DesktopError::Io(format!("deleting the profile: {e}")),
        })?;
        self.send_profile_changed();
        Ok(())
    }

    pub fn active_profile(&self) -> Option<&ActiveProfile> {
        self.active.as_ref()
    }

    /// Turn profile `id` on. A profile already on is turned off in the same write (K3), so a
    /// switch is one commit. The power mode follows the write and never fails it.
    pub async fn apply_profile(&mut self, id: &str) -> Result<ProfileReport, DesktopError> {
        let p = self.load_profile(id)?;
        let old = self.active.clone();
        let (base, mut kept) = match &old {
            Some(a) => turn_off(&self.state, a).map_err(refused)?,
            None => (self.state.clone(), Vec::new()),
        };
        let now = Utc::now().timestamp();
        let (next, mut memo) = profile::apply(&base, &p, now).map_err(refused)?;
        if self.themes.get(&next.theme, next.variant).is_none() {
            return Err(DesktopError::UnknownTheme(next.theme));
        }
        // Until the power step below, the old profile's power mode is still in force.
        let old_power = old.as_ref().and_then(|a| a.power.clone());
        memo.power = old_power.clone();

        let point = match self.restore.save(
            &self.state,
            &format!("Before the {} profile", p.name),
            PointKind::Auto(AutoReason::BeforeProfile),
            now,
        ) {
            Ok(point) => Some(point.id),
            Err(e) => {
                tracing::warn!("saving before-profile point: {e}");
                None
            }
        };
        let active_file = self.paths.profile_active.clone();
        if let Err(e) = save_active(&active_file, &memo) {
            self.drop_point(point);
            return Err(io_err("saving the active profile")(e));
        }
        let res = self
            .commit(next, Change::Theme, Origin::Profile(p.id.clone()))
            .await;
        if let Err(e) = res {
            let put_back = match &old {
                Some(a) => save_active(&active_file, a),
                None => clear_active(&active_file),
            };
            if let Err(err) = put_back {
                tracing::warn!("putting the active profile file back: {err}");
            }
            self.drop_point(point);
            return Err(e);
        }
        self.announce_all();

        let mut notices = Vec::new();
        memo.power = self
            .switch_power(p.power(), old_power.as_ref(), &mut kept, &mut notices)
            .await;
        if let Err(e) = save_active(&active_file, &memo) {
            tracing::warn!("saving the active profile: {e}");
        }
        self.active = Some(memo);
        self.send_profile_changed();
        Ok(ProfileReport {
            active: p.id,
            kept,
            notices,
        })
    }

    /// "Back to normal" (K3): put back what the profile replaced, except keys changed by hand
    /// since, which are kept and named. Journalled as `profile:<id>`.
    pub async fn profile_off(&mut self) -> Result<ProfileReport, DesktopError> {
        let Some(a) = self.active.clone() else {
            return Ok(ProfileReport::default());
        };
        let (next, mut kept) = turn_off(&self.state, &a).map_err(refused)?;
        if next != self.state {
            self.commit(next, Change::Theme, Origin::Profile(a.id.clone()))
                .await?;
            self.announce_all();
        }
        let mut notices = Vec::new();
        if let Some(pm) = &a.power {
            // A set that timed out may have landed since, so read the mode rather than assume.
            match self.power_active().await {
                Ok(cur) if cur == pm.set && cur != pm.before => {
                    if let Err(e) = self.power_set(&pm.before).await {
                        notices.push(power_notice(&e.to_string()));
                    }
                }
                Ok(cur) if cur == pm.set || cur == pm.before => {}
                Ok(_) => kept.push(power_kept()),
                Err(e) => notices.push(power_notice(&e.to_string())),
            }
        }
        self.active = None;
        self.forget_active();
        Ok(ProfileReport {
            active: String::new(),
            kept,
            notices,
        })
    }

    /// Whether the active profile sets the light/dark variant, which holds the schedule.
    pub(crate) fn profile_holds_variant(&self) -> bool {
        self.active.as_ref().is_some_and(|a| {
            a.keys
                .iter()
                .any(|m| m.domain == "appearance" && m.path == ["variant"])
        })
    }

    /// Remove the active file and announce that no profile is on. `self.active` is the
    /// caller's to clear.
    pub(crate) fn forget_active(&self) {
        if let Err(e) = clear_active(&self.paths.profile_active) {
            tracing::warn!("removing the active profile file: {e}");
        }
        let _ = self.events.send(Event::ProfileChanged(String::new()));
    }

    fn send_profile_changed(&self) {
        let id = self.active.as_ref().map_or("", |a| a.id.as_str());
        let _ = self.events.send(Event::ProfileChanged(id.to_string()));
    }

    fn load_profile(&self, id: &str) -> Result<Profile, DesktopError> {
        self.profiles.load(id).map_err(|e| match e.kind() {
            ErrorKind::NotFound | ErrorKind::InvalidInput => no_profile(id),
            ErrorKind::InvalidData => DesktopError::InvalidSetting(e.to_string()),
            _ => DesktopError::Io(format!("reading the profile: {e}")),
        })
    }

    /// Delete the before-profile point of an apply that changed nothing.
    fn drop_point(&self, point: Option<String>) {
        if let Some(id) = point
            && let Err(e) = self.restore.delete(&id)
        {
            tracing::warn!("removing before-profile point: {e}");
        }
    }

    /// Set the power mode for a newly applied profile, and return what it should remember.
    /// `target` is the new profile's mode; `old` is the memo of the profile it replaced. With
    /// no target, the old profile's mode is put back if it's still in force (K3). Failures are
    /// notices; a hand-changed mode is kept and named.
    async fn switch_power(
        &self,
        target: Option<&str>,
        old: Option<&PowerMemo>,
        kept: &mut Vec<Kept>,
        notices: &mut Vec<String>,
    ) -> Option<PowerMemo> {
        if target.is_none() && old.is_none() {
            return None;
        }
        let cur = match self.power_active().await {
            Ok(cur) => cur,
            Err(e) => {
                notices.push(power_notice(&e.to_string()));
                return old.cloned();
            }
        };
        // The old profile's mode is in force when the current one is still what it set.
        let in_force = old.filter(|o| o.set == cur);
        if let Some(o) = old
            && cur != o.set
            && cur != o.before
        {
            kept.push(power_kept());
        }
        let before = in_force.map_or_else(|| cur.clone(), |o| o.before.clone());
        let Some(target) = target else {
            if in_force.is_some()
                && cur != before
                && let Err(e) = self.power_set(&before).await
            {
                notices.push(power_notice(&e.to_string()));
                return in_force.cloned();
            }
            return None;
        };
        let memo = PowerMemo {
            before,
            set: target.to_string(),
        };
        if cur == target {
            return Some(memo);
        }
        match self.power_set(target).await {
            Ok(()) => Some(memo),
            Err(e) => {
                notices.push(power_notice(&e.to_string()));
                match e {
                    // It may still land; "Back to normal" reads the mode before acting.
                    crate::power::PowerError::Timeout => Some(memo),
                    _ => in_force.cloned(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
