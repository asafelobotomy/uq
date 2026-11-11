//! Welcome, the first-run flow in Settings (Phase 5 spec §5.1–5.2): whether it is still due, and
//! the bindle hand-off's defer file.
//!
//! uqd keeps `[welcome]` in `state.toml`. It is bookkeeping, not a setting: it is never written
//! to the history journal, a restore point never brings it back, and a profile never touches it
//! (see `uqd::service::Desktop::set_welcome_done`).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The Welcome steps this release shows. A later release that adds steps raises it, and users
/// who finished an older Welcome see only the steps added since (spec §5.1 "Version").
pub const CURRENT_WELCOME_VERSION: u32 = 1;

/// A defer file older than this is ignored, so a crashed bindle can't hide Welcome for good.
pub const DEFER_STALE_AFTER: Duration = Duration::from_secs(30 * 60);

/// `[welcome]` in `state.toml`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Welcome {
    /// The user finished (or closed) Welcome.
    pub done: bool,
    /// The Welcome version they saw last; 0 before the first run.
    pub version: u32,
}

impl Welcome {
    /// Welcome is due: never finished, or finished before this release added steps.
    pub fn pending(&self) -> bool {
        !self.done || self.version < CURRENT_WELCOME_VERSION
    }

    /// The version whose steps the user has already seen: 0 when never done (every step shows).
    pub fn seen_version(&self) -> u32 {
        if self.done { self.version } else { 0 }
    }

    /// The JSON `GetWelcome` answers: `{done, version, current, pending}`.
    pub fn status_json(&self) -> serde_json::Value {
        serde_json::json!({
            "done": self.done,
            "version": self.version,
            "current": CURRENT_WELCOME_VERSION,
            "pending": self.pending(),
        })
    }
}

/// `[welcome]` loads on its own: a wrong-typed table (or field) is the default, never a reason
/// to move `state.toml` aside.
pub(crate) fn lenient<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Welcome, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    let Some(obj) = v.as_object() else {
        tracing::warn!("state: [welcome] is not a table; ignoring it");
        return Ok(Welcome::default());
    };
    Ok(Welcome {
        done: obj
            .get("done")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        version: obj
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0),
    })
}

/// `$XDG_RUNTIME_DIR/uq/welcome.defer`: bindle creates it while its first-login wizard runs,
/// and removes it when it hands off with `uqctl welcome` (docs/welcome.md).
pub fn defer_path(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("uq").join("welcome.defer")
}

/// The defer file at `path` holds Welcome back at `now`: it exists and was modified less than
/// [`DEFER_STALE_AFTER`] ago. A missing or stale file doesn't, and neither does one dated in
/// the future (a clock jump must not hide Welcome for good either).
pub fn defer_active_at(path: &Path, now: SystemTime) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(mtime) = meta.modified() else {
        return false;
    };
    match now.duration_since(mtime) {
        Ok(age) => age < DEFER_STALE_AFTER,
        // A little in the future is ordinary clock noise between processes; more is stale.
        Err(e) => e.duration() < Duration::from_secs(60),
    }
}

/// [`defer_active_at`] for this session's runtime dir, now. No runtime dir: never deferred.
pub fn defer_active(runtime_dir: Option<&Path>) -> bool {
    runtime_dir.is_some_and(|d| defer_active_at(&defer_path(d), SystemTime::now()))
}

/// `~/.local/state/uq/session.log`, where `uq-session` writes a failed `uqctl setup`.
pub fn session_log_path(state_home: &Path) -> PathBuf {
    state_home.join("uq").join("session.log")
}

/// The line `uq-session` writes when `uqctl setup` failed (packaging/uq-session).
const SETUP_FAILED: &str = "uqctl setup exited";

/// `uq-session`'s log reports a failed setup: Welcome's first step shows a card naming the log.
/// Only the last 64 KiB is read; a missing or unreadable log reports nothing.
pub fn setup_problem(log: &Path) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(log) else {
        return false;
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(64 * 1024)));
    let mut buf = Vec::new();
    if f.take(64 * 1024).read_to_end(&mut buf).is_err() {
        return false;
    }
    String::from_utf8_lossy(&buf).contains(SETUP_FAILED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_until_done_at_the_current_version() {
        assert!(Welcome::default().pending());
        let done = Welcome {
            done: true,
            version: CURRENT_WELCOME_VERSION,
        };
        assert!(!done.pending());
        assert_eq!(done.seen_version(), CURRENT_WELCOME_VERSION);
        let older = Welcome {
            done: true,
            version: CURRENT_WELCOME_VERSION - 1,
        };
        assert!(older.pending(), "a newer release's steps are due once");
        let newer = Welcome {
            done: true,
            version: CURRENT_WELCOME_VERSION + 1,
        };
        assert!(!newer.pending(), "a downgrade doesn't show Welcome again");
        let reset = Welcome {
            done: false,
            version: CURRENT_WELCOME_VERSION,
        };
        assert!(reset.pending());
        assert_eq!(reset.seen_version(), 0, "a reset shows every step");
    }

    #[test]
    fn status_json_names_every_field() {
        let v = Welcome::default().status_json();
        assert_eq!(
            v,
            serde_json::json!({"done": false, "version": 0,
                "current": CURRENT_WELCOME_VERSION, "pending": true})
        );
    }

    fn touch_at(path: &Path, t: SystemTime) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let f = std::fs::File::create(path).unwrap();
        f.set_modified(t).unwrap();
    }

    #[test]
    fn defer_is_active_only_while_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let p = defer_path(dir.path());
        assert_eq!(p, dir.path().join("uq/welcome.defer"));
        let now = SystemTime::now();
        assert!(!defer_active_at(&p, now), "no file: not deferred");
        touch_at(&p, now - Duration::from_secs(60));
        assert!(defer_active_at(&p, now));
        touch_at(&p, now - Duration::from_secs(29 * 60 + 59));
        assert!(defer_active_at(&p, now));
        touch_at(&p, now - DEFER_STALE_AFTER);
        assert!(!defer_active_at(&p, now), "30 min old: stale");
        touch_at(&p, now - Duration::from_secs(5 * 3600));
        assert!(!defer_active_at(&p, now), "a crashed bindle's file");
        touch_at(&p, now + Duration::from_secs(5));
        assert!(defer_active_at(&p, now), "clock noise");
        touch_at(&p, now + Duration::from_secs(24 * 3600));
        assert!(!defer_active_at(&p, now), "far in the future: ignored");
        assert!(!defer_active(Some(dir.path())));
        touch_at(&p, SystemTime::now());
        assert!(
            defer_active(Some(dir.path())),
            "bindle's wizard just started"
        );
        assert!(!defer_active(None));
    }

    #[test]
    fn setup_problem_reads_the_session_log() {
        let dir = tempfile::tempdir().unwrap();
        let log = session_log_path(dir.path());
        assert!(!setup_problem(&log), "no log");
        std::fs::create_dir_all(log.parent().unwrap()).unwrap();
        std::fs::write(&log, "").unwrap();
        assert!(!setup_problem(&log));
        std::fs::write(
            &log,
            "2026-11-11 08:00:00 uqctl setup exited 1; starting the session anyway\n\
             2026-11-11 08:00:00   uqctl: can't write ~/.config/hypr\n",
        )
        .unwrap();
        assert!(setup_problem(&log));
    }

    #[test]
    fn lenient_load_defaults_bad_values() {
        #[derive(Deserialize)]
        struct W {
            #[serde(deserialize_with = "lenient")]
            welcome: Welcome,
        }
        let w: W = toml::from_str("welcome = 5").unwrap();
        assert_eq!(w.welcome, Welcome::default());
        let w: W = toml::from_str("[welcome]\ndone = \"yes\"\nversion = 2\n").unwrap();
        assert_eq!(
            w.welcome,
            Welcome {
                done: false,
                version: 2
            }
        );
        let w: W = toml::from_str("[welcome]\ndone = true\nversion = -1\n").unwrap();
        assert_eq!(
            w.welcome,
            Welcome {
                done: true,
                version: 0
            }
        );
    }
}
