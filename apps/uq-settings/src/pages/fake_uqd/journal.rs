//! The fake's journal, restore points and explain overrides, and how a write is journalled.

use super::*;

/// The fake's journal and restore points.
#[derive(Clone, Default)]
pub struct History {
    /// Oldest first (`ListHistory` answers newest first).
    pub entries: Arc<Mutex<Vec<Entry>>>,
    /// Oldest first, each with the state it brings back.
    pub points: Arc<Mutex<Vec<(RestorePoint, State)>>>,
}

impl History {
    /// Add an entry as if some writer had journalled it (it changes no state).
    pub fn seed(&self, entry: Entry) -> String {
        let id = entry.id.clone();
        self.entries.lock().unwrap().push(entry);
        id
    }

    /// Add a restore point of `state`; returns its id.
    pub fn add_point(&self, name: &str, kind: PointKind, state: State) -> String {
        let mut points = self.points.lock().unwrap();
        let id = format!("{:010}-{:06x}", now(), points.len() + 1);
        let point = RestorePoint {
            id: id.clone(),
            name: name.chars().filter(|c| !c.is_control()).take(100).collect(),
            time: now(),
            kind,
        };
        points.push((point, state));
        id
    }

    /// The origins of the journal, oldest first.
    pub fn origins(&self) -> Vec<String> {
        let entries = self.entries.lock().unwrap();
        entries.iter().map(|e| e.origin.clone()).collect()
    }

    pub fn last_id(&self) -> String {
        let entries = self.entries.lock().unwrap();
        entries.last().map(|e| e.id.clone()).unwrap_or_default()
    }
}

/// Unix seconds now.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

impl FakeUqd {
    /// Change the state through `edit` and journal what changed under `origin`, as uqd's one
    /// save point does. Returns the ids journalled.
    pub(crate) fn write(
        &self,
        origin: &str,
        edit: impl FnOnce(&mut State) -> Result<(), FakeError>,
    ) -> Result<Vec<String>, FakeError> {
        let mut s = self.state.lock().unwrap();
        let mut next = s.clone();
        edit(&mut next)?;
        let origin = if origin.is_empty() { "unknown" } else { origin };
        let entries: Vec<Entry> = diff(&s, &next, &uq_core::history::label)
            .into_iter()
            .map(|c| Entry::new(now(), origin, c.domain, c.summary, c.before, c.after))
            .collect();
        *s = next;
        *self.explain.rules_rejected.lock().unwrap() = None;
        *self.explain.keybinds_rejected.lock().unwrap() = None;
        let ids = entries.iter().map(|e| e.id.clone()).collect();
        self.history.entries.lock().unwrap().extend(entries);
        Ok(ids)
    }

    /// Reset and a backup: a point of the current settings first, launch counts kept.
    pub(crate) fn replace_all(&self, next: State, reason: AutoReason) -> Result<(), FakeError> {
        let current = self.state.lock().unwrap().clone();
        self.history
            .add_point("Before", PointKind::Auto(reason), current);
        self.write("restore", |s| {
            let launch_counts = std::mem::take(&mut s.launch_counts);
            *s = next;
            s.launch_counts = launch_counts;
            Ok(())
        })?;
        Ok(())
    }

    /// `SetSection`, as uqd: refusals, the write and what `explain` reports after it.
    pub(crate) fn write_section(
        &self,
        domain: &str,
        json: &str,
        origin: &str,
    ) -> Result<(), FakeError> {
        if domain == "monitors" {
            return self.set_monitors_section(json, origin);
        }
        if let Some(text) = self.refuse_next.lock().unwrap().take() {
            return Err(FakeError::InvalidSetting(text));
        }
        let value =
            serde_json::from_str(json).map_err(|e| invalid(format!("value is not JSON: {e}")))?;
        let done = self.write(origin, |next| {
            next.set_section(domain, value).map_err(invalid)?;
            if renders_section(domain)
                && let Some(message) = self.verify_error.lock().unwrap().take()
            {
                return Err(FakeError::ConfigInvalid(format!(
                    "Hyprland rejected the new config: {message}"
                )));
            }
            Ok(())
        });
        if domain == "rules" {
            match &done {
                Ok(_) => {}
                Err(FakeError::ConfigInvalid(m)) => {
                    *self.explain.rules_rejected.lock().unwrap() = Some(m.clone());
                }
                Err(_) => {}
            }
        }
        if domain == "keybinds"
            && let Err(FakeError::ConfigInvalid(m)) = &done
        {
            *self.explain.keybinds_rejected.lock().unwrap() = Some(m.clone());
        }
        if matches!(domain, "env" | "compat" | "autostart") {
            let mut rejected = self.explain.startup_rejected.lock().unwrap();
            match &done {
                Ok(_) if rejected.as_ref().is_some_and(|(d, _)| d == domain) => *rejected = None,
                Err(FakeError::ConfigInvalid(m)) => *rejected = Some((domain.into(), m.clone())),
                _ => {}
            }
        }
        done?;
        Ok(())
    }
}

/// What the fake's `Explain` answers. Without overrides each key is `uq` when the section sets
/// it, else its [`provenance::OptionRow::null_source`], as uqd says when the live value matches.
#[derive(Clone, Default)]
pub struct Explain {
    /// `"<domain>.<key>"` → fields that replace the computed entry's (`source`, `detail`).
    pub overrides: Arc<Mutex<Map<String, Value>>>,
    /// When set, every `Explain` call fails.
    pub fails: Arc<AtomicBool>,
    /// The domains `Reapply` was called with, in order.
    pub reapplied: Arc<Mutex<Vec<String>>>,
    /// The message of the last `rules` write refused with `ConfigInvalid`; a good `rules`
    /// write clears it. `Explain("rules")` reports it as uqd does.
    pub rules_rejected: Arc<Mutex<Option<String>>>,
    /// Likewise for `keybinds`.
    pub keybinds_rejected: Arc<Mutex<Option<String>>>,
    /// `(domain, message)` of the last `env`, `compat` or `autostart` write refused with
    /// `ConfigInvalid`, as uqd's `last_rejected`; a good write of that domain clears it.
    pub startup_rejected: Arc<Mutex<Option<(String, String)>>>,
}

impl Explain {
    pub fn set(&self, key: &str, entry: Value) {
        self.overrides.lock().unwrap().insert(key.into(), entry);
    }

    pub fn fail(&self) {
        self.fails.store(true, Ordering::SeqCst);
    }
}
