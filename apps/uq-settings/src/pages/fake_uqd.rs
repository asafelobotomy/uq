//! One fake uqd for every page model's tests: `org.uq.Desktop1` over a real
//! [`uq_core::state::State`] on a private bus, so refusals carry uq-core's own messages.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Map, Value, json};
use uq_core::client::DESKTOP_NAME;
use uq_core::schedule::Schedule;
use uq_core::settings::provenance;
use uq_core::settings::renders_section;
use uq_core::state::State;
use uq_core::test_bus::{Bus, connect, private_bus};

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.uq.Desktop1.Error")]
pub enum FakeError {
    #[zbus(error)]
    ZBus(zbus::Error),
    InvalidSetting(String),
    ConfigInvalid(String),
    Io(String),
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

    // Writes take uqd's last `origin` argument; the fake keeps no history, so ignores it.
    fn set_theme(&self, name: &str, variant: &str, _origin: &str) -> Result<(), FakeError> {
        let variant = variant.parse().map_err(invalid)?;
        let mut s = self.state.lock().unwrap();
        s.theme = name.to_string();
        s.variant = variant;
        Ok(())
    }

    fn list_themes(&self) -> Vec<String> {
        vec!["gruvbox".into(), "dracula".into()]
    }

    fn get_schedule(&self) -> String {
        serde_json::to_string(&self.state.lock().unwrap().schedule).unwrap()
    }

    fn set_schedule(&self, json: &str, _origin: &str) -> Result<(), FakeError> {
        let schedule: Schedule = serde_json::from_str(json).map_err(invalid)?;
        schedule.validate().map_err(invalid)?;
        self.state.lock().unwrap().schedule = schedule;
        Ok(())
    }

    async fn get_settings(&self) -> String {
        let read = serde_json::to_string(&self.state.lock().unwrap().settings).unwrap();
        // Slow, like `get_section`, so an unserialised read-modify-write of pins would lose one.
        tokio::time::sleep(Duration::from_millis(30)).await;
        read
    }

    fn set_setting(&self, key: &str, json: &str, _origin: &str) -> Result<(), FakeError> {
        let value =
            serde_json::from_str(json).map_err(|e| invalid(format!("value is not JSON: {e}")))?;
        self.state
            .lock()
            .unwrap()
            .settings
            .apply(key, value)
            .map_err(invalid)
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

    fn set_section(&self, domain: &str, json: &str, _origin: &str) -> Result<(), FakeError> {
        let value =
            serde_json::from_str(json).map_err(|e| invalid(format!("value is not JSON: {e}")))?;
        let mut s = self.state.lock().unwrap();
        let mut next = s.clone();
        next.set_section(domain, value).map_err(invalid)?;
        if renders_section(domain)
            && let Some(message) = self.verify_error.lock().unwrap().take()
        {
            return Err(FakeError::ConfigInvalid(format!(
                "Hyprland rejected the new config: {message}"
            )));
        }
        *s = next;
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
}

/// A private bus with a [`FakeUqd`] serving `state`.
pub async fn rig(state: State) -> Rig {
    let bus = private_bus();
    let state = Arc::new(Mutex::new(state));
    let verify_error = Arc::new(Mutex::new(None));
    let explain = Explain::default();
    let uqd = connect(&bus).await;
    uqd.object_server()
        .at(
            "/org/uq/Desktop1",
            FakeUqd {
                state: state.clone(),
                verify_error: verify_error.clone(),
                explain: explain.clone(),
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
    }
}
