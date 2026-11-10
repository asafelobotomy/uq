//! One fake uqd for every page model's tests: `org.uq.Desktop1` over a real
//! [`uq_core::state::State`] on a private bus, so refusals carry uq-core's own messages. Writes
//! are journalled like uqd does (one entry per changed domain, with the write's origin), and
//! `Undo`, the restore points and the profiles follow uqd's rules.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Map, Value, json};
use uq_core::client::DESKTOP_NAME;
use uq_core::history::{AutoReason, Entry, PointKind, RestorePoint, diff, revert_changed};
use uq_core::schedule::Schedule;
use uq_core::settings::provenance;
use uq_core::settings::renders_section;
use uq_core::state::State;
use uq_core::test_bus::{Bus, connect, private_bus};
use zbus::object_server::SignalEmitter;

mod devices;
mod profiles;
pub use devices::Displays;
pub use profiles::FakeProfiles;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.uq.Desktop1.Error")]
pub enum FakeError {
    #[zbus(error)]
    ZBus(zbus::Error),
    InvalidSetting(String),
    InvalidArgument(String),
    ConfigInvalid(String),
    Io(String),
    Conflict(String),
    UnknownTheme(String),
}

fn invalid(e: impl ToString) -> FakeError {
    FakeError::InvalidSetting(e.to_string())
}

pub struct FakeUqd {
    pub state: Arc<Mutex<State>>,
    /// When set, the next `SetSection` of a domain that renders config fails with
    /// `ConfigInvalid`, as uqd does when Hyprland rejects the generated config.
    pub verify_error: Arc<Mutex<Option<String>>>,
    pub explain: Explain,
    pub history: History,
    pub displays: Displays,
    pub profiles: FakeProfiles,
}

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
    fn write(
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
}

impl Explain {
    pub fn set(&self, key: &str, entry: Value) {
        self.overrides.lock().unwrap().insert(key.into(), entry);
    }

    pub fn fail(&self) {
        self.fails.store(true, Ordering::SeqCst);
    }
}

#[zbus::interface(name = "org.uq.Desktop1")]
impl FakeUqd {
    fn get_theme(&self) -> (String, String) {
        let s = self.state.lock().unwrap();
        (s.theme.clone(), s.variant.to_string())
    }

    // Writes take uqd's last `origin` argument, and journal what they change under it.
    fn set_theme(&self, name: &str, variant: &str, origin: &str) -> Result<(), FakeError> {
        let variant = variant.parse().map_err(invalid)?;
        self.write(origin, |s| {
            s.theme = name.to_string();
            s.variant = variant;
            Ok(())
        })?;
        Ok(())
    }

    fn list_themes(&self) -> Vec<String> {
        vec!["gruvbox".into(), "dracula".into()]
    }

    fn get_schedule(&self) -> String {
        serde_json::to_string(&self.state.lock().unwrap().schedule).unwrap()
    }

    fn set_schedule(&self, json: &str, origin: &str) -> Result<(), FakeError> {
        let schedule: Schedule = serde_json::from_str(json).map_err(invalid)?;
        schedule.validate().map_err(invalid)?;
        self.write(origin, |s| {
            s.schedule = schedule;
            Ok(())
        })?;
        Ok(())
    }

    async fn get_settings(&self) -> String {
        let read = serde_json::to_string(&self.state.lock().unwrap().settings).unwrap();
        // Slow, like `get_section`, so an unserialised read-modify-write of pins would lose one.
        tokio::time::sleep(Duration::from_millis(30)).await;
        read
    }

    fn set_setting(&self, key: &str, json: &str, origin: &str) -> Result<(), FakeError> {
        let value =
            serde_json::from_str(json).map_err(|e| invalid(format!("value is not JSON: {e}")))?;
        self.write(origin, |s| s.settings.apply(key, value).map_err(invalid))?;
        Ok(())
    }

    async fn get_section(&self, domain: &str) -> Result<String, FakeError> {
        let read = self
            .state
            .lock()
            .unwrap()
            .section(domain)
            .map_err(invalid)?;
        // A slow read, so an unserialised read-modify-write would lose an update.
        tokio::time::sleep(Duration::from_millis(30)).await;
        Ok(read.to_string())
    }

    fn set_section(&self, domain: &str, json: &str, origin: &str) -> Result<(), FakeError> {
        if domain == "monitors" {
            return Err(invalid("displays change through ApplyMonitors"));
        }
        let value =
            serde_json::from_str(json).map_err(|e| invalid(format!("value is not JSON: {e}")))?;
        self.write(origin, |next| {
            next.set_section(domain, value).map_err(invalid)?;
            if renders_section(domain)
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

    fn list_outputs(&self) -> String {
        let stored = self.state.lock().unwrap().settings.monitors.clone();
        self.displays.list(&stored)
    }

    async fn apply_monitors(
        &self,
        json: &str,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<String, FakeError> {
        let token = self.displays.apply(json)?;
        let _ = Self::monitors_changed(&e).await;
        Ok(token)
    }

    async fn confirm_monitors(
        &self,
        token: &str,
        origin: &str,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<(), FakeError> {
        let candidate = self.displays.take(token)?;
        self.write(origin, |s| {
            s.settings.monitors = candidate;
            Ok(())
        })?;
        let _ = Self::monitors_changed(&e).await;
        Ok(())
    }

    async fn revert_monitors(
        &self,
        token: &str,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<(), FakeError> {
        self.displays.take(token)?;
        let _ = Self::monitors_changed(&e).await;
        Ok(())
    }

    fn list_input_devices(&self) -> String {
        devices::input_devices()
    }

    #[zbus(signal)]
    async fn monitors_changed(e: &SignalEmitter<'_>) -> zbus::Result<()>;

    fn list_history(&self, limit: u32) -> String {
        let entries = self.history.entries.lock().unwrap();
        let newest_first: Vec<&Entry> = entries.iter().rev().take(limit as usize).collect();
        serde_json::to_string(&newest_first).unwrap()
    }

    /// uqd's rules: `Conflict` when a newer entry changed the same domain, unless `force`; only
    /// the keys the entry changed go back; the undo is a write with origin `restore`; returns the last id it journalled ("" if none).
    fn undo(&self, entry_id: &str, force: bool) -> Result<String, FakeError> {
        let entry = {
            let entries = self.history.entries.lock().unwrap();
            let Some(pos) = entries.iter().position(|e| e.id == entry_id) else {
                return Err(FakeError::InvalidArgument(format!(
                    "no history entry {entry_id:?}"
                )));
            };
            let entry = entries[pos].clone();
            if !force && entries[pos + 1..].iter().any(|e| e.domain == entry.domain) {
                return Err(FakeError::Conflict(format!(
                    "{} was changed again after this. Undo anyway?",
                    entry.domain
                )));
            }
            entry
        };
        let before = entry.before.clone();
        let ids = self.write("restore", |s| match entry.domain.as_str() {
            "theme" => {
                s.theme = serde_json::from_value(before["theme"].clone()).map_err(invalid)?;
                s.variant = serde_json::from_value(before["variant"].clone()).map_err(invalid)?;
                Ok(())
            }
            "schedule" => {
                s.schedule = serde_json::from_value(before).map_err(invalid)?;
                Ok(())
            }
            "settings" => {
                for (k, v) in before.as_object().into_iter().flatten() {
                    if entry.after.get(k) != Some(v) {
                        s.settings.apply(k, v.clone()).map_err(invalid)?;
                    }
                }
                Ok(())
            }
            domain => {
                let current = s.section(domain).map_err(invalid)?;
                s.set_section(domain, revert_changed(&current, &before, &entry.after))
                    .map_err(invalid)
            }
        })?;
        Ok(ids.last().cloned().unwrap_or_default())
    }

    fn list_restore_points(&self) -> String {
        let points = self.history.points.lock().unwrap();
        let newest_first: Vec<&RestorePoint> = points.iter().rev().map(|(p, _)| p).collect();
        serde_json::to_string(&newest_first).unwrap()
    }

    fn save_restore_point(&self, name: &str) -> String {
        let state = self.state.lock().unwrap().clone();
        self.history.add_point(name, PointKind::Manual, state)
    }

    fn delete_restore_point(&self, id: &str) {
        self.history
            .points
            .lock()
            .unwrap()
            .retain(|(p, _)| p.id != id);
    }

    fn restore(&self, id: &str) -> Result<(), FakeError> {
        let next = {
            let points = self.history.points.lock().unwrap();
            let Some((_, state)) = points.iter().find(|(p, _)| p.id == id) else {
                return Err(FakeError::InvalidArgument(format!("no restore point {id}")));
            };
            state.clone()
        };
        // As uqd: save the current settings first, so the restore can be reversed.
        let current = self.state.lock().unwrap().clone();
        self.history.add_point(
            "Before restoring a restore point",
            PointKind::Auto(AutoReason::BeforeRestore),
            current,
        );
        self.write("restore", |s| {
            let launch_counts = std::mem::take(&mut s.launch_counts);
            *s = next;
            s.launch_counts = launch_counts;
            Ok(())
        })?;
        Ok(())
    }

    fn explain(&self, domain: &str) -> Result<String, FakeError> {
        if self.explain.fails.load(Ordering::SeqCst) {
            return Err(FakeError::Io("can't read the live session".into()));
        }
        let section = self
            .state
            .lock()
            .unwrap()
            .section(domain)
            .map_err(invalid)?;
        let overrides = self.explain.overrides.lock().unwrap();
        let mut out = Map::new();
        for row in provenance::options_for(domain) {
            let stored = row.stored(&section);
            let source = if stored.is_null() {
                row.null_source.as_str()
            } else {
                "uq"
            };
            let mut entry =
                json!({"source": source, "live": null, "stored": stored, "detail": null});
            if let Some(Value::Object(o)) = overrides.get(&format!("{domain}.{}", row.json_key())) {
                for (k, v) in o {
                    entry[k] = v.clone();
                }
            }
            out.insert(row.json_key().to_string(), entry);
        }
        Ok(Value::Object(out).to_string())
    }

    fn reapply(&self, domain: &str) -> Result<(), FakeError> {
        self.state
            .lock()
            .unwrap()
            .section(domain)
            .map_err(invalid)?;
        self.explain
            .reapplied
            .lock()
            .unwrap()
            .push(domain.to_string());
        Ok(())
    }

    fn list_profiles(&self) -> String {
        self.profiles.list()
    }

    fn get_profile(&self, id: &str) -> Result<String, FakeError> {
        self.profiles.get(id)
    }

    async fn set_profile(
        &self,
        toml: &str,
        replace: bool,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<String, FakeError> {
        let id = self.profiles.set(toml, replace, &self.list_themes())?;
        let _ = Self::profile_changed(&e, &self.profiles.active_id()).await;
        Ok(id)
    }

    async fn delete_profile(
        &self,
        id: &str,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<(), FakeError> {
        self.profiles.delete(id)?;
        let _ = Self::profile_changed(&e, &self.profiles.active_id()).await;
        Ok(())
    }

    async fn apply_profile(
        &self,
        id: &str,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<String, FakeError> {
        let report = self.profiles.apply(self, id)?;
        let _ = Self::profile_changed(&e, id).await;
        Ok(report)
    }

    async fn profile_off(
        &self,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<String, FakeError> {
        let report = self.profiles.off(self)?;
        let _ = Self::profile_changed(&e, "").await;
        Ok(report)
    }

    #[zbus(signal)]
    async fn profile_changed(e: &SignalEmitter<'_>, id: &str) -> zbus::Result<()>;

    fn clear_launch_counts(&self) {
        self.state.lock().unwrap().launch_counts.clear();
    }

    fn get_launch_counts(&self) -> HashMap<String, u32> {
        let s = self.state.lock().unwrap();
        s.launch_counts
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }
}

pub struct Rig {
    /// Held so the bus lives as long as the rig.
    #[allow(dead_code, reason = "held, not read, by most tests")]
    pub bus: Bus,
    /// The fake's own connection; it owns `org.uq.Desktop1` (and can emit its signals).
    #[allow(dead_code, reason = "held, not read, by most tests")]
    pub uqd: zbus::Connection,
    /// A connection for the code under test.
    pub client: zbus::Connection,
    pub state: Arc<Mutex<State>>,
    pub verify_error: Arc<Mutex<Option<String>>>,
    /// What `Explain` answers, and what `Reapply` was asked.
    pub explain: Explain,
    /// The journal and the restore points.
    pub history: History,
    /// The live outputs and the pending display change.
    pub displays: Displays,
    /// The stored profiles and the active one.
    pub profiles: FakeProfiles,
}

/// uqd's revert timer firing: the pending display change is dropped and `MonitorsChanged` sent.
pub async fn expire(rig: &Rig) {
    rig.displays.pending.lock().unwrap().take();
    let iface = rig
        .uqd
        .object_server()
        .interface::<_, FakeUqd>("/org/uq/Desktop1")
        .await
        .unwrap();
    FakeUqd::monitors_changed(iface.signal_emitter())
        .await
        .unwrap();
}

/// A private bus with a [`FakeUqd`] serving `state`.
pub async fn rig(state: State) -> Rig {
    let bus = private_bus();
    let state = Arc::new(Mutex::new(state));
    let verify_error = Arc::new(Mutex::new(None));
    let explain = Explain::default();
    let history = History::default();
    let displays = Displays::default();
    let profiles = FakeProfiles::default();
    let uqd = connect(&bus).await;
    uqd.object_server()
        .at(
            "/org/uq/Desktop1",
            FakeUqd {
                state: state.clone(),
                verify_error: verify_error.clone(),
                explain: explain.clone(),
                history: history.clone(),
                displays: displays.clone(),
                profiles: profiles.clone(),
            },
        )
        .await
        .unwrap();
    uqd.request_name(DESKTOP_NAME).await.unwrap();
    let client = connect(&bus).await;
    Rig {
        bus,
        uqd,
        client,
        state,
        verify_error,
        explain,
        history,
        displays,
        profiles,
    }
}
