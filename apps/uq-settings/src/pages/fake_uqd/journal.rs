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
