//! The fake uqd's profiles: a store in memory and the active profile, with uqd's rules and the
//! same uq-core functions. There is no power service: a profile's power mode always gives the
//! notice uqd gives when the service is missing.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use serde_json::json;
use uq_core::profile::{
    self, ActiveProfile, MAX_PROFILES, Profile, ProfileError, emptied_elsewhere, kept_fix_notice,
    missing_binds, missing_binds_notice, missing_rules, missing_rules_notice, missing_startup,
    missing_startup_notice, power_notice, startup_notice, turn_off,
};

use super::{FakeError, FakeUqd, now};

/// Why the fake never changes the power mode.
pub const NO_POWER: &str = "power modes aren't available on this computer";

#[derive(Clone, Default)]
pub struct FakeProfiles {
    /// By id.
    pub store: Arc<Mutex<BTreeMap<String, Profile>>>,
    pub active: Arc<Mutex<Option<ActiveProfile>>>,
}

fn refused(e: ProfileError) -> FakeError {
    FakeError::InvalidSetting(e.to_string())
}

fn no_profile(id: &str) -> FakeError {
    FakeError::InvalidArgument(format!("no profile {id:?}"))
}

/// The theme a profile sets, when it isn't one uqd has.
fn missing_theme(p: &Profile, themes: &[String]) -> Option<String> {
    let theme = p.values.get("appearance")?.get("theme")?.as_str()?;
    (!themes.iter().any(|t| t == theme)).then(|| theme.to_string())
}

impl FakeProfiles {
    /// The active profile's id, or `""`.
    pub fn active_id(&self) -> String {
        let active = self.active.lock().unwrap();
        active.as_ref().map(|a| a.id.clone()).unwrap_or_default()
    }

    /// `ListProfiles`: by name (case-insensitive), then id.
    pub fn list(&self) -> String {
        let active = self.active_id();
        let mut all: Vec<Profile> = self.store.lock().unwrap().values().cloned().collect();
        all.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
        let list: Vec<_> = all
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "name": p.name,
                    "icon": p.icon,
                    "keys": p.keys().iter().filter(|k| k.domain != "power").count(),
                    "power": p.power(),
                    "active": p.id == active,
                })
            })
            .collect();
        serde_json::to_string(&list).unwrap()
    }

    fn load(&self, id: &str) -> Result<Profile, FakeError> {
        let store = self.store.lock().unwrap();
        store.get(id).cloned().ok_or_else(|| no_profile(id))
    }

    pub fn get(&self, id: &str) -> Result<String, FakeError> {
        self.load(id).map(|p| p.to_toml())
    }

    /// `SetProfile`: validated, a theme it sets must be one of `themes`, and an existing id is
    /// replaced only with `replace`.
    pub fn set(&self, toml: &str, replace: bool, themes: &[String]) -> Result<String, FakeError> {
        let p = Profile::parse(toml).map_err(refused)?;
        if let Some(theme) = missing_theme(&p, themes) {
            return Err(FakeError::UnknownTheme(theme));
        }
        let mut store = self.store.lock().unwrap();
        match store.get(&p.id) {
            Some(old) if !replace => {
                return Err(FakeError::Conflict(format!(
                    "A profile called \u{201c}{}\u{201d} already exists. Replace it?",
                    old.name
                )));
            }
            None if store.len() >= MAX_PROFILES => {
                return Err(FakeError::InvalidArgument(format!(
                    "You can keep up to {MAX_PROFILES} profiles."
                )));
            }
            _ => {}
        }
        let id = p.id.clone();
        store.insert(id.clone(), p);
        Ok(id)
    }

    pub fn delete(&self, id: &str) -> Result<(), FakeError> {
        if self.active_id() == id {
            return Err(FakeError::InvalidArgument(
                "Turn this profile off before deleting it.".into(),
            ));
        }
        let removed = self.store.lock().unwrap().remove(id);
        removed.map(|_| ()).ok_or_else(|| no_profile(id))
    }

    /// `ApplyProfile`: the profile on (switching from another in the same write), journalled as
    /// `profile:<id>` through [`FakeUqd::write`]. Returns the report's JSON.
    pub fn apply(&self, uqd: &FakeUqd, id: &str) -> Result<String, FakeError> {
        let p = self.load(id)?;
        if let Some(theme) = missing_theme(&p, &uqd.list_themes()) {
            return Err(FakeError::UnknownTheme(theme));
        }
        let old = self.active.lock().unwrap().clone();
        // The startup ids "found in the folders" are the scripted `ListAutostart` rows.
        let found: Vec<String> = uqd
            .autostart
            .rows
            .lock()
            .unwrap()
            .iter()
            .filter_map(|r| r["id"].as_str().map(str::to_string))
            .collect();
        let mut done = None;
        uqd.write(&format!("profile:{id}"), |s| {
            let (base, kept) = match &old {
                Some(a) => turn_off(s, a).map_err(refused)?,
                None => (s.clone(), Vec::new()),
            };
            // Like uqd: counted against the settings the profile goes over.
            let missing = missing_rules(&base, &p);
            let binds = missing_binds(&base, &p);
            let known: BTreeSet<String> = found
                .iter()
                .cloned()
                .chain(base.autostart.added.iter().map(|a| a.id().to_string()))
                .collect();
            let startup = missing_startup(&known, &p);
            let kept_fixes = emptied_elsewhere(&base, &p).len();
            let (next, memo) = profile::apply(&base, &p, &known, now()).map_err(refused)?;
            *s = next;
            done = Some((kept, memo, missing, binds, startup, kept_fixes));
            Ok(())
        })?;
        let (kept, memo, missing, binds, startup, kept_fixes) =
            done.expect("a write that succeeded ran its edit");
        let notices: Vec<String> = (missing > 0)
            .then(|| missing_rules_notice(missing))
            .into_iter()
            .chain((binds > 0).then(|| missing_binds_notice(binds)))
            .chain((startup > 0).then(|| missing_startup_notice(startup)))
            .chain((kept_fixes > 0).then(|| kept_fix_notice(kept_fixes)))
            .chain(startup_notice(&p))
            .chain(p.power().map(|_| power_notice(NO_POWER)))
            .collect();
        *self.active.lock().unwrap() = Some(memo);
        Ok(json!({"active": id, "kept": kept, "notices": notices}).to_string())
    }

    /// `ProfileOff`: "Back to normal", journalled as `profile:<id>`. Returns the report's JSON.
    pub fn off(&self, uqd: &FakeUqd) -> Result<String, FakeError> {
        let Some(a) = self.active.lock().unwrap().clone() else {
            return Ok(json!({"active": "", "kept": [], "notices": []}).to_string());
        };
        let mut kept = Vec::new();
        uqd.write(&format!("profile:{}", a.id), |s| {
            let (next, k) = turn_off(s, &a).map_err(refused)?;
            *s = next;
            kept = k;
            Ok(())
        })?;
        *self.active.lock().unwrap() = None;
        Ok(json!({"active": "", "kept": kept, "notices": []}).to_string())
    }
}
