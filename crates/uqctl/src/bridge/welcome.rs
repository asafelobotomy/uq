//! Welcome's status for the shell (Phase 5 spec §5.1–5.2): `Hello` carries it, and the
//! `WelcomeStatus` call asks again (for a uqd that came up after `Hello`). The shell decides
//! whether to start Welcome (shell/welcome.js) from `{pending, deferred}`.

use super::uqd::Link;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

/// `{pending, deferred}`. `pending` is null when uqd can't say (not running yet, or no answer
/// within `wait`); `deferred` is whether bindle's defer file holds Welcome back right now.
/// `wait` None uses the bridge's usual call timeout.
pub async fn status(link: &Link, runtime_dir: Option<&Path>, wait: Option<Duration>) -> Value {
    let ask = link.call("GetWelcome", &[]);
    let answer = match wait {
        Some(w) => tokio::time::timeout(w, ask)
            .await
            .unwrap_or_else(|_| Err("timed out".into())),
        None => ask.await,
    };
    let pending = match answer {
        Ok(v) => v.get("pending").cloned().filter(Value::is_boolean),
        Err(_) => None,
    };
    json!({
        "pending": pending,
        "deferred": uq_core::welcome::defer_active(runtime_dir),
    })
}

/// `$XDG_RUNTIME_DIR` when it is set and absolute.
pub fn runtime_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
}
