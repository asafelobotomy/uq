//! The fake uqd's keyboard shortcuts: `ListBinds` and `Explain("keybinds")` through uq-core's own
//! pure functions (as uqd calls them, with no live Hyprland), and the key-capture pause with its
//! calls recorded and a switch that makes it fail as uqd does when Hyprland isn't answering.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value};
use uq_core::keybinds::{self, Keybinds};

use super::FakeError;

#[derive(Clone, Default)]
pub struct FakeBinds {
    /// What `user.lua` holds, for the `-- uq:bind` rows and the explanation.
    pub user_lua: Arc<Mutex<Option<String>>>,
    /// `pause` and `resume <token>`, in order.
    pub calls: Arc<Mutex<Vec<String>>>,
    /// When set, `PauseShortcuts` fails with `Io`.
    pub fail_pause: Arc<AtomicBool>,
    /// The token of the pause in force.
    pub paused: Arc<Mutex<Option<String>>>,
    counter: Arc<AtomicU32>,
}

impl FakeBinds {
    pub fn set_user_lua(&self, text: &str) {
        *self.user_lua.lock().unwrap() = Some(text.to_string());
    }

    pub fn fail_pauses(&self) {
        self.fail_pause.store(true, Ordering::SeqCst);
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub fn list(&self, kb: &Keybinds) -> String {
        let lua = self.user_lua.lock().unwrap().clone();
        keybinds::list::list(kb, lua.as_deref()).to_string()
    }

    /// `Explain("keybinds")` with Hyprland down (every row reports from state), plus the rejected
    /// message of the last refused write.
    pub fn explain(
        &self,
        kb: &Keybinds,
        rejected: Option<&str>,
        overrides: &Map<String, Value>,
    ) -> Value {
        let lua = self.user_lua.lock().unwrap().clone();
        let mut out = keybinds::explain(kb, None, lua.as_deref(), rejected, "user.lua");
        for (key, row) in out.as_object_mut().into_iter().flatten() {
            if let Some(Value::Object(o)) = overrides.get(&format!("keybinds.{key}")) {
                for (k, v) in o {
                    row[k] = v.clone();
                }
            }
        }
        out
    }

    pub fn pause(&self) -> Result<String, FakeError> {
        self.calls.lock().unwrap().push("pause".into());
        if self.fail_pause.load(Ordering::SeqCst) {
            return Err(FakeError::Io("Hyprland isn't answering".into()));
        }
        let token = format!("{:032x}", self.counter.fetch_add(1, Ordering::SeqCst) + 1);
        *self.paused.lock().unwrap() = Some(token.clone());
        Ok(token)
    }

    pub fn resume(&self, token: &str) -> Result<(), FakeError> {
        self.calls.lock().unwrap().push(format!("resume {token}"));
        let mut paused = self.paused.lock().unwrap();
        if paused.as_deref() != Some(token) {
            return Err(FakeError::InvalidArgument(
                "Shortcuts aren't paused for that token.".into(),
            ));
        }
        *paused = None;
        Ok(())
    }
}
