//! Pick mode (spec §5, §10): the user clicks a window and uqd answers with that window's
//! metadata only. The broker lives outside the desktop lock, so waiting never blocks a write.

use crate::compositor::{Compositor, CompositorError};
use crate::service::DesktopError;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{Instant, timeout_at};
use uq_core::rules;

/// How long the shell has to say `shown` before the pick answers `no_shell`.
pub const PICK_ACK_TIMEOUT: Duration = Duration::from_secs(2);
/// How long the whole pick may take.
pub const PICK_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Shown,
    Click,
    Cancel,
}

impl std::str::FromStr for Outcome {
    type Err = DesktopError;
    fn from_str(s: &str) -> Result<Self, DesktopError> {
        match s {
            "shown" => Ok(Outcome::Shown),
            "click" => Ok(Outcome::Click),
            "cancel" => Ok(Outcome::Cancel),
            other => Err(DesktopError::InvalidArgument(format!(
                "unknown pick outcome {other:?} (expected shown, click or cancel)"
            ))),
        }
    }
}

type Slot = Arc<Mutex<Option<(String, mpsc::UnboundedSender<Outcome>)>>>;

/// The one pending pick, if any.
pub struct PickBroker {
    slot: Slot,
    ack: Duration,
    total: Duration,
}

impl Default for PickBroker {
    fn default() -> Self {
        Self::with_timeouts(PICK_ACK_TIMEOUT, PICK_TIMEOUT)
    }
}

/// Receives the outcomes for one token; dropping it frees the broker for the next pick.
pub struct Waiter {
    rx: mpsc::UnboundedReceiver<Outcome>,
    slot: Slot,
    token: String,
}

impl Drop for Waiter {
    fn drop(&mut self) {
        let mut g = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        if g.as_ref().is_some_and(|(t, _)| *t == self.token) {
            *g = None;
        }
    }
}

impl PickBroker {
    pub fn with_timeouts(ack: Duration, total: Duration) -> Self {
        PickBroker {
            slot: Arc::default(),
            ack,
            total,
        }
    }

    /// Start a pick: a fresh token and the waiter for its outcomes.
    pub fn begin(&self) -> Result<(String, Waiter), DesktopError> {
        let mut g = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_some() {
            return Err(DesktopError::Conflict("Already choosing a window.".into()));
        }
        let token = crate::monitors::new_token()?;
        let (tx, rx) = mpsc::unbounded_channel();
        *g = Some((token.clone(), tx));
        Ok((
            token.clone(),
            Waiter {
                rx,
                slot: self.slot.clone(),
                token,
            },
        ))
    }

    /// An outcome from the shell. An unknown token (forged or stale) does nothing.
    pub fn done(&self, token: &str, o: Outcome) {
        let g = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((t, tx)) = g.as_ref()
            && t == token
        {
            let _ = tx.send(o);
        }
    }
}

fn status(s: &str) -> String {
    serde_json::json!({ "status": s }).to_string()
}

/// Run one pick and return the reply JSON (`picked`, `none`, `cancelled`, `timeout`, `no_shell`
/// or `unavailable`). `Err` only for a second pick while one is pending.
pub async fn pick(
    broker: &PickBroker,
    comp: Arc<dyn Compositor>,
    emit: impl Fn(String),
) -> Result<String, DesktopError> {
    if matches!(comp.clients().await, Err(CompositorError::NotRunning)) {
        return Ok(status("unavailable"));
    }
    // Begin before emitting, so a fast `shown` always finds its token.
    let (token, mut waiter) = broker.begin()?;
    let start = Instant::now();
    emit(token);
    let mut shown = false;
    loop {
        let deadline = if shown {
            start + broker.total
        } else {
            start + broker.ack
        };
        match timeout_at(deadline, waiter.rx.recv()).await {
            Err(_) => return Ok(status(if shown { "timeout" } else { "no_shell" })),
            Ok(None) | Ok(Some(Outcome::Cancel)) => return Ok(status("cancelled")),
            Ok(Some(Outcome::Shown)) => shown = true,
            Ok(Some(Outcome::Click)) => break,
        }
    }
    drop(waiter);
    let (Ok(monitors), Ok(clients), Ok(cursor)) = (
        comp.monitors().await,
        comp.clients().await,
        comp.cursor_pos().await,
    ) else {
        return Ok(status("unavailable"));
    };
    Ok(match rules::window_at(&clients, &monitors, &cursor) {
        Some(w) => serde_json::json!({ "status": "picked", "window": w }).to_string(),
        None => status("none"),
    })
}

#[cfg(test)]
mod tests;
