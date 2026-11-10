//! Displays: apply a layout live, keep it only when confirmed, and put the old one back on its
//! own otherwise (spec §2.3 "Monitor apply and revert").
//!
//! The rollback point is the stored config: `state.toml` and `generated.lua` don't change until
//! `confirm_monitors`, so a revert is a Hyprland reload, and a crash or reboot during the window
//! comes back on the old layout. A marker file lets a restarted uqd notice the crash.

use crate::compositor::CompositorError;
use crate::service::{Desktop, DesktopError, Event};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use uq_core::history::Origin;
use uq_core::lua::{emit_monitor_line, emit_monitors};
use uq_core::settings::{Monitors, OutputConfig, parse_live, stays_visible};

/// How long an applied display change waits for a confirm before uqd reverts it.
pub const REVERT_AFTER: Duration = Duration::from_secs(15);

const NO_DESKTOP: &str = "displays can't be changed while the desktop isn't running";
const NOT_WAITING: &str = "no display change is waiting for confirmation";

/// The display change waiting for a confirm.
#[derive(Debug, Clone)]
pub struct Pending {
    pub token: String,
    pub candidate: Monitors,
    /// The stored monitors when the first apply of this window started.
    pub rollback: Monitors,
    /// Every output named by any candidate since the first apply.
    pub touched: BTreeSet<String>,
    pub deadline: Instant,
}

/// 32 hex characters from the kernel's random source.
fn new_token() -> Result<String, DesktopError> {
    use std::io::Read;
    let mut b = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .map_err(|e| DesktopError::Io(format!("making a token: {e}")))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// `m`'s lines plus a default line for each name in `others` that `m` doesn't set, as one
/// `eval` string. The defaults undo what an earlier live apply did to those outputs.
fn live_lines<'a>(m: &Monitors, others: impl IntoIterator<Item = &'a String>) -> String {
    let mut lines: Vec<String> = emit_monitors(m).lines().map(str::to_string).collect();
    let extra: BTreeSet<&String> = others
        .into_iter()
        .filter(|n| !m.outputs.contains_key(*n))
        .collect();
    lines.extend(
        extra
            .into_iter()
            .map(|n| emit_monitor_line(n, &OutputConfig::default())),
    );
    lines.join(" ")
}

impl Desktop {
    /// Shorten the revert window (tests).
    pub fn with_revert_after(mut self, d: Duration) -> Self {
        self.revert_after = d;
        self
    }

    fn marker_path(&self) -> std::path::PathBuf {
        self.paths
            .state_file
            .with_file_name("monitors-pending.json")
    }

    /// Delete a marker left by a uqd that stopped during a window; true if there was one.
    pub(crate) fn clear_stale_monitor_marker(&mut self) -> bool {
        let path = self.marker_path();
        if !path.exists() {
            return false;
        }
        tracing::warn!(
            "a display change was never confirmed before uqd stopped; keeping the old layout"
        );
        let _ = std::fs::remove_file(path);
        true
    }

    /// Apply `json` (a whole `monitors` section) live for [`REVERT_AFTER`]. Nothing is saved;
    /// returns the token `confirm_monitors` needs and the revert deadline. A second apply
    /// replaces the first and keeps the original rollback point.
    pub async fn apply_monitors(
        &mut self,
        json: &str,
        now: Instant,
    ) -> Result<(String, Instant), DesktopError> {
        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| DesktopError::InvalidSetting(format!("value is not JSON: {e}")))?;
        let mut candidate = Monitors::from_section(&value)
            .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
        let live = match self.comp.monitors().await {
            Ok(j) => parse_live(&j),
            Err(CompositorError::NotRunning) => {
                return Err(DesktopError::InvalidSetting(NO_DESKTOP.into()));
            }
            Err(e) => return Err(DesktopError::Io(e.to_string())),
        };
        if !stays_visible(&candidate, &live) {
            return Err(DesktopError::InvalidSetting(
                "at least one display must stay on".into(),
            ));
        }
        for (name, cfg) in candidate.outputs.iter_mut() {
            if let Some(o) = live.iter().find(|o| &o.name == name)
                && !o.description.is_empty()
            {
                cfg.description = Some(o.description.clone());
            }
        }
        let mut next = self.state.clone();
        next.settings.monitors = candidate.clone();
        let c = self.render_candidate(&next).await?;
        let _ = std::fs::remove_file(&c.path);

        let token = new_token()?;
        let rollback = match &self.pending {
            Some(p) => p.rollback.clone(),
            None => self.state.settings.monitors.clone(),
        };
        let mut touched = self
            .pending
            .as_ref()
            .map(|p| p.touched.clone())
            .unwrap_or_default();
        touched.extend(candidate.outputs.keys().cloned());
        let marker = serde_json::json!({"token": token, "touched": touched});
        uq_core::atomic::atomic_write(&self.marker_path(), &marker.to_string())
            .map_err(|e| DesktopError::Io(format!("writing the display marker: {e}")))?;

        let deadline = now + self.revert_after;
        let lines = live_lines(&candidate, touched.iter().chain(rollback.outputs.keys()));
        self.pending = Some(Pending {
            token: token.clone(),
            candidate,
            rollback,
            touched,
            deadline,
        });
        if let Err(e) = self.comp.eval(&lines).await {
            self.revert().await;
            return Err(match e {
                CompositorError::Rejected(msg) => DesktopError::ConfigInvalid(msg),
                CompositorError::NotRunning => DesktopError::InvalidSetting(NO_DESKTOP.into()),
                e => DesktopError::Io(e.to_string()),
            });
        }
        let _ = self.events.send(Event::MonitorsChanged);
        Ok((token, deadline))
    }

    fn take_matching(&self, token: &str) -> Result<&Pending, DesktopError> {
        self.pending
            .as_ref()
            .filter(|p| !token.is_empty() && p.token == token)
            .ok_or_else(|| DesktopError::InvalidSetting(NOT_WAITING.into()))
    }

    /// Keep the change `token` applied: one save, journalled with `origin`. If the save fails
    /// the change stays pending, so the timer still reverts it.
    pub async fn confirm_monitors(
        &mut self,
        token: &str,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        let candidate = self.take_matching(token)?.candidate.clone();
        let mut next = self.state.clone();
        next.settings.monitors = candidate;
        self.commit_section("monitors", next, origin).await?;
        self.pending = None;
        let _ = std::fs::remove_file(self.marker_path());
        let _ = self.events.send(Event::MonitorsChanged);
        Ok(())
    }

    /// Put the old layout back now ("Revert" in the dialog).
    pub async fn revert_monitors(&mut self, token: &str) -> Result<(), DesktopError> {
        self.take_matching(token)?;
        self.revert().await;
        Ok(())
    }

    /// The timer's end: revert if `token` is still the pending one and its time is up.
    pub async fn expire_monitors(&mut self, token: &str, now: Instant) -> bool {
        match &self.pending {
            Some(p) if p.token == token && now >= p.deadline => {
                self.revert().await;
                true
            }
            _ => false,
        }
    }

    /// Reload the stored config; if Hyprland won't, eval the rollback lines instead. Never saves.
    async fn revert(&mut self) {
        let Some(p) = self.pending.take() else {
            return;
        };
        let reloaded = matches!(
            tokio::time::timeout(Duration::from_secs(5), self.comp.reload()).await,
            Ok(Ok(()))
        );
        if !reloaded {
            let lines = live_lines(&p.rollback, p.touched.iter());
            if let Err(e) = self.comp.eval(&lines).await {
                tracing::warn!("putting the old displays back: {e}");
            }
        }
        let _ = std::fs::remove_file(self.marker_path());
        let _ = self.events.send(Event::MonitorsChanged);
    }

    /// After anything reloads Hyprland during a window, apply the candidate again, or the
    /// reload would quietly revert it early.
    pub(crate) async fn reapply_pending(&self) {
        if let Some(p) = &self.pending {
            let lines = live_lines(
                &p.candidate,
                p.touched.iter().chain(p.rollback.outputs.keys()),
            );
            if let Err(e) = self.comp.eval(&lines).await {
                tracing::warn!("re-applying the pending display change: {e}");
            }
        }
    }

    /// Undo or restore of a display change: save `next` and reload, outside the apply window.
    pub(crate) async fn write_monitors(
        &mut self,
        next: uq_core::state::State,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        if self.pending.is_some() {
            return Err(DesktopError::InvalidSetting(
                "keep or revert the display change first".into(),
            ));
        }
        self.commit_section("monitors", next, origin).await?;
        if let Err(e) = self.comp.reload().await {
            tracing::warn!("applying the restored displays: {e}");
        }
        Ok(())
    }

    /// `{live, stored, pending}` for the Displays page. `pending` is `null` or
    /// `{token, seconds_left}`; `live` is empty when Hyprland is down.
    pub async fn list_outputs(&self, now: Instant) -> String {
        let live = self
            .comp
            .monitors()
            .await
            .map(|j| parse_live(&j))
            .unwrap_or_default();
        let pending = self.pending.as_ref().map(|p| {
            serde_json::json!({
                "token": p.token,
                "seconds_left": p.deadline.saturating_duration_since(now).as_secs(),
            })
        });
        serde_json::json!({
            "live": live,
            "stored": self.state.settings.monitors,
            "pending": pending,
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests;
