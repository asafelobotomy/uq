//! The fake uqd's displays and input devices: canned live outputs and devices, and uqd's
//! apply / confirm / revert rules without the timer ([`super::expire`] stands in for it).

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use uq_core::settings::{LiveOutput, Monitors, parse_live, stays_visible};

use super::{FakeError, invalid};

/// Two outputs, as `hyprctl monitors all -j` reports them (uq-core's fixture).
pub const FIXTURE: &str =
    include_str!("../../../../../crates/uq-core/tests/fixtures/monitors-all.json");

/// What the fake knows about displays.
#[derive(Clone)]
pub struct Displays {
    pub live_outputs: Arc<Mutex<Vec<LiveOutput>>>,
    /// `(token, candidate)` of the apply waiting for a confirm.
    pub pending: Arc<Mutex<Option<(String, Monitors)>>>,
    /// The id of an arrangement uqd skipped the last automatic switch for (`ListOutputs`'
    /// `arrangement.failed`).
    pub failed: Arc<Mutex<Option<String>>>,
    next_token: Arc<Mutex<u32>>,
}

impl Default for Displays {
    fn default() -> Self {
        Self {
            live_outputs: Arc::new(Mutex::new(parse_live(FIXTURE))),
            pending: Arc::default(),
            failed: Arc::default(),
            next_token: Arc::default(),
        }
    }
}

impl Displays {
    /// `{live, stored, roles, arrangement, pending}` as uqd's `ListOutputs`; a pending apply has
    /// 15 s left.
    pub fn list(&self, stored: &Monitors) -> String {
        let pending = self
            .pending
            .lock()
            .unwrap()
            .as_ref()
            .map(|(token, _)| json!({"token": token, "seconds_left": 15}));
        let (roles, arrangement) = self.roles_and_arrangement(stored);
        json!({
            "live": *self.live_outputs.lock().unwrap(),
            "stored": stored,
            "roles": roles,
            "arrangement": arrangement,
            "pending": pending,
        })
        .to_string()
    }

    /// uqd's checks, then a pending candidate under tokens `t1`, `t2`, …: the stored settings
    /// with the layout the JSON holds (R7), so a kept change keeps the arrangements and roles.
    pub fn apply(&self, json: &str, stored: &Monitors) -> Result<String, FakeError> {
        let value: Value =
            serde_json::from_str(json).map_err(|e| invalid(format!("value is not JSON: {e}")))?;
        let parsed = Monitors::from_section(&value).map_err(invalid)?;
        let candidate = stored.with_layout(parsed.outputs);
        if !stays_visible(&candidate, &self.live_outputs.lock().unwrap()) {
            return Err(invalid("at least one display must stay on"));
        }
        let mut n = self.next_token.lock().unwrap();
        *n += 1;
        let token = format!("t{n}");
        *self.pending.lock().unwrap() = Some((token.clone(), candidate));
        Ok(token)
    }

    /// The pending candidate for `token`, taken; uqd's message when nothing matches.
    pub fn take(&self, token: &str) -> Result<Monitors, FakeError> {
        let mut pending = self.pending.lock().unwrap();
        match pending.take() {
            Some((t, m)) if t == token => Ok(m),
            other => {
                *pending = other;
                Err(invalid("no display change is waiting for confirmation"))
            }
        }
    }
}

/// `ListInputDevices`: one keyboard, a touchpad and a mouse.
pub fn input_devices() -> String {
    json!({
        "keyboards": [{"name": "at-translated-set-2-keyboard"}],
        "pointers": [
            {"name": "elan-touchpad", "default_speed": 0.0},
            {"name": "logitech-mouse", "default_speed": 0.0},
        ],
        "touch": [],
    })
    .to_string()
}
