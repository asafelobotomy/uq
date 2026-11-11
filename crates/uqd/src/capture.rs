//! Pausing uQ's shortcuts while the page listens for a new one (R11).
//!
//! Hyprland sees keys before any app, so pressing a combo that is already bound (Super + Space)
//! would open Spotlight instead of reaching the editor. `generated.lua` defines an empty submap,
//! `uq_capture`; in it no bind matches and keys go to the focused app. `pause` switches to it
//! and `resume` (or 15 s passing, or a second `pause`) switches back. While paused every
//! shortcut, the lock recovery bind included, is off.
//!
//! The broker lives outside the desktop lock (on the D-Bus interface, as the pick broker does),
//! so waiting never blocks a write, and the timer is its own task, so it still fires if the
//! page crashed. The timer is armed before the switch is sent, so a switch Hyprland applied but
//! never answered can't strand the shortcuts; and `Desktop::startup` always resets the submap, so
//! a restart of uqd mid-capture can't either.

use crate::compositor::{Compositor, CompositorError};
use crate::service::DesktopError;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;
use uq_core::keybinds::hypr::CAPTURE_SUBMAP;
use uq_core::lua::lua_string;

/// How long shortcuts stay paused if nobody resumes them.
pub const CAPTURE_TIMEOUT: Duration = Duration::from_secs(15);

const RESET: &str = "hl.dispatch(hl.dsp.submap(\"reset\"))";

struct Armed {
    token: String,
    timer: JoinHandle<()>,
}

type Slot = Arc<Mutex<Option<Armed>>>;

/// The one pause in force, if any.
pub struct CaptureBroker {
    slot: Slot,
    timeout: Duration,
}

impl Default for CaptureBroker {
    fn default() -> Self {
        Self::with_timeout(CAPTURE_TIMEOUT)
    }
}

fn down() -> DesktopError {
    DesktopError::Io("Hyprland isn't answering".into())
}

fn lock(slot: &Slot) -> std::sync::MutexGuard<'_, Option<Armed>> {
    slot.lock().unwrap_or_else(|e| e.into_inner())
}

impl CaptureBroker {
    pub fn with_timeout(timeout: Duration) -> Self {
        CaptureBroker {
            slot: Arc::default(),
            timeout,
        }
    }

    /// Switch to the empty submap and arm the timer. Returns the token `resume` needs. A second
    /// pause replaces the first: its token stops working and the timer starts over. With
    /// Hyprland down it's `Io("Hyprland isn't answering")`, and the page types keys instead.
    ///
    /// Takes the compositor by `Arc` because the timer task owns one.
    pub async fn pause(&self, comp: Arc<dyn Compositor>) -> Result<String, DesktopError> {
        // Arm first: Hyprland may apply the switch and still fail to answer in time, so from
        // here on shortcuts can never stay off without a timer that puts them back.
        let token = crate::monitors::new_token()?;
        // The deadline is fixed now, not when the task is first polled.
        let deadline = tokio::time::Instant::now() + self.timeout;
        let (slot, mine) = (self.slot.clone(), token.clone());
        let timer_comp = comp.clone();
        let timer = tokio::spawn(async move {
            tokio::time::sleep_until(deadline).await;
            let ours = {
                let mut g = lock(&slot);
                if g.as_ref().is_some_and(|a| a.token == mine) {
                    g.take();
                    true
                } else {
                    false
                }
            };
            if ours && let Err(e) = timer_comp.eval(RESET).await {
                tracing::warn!("resuming shortcuts after the timeout: {e}");
            }
        });
        // A replaced pause's timer must not reset the new one.
        if let Some(old) = lock(&self.slot).replace(Armed {
            token: token.clone(),
            timer,
        }) {
            old.timer.abort();
        }
        let sent = comp
            .eval(&format!(
                "hl.dispatch(hl.dsp.submap({}))",
                lua_string(CAPTURE_SUBMAP)
            ))
            .await;
        if let Err(e) = sent {
            tracing::debug!("pausing shortcuts: {e}");
            // Not running: nothing was switched, so nothing stays armed. Any other failure (a
            // reply that came too late or wasn't `ok`) may still have been applied: put the
            // shortcuts back now, and if that fails too the timer tries again.
            let undone = match e {
                CompositorError::NotRunning => true,
                _ => comp.eval(RESET).await.is_ok(),
            };
            if undone {
                let mut g = lock(&self.slot);
                if g.as_ref().is_some_and(|a| a.token == token)
                    && let Some(a) = g.take()
                {
                    a.timer.abort();
                }
            }
            return Err(down());
        }
        Ok(token)
    }

    /// Switch back. A token that isn't the current pause's (forged, stale, replaced or timed
    /// out) is `InvalidArgument`, and nothing is dispatched.
    pub async fn resume(&self, comp: &dyn Compositor, token: &str) -> Result<(), DesktopError> {
        {
            let mut g = lock(&self.slot);
            if !g.as_ref().is_some_and(|a| a.token == token) {
                return Err(DesktopError::InvalidArgument(
                    "Shortcuts aren't paused for that token.".into(),
                ));
            }
            if let Some(a) = g.take() {
                a.timer.abort();
            }
        }
        comp.eval(RESET).await.map_err(|e| {
            tracing::debug!("resuming shortcuts: {e}");
            down()
        })
    }
}

#[cfg(test)]
mod tests;
