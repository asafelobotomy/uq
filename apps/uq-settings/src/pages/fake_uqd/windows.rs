//! The fake uqd's open windows and window picker: a window list, a queue of scripted pick
//! replies, and a switch that makes `PickWindow` fail as uqd does when one is already pending.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use uq_core::rules::WindowInfo;

use super::FakeError;

#[derive(Clone, Default)]
pub struct FakeWindows {
    pub windows: Arc<Mutex<Vec<WindowInfo>>>,
    /// Replies for `PickWindow`, oldest first: `{status, window?}`. With none left, the pick is
    /// `cancelled`.
    pub picks: Arc<Mutex<VecDeque<Value>>>,
    /// When set, `PickWindow` fails with `Conflict`.
    pub conflict: Arc<AtomicBool>,
}

impl FakeWindows {
    pub fn set(&self, windows: Vec<WindowInfo>) {
        *self.windows.lock().unwrap() = windows;
    }

    /// Queue the next pick's reply.
    pub fn script(&self, reply: Value) {
        self.picks.lock().unwrap().push_back(reply);
    }

    /// Queue a pick that clicked `window`.
    pub fn script_picked(&self, window: &WindowInfo) {
        self.script(json!({"status": "picked", "window": window}));
    }

    /// Make every `PickWindow` fail with `Conflict`.
    pub fn fail_picks(&self) {
        self.conflict.store(true, Ordering::SeqCst);
    }

    /// `ListWindows`.
    pub fn list(&self) -> String {
        serde_json::to_string(&*self.windows.lock().unwrap()).unwrap()
    }

    /// `PickWindow`.
    pub fn pick(&self) -> Result<String, FakeError> {
        if self.conflict.load(Ordering::SeqCst) {
            return Err(FakeError::Conflict("Already choosing a window.".into()));
        }
        let reply = self
            .picks
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| json!({"status": "cancelled"}));
        Ok(reply.to_string())
    }
}
