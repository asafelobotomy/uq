//! Crash reports (docs/diagnostics.md): uqd watches Quickshell's crash folder and uQ's own, and
//! for each crash it hasn't seen it writes the report (`~/.local/state/uq/crashes/<id>/report.md`),
//! logs it under its CRASH code and, when the crash is recent, says so in a notification that
//! points to Settings › Problems. A crash seen once is never announced again (its report exists).

use std::sync::Arc;
use std::time::Duration;

use uq_core::diag::Registry;
use uq_core::diag::crash::{self, Crash, Dirs, Kind};
use uq_core::diag::report;
use uq_core::integration::watch::watch_dirs;
use uq_core::paths::Paths;

use crate::notify::Notifier;

/// A crash older than this when uqd first sees it (after an update, or with uqd down) gets its
/// report but no notification.
pub const ANNOUNCE_WITHIN: i64 = 6 * 3600;
/// The most crashes announced one by one in a pass; the rest share one notification.
pub const ANNOUNCE_MAX: usize = 2;
/// Quickshell writes a crash folder in a few steps; wait for it to settle.
const SETTLE: Duration = Duration::from_secs(2);

/// What the notification says for `c`.
pub fn notice(c: &Crash) -> (String, String) {
    let body = match &c.kind {
        Kind::Shell | Kind::Lock | Kind::Greeter => {
            "It restarted on its own. Settings › Problems has a report you can copy."
        }
        Kind::App(_) => "Settings › Problems has a report you can send to its developer.",
        Kind::Program(_) => {
            "It restarts on its own. Settings › Problems has a report you can copy."
        }
    };
    let summary = match &c.kind {
        Kind::Program(p) => format!("{p} stopped on an internal error"),
        _ => c.title(),
    };
    (summary, body.to_string())
}

fn log(c: &Crash, path: &str) {
    let (what, why) = (c.title(), c.reason.as_str());
    match c.kind {
        Kind::Shell => tracing::error!("[CRASH-001] {what}: {why}; report in {path}"),
        Kind::Lock | Kind::Greeter => {
            tracing::error!("[CRASH-002] {what}: {why}; report in {path}")
        }
        Kind::App(_) => tracing::error!("[CRASH-003] {what}: {why}; report in {path}"),
        Kind::Program(_) => tracing::error!("[CRASH-004] {what}: {why}; report in {path}"),
    }
}

/// The crashes in `all` that have no report yet.
pub fn unseen(all: Vec<Crash>) -> Vec<Crash> {
    all.into_iter()
        .filter(|c| !c.reported && !c.dismissed)
        .collect()
}

/// Which of `new` (newest first) to announce at `now`: the recent ones, at most
/// [`ANNOUNCE_MAX`] by name and a count of the rest.
pub fn to_announce(new: &[Crash], now: i64) -> (Vec<&Crash>, usize) {
    let recent: Vec<&Crash> = new
        .iter()
        .filter(|c| now - c.time <= ANNOUNCE_WITHIN)
        .collect();
    let more = recent.len().saturating_sub(ANNOUNCE_MAX);
    (recent.into_iter().take(ANNOUNCE_MAX).collect(), more)
}

/// Report every unseen crash and announce the recent ones.
pub async fn pass(dirs: &Dirs, reg: &Registry, paths: &Paths, notifier: &dyn Notifier) {
    let new = {
        let dirs = dirs.clone();
        tokio::task::spawn_blocking(move || unseen(crash::scan(&dirs)))
            .await
            .unwrap_or_default()
    };
    if new.is_empty() {
        return;
    }
    for c in &new {
        let (c2, reg2, paths2) = (c.clone(), reg.clone(), paths.clone());
        let saved =
            tokio::task::spawn_blocking(move || report::write_for(&c2, &reg2, &paths2)).await;
        match saved {
            Ok(Ok(p)) => log(c, &p.display().to_string()),
            Ok(Err(e)) => tracing::error!("[CRASH-005] {} ({}): {e}", c.title(), c.id),
            Err(e) => tracing::error!("[CRASH-005] {} ({}): {e}", c.title(), c.id),
        }
    }
    let now = chrono::Utc::now().timestamp();
    let (named, more) = to_announce(&new, now);
    if named.iter().any(|c| matches!(c.kind, Kind::Shell)) {
        // The notification server is the shell: give it a moment to come back.
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    for c in named {
        let (summary, body) = notice(c);
        notifier.notify(&summary, &body).await;
    }
    if more > 0 {
        notifier
            .notify(
                &format!("{more} more crashes"),
                "Settings › Problems lists them, each with a report.",
            )
            .await;
    }
}

/// Follow both crash folders until uqd stops.
pub async fn run(notifier: Arc<dyn Notifier>, paths: Paths) {
    let dirs = Dirs::from_env();
    let reg = Registry::installed();
    pass(&dirs, &reg, &paths, &*notifier).await;
    let mut rx = match watch_dirs(vec![dirs.quickshell.clone(), dirs.uq.clone()], SETTLE) {
        Ok(rx) => rx,
        Err(e) => {
            tracing::warn!("[WATCH-001] watching for crashes: {e}; crashes show at the next start");
            return;
        }
    };
    while rx.recv().await.is_some() {
        pass(&dirs, &reg, &paths, &*notifier).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notify::RecordingNotifier;

    fn write_qs_crash(dirs: &Dirs, run: &str, config: &str) {
        let d = dirs.quickshell.join(run);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("report.txt"),
            format!(
                "===== Instance Information =====\nSignal: Segmentation fault (11)\nRun ID: {run}\nConfig Path: {config}\n\n===== Stacktrace =====\n#1  0x1 in QFile::QFile(QString const&) at /usr/lib/libQt6Core.so.6\n"
            ),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn a_new_crash_is_reported_and_announced_once() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().to_string_lossy().into_owned();
        let dirs = Dirs::from_vars(&|k| (k == "HOME").then(|| home.clone()));
        let paths = Paths::under(root.path());
        write_qs_crash(&dirs, "run1", "/home/a/.local/share/notifd/shell.qml");
        let n = RecordingNotifier::default();
        pass(&dirs, Registry::builtin(), &paths, &n).await;
        {
            let sent = n.sent.lock().unwrap();
            assert_eq!(sent.len(), 1);
            assert_eq!(sent[0].0, "noti-fi's notification daemon crashed");
        }
        let c = crash::find(&dirs, "run1").unwrap();
        assert!(c.reported);
        let md = std::fs::read_to_string(c.report_path()).unwrap();
        assert!(
            md.starts_with("# noti-fi's notification daemon crashed"),
            "{md}"
        );
        // Seen: not again.
        pass(&dirs, Registry::builtin(), &paths, &n).await;
        assert_eq!(n.sent.lock().unwrap().len(), 1);
    }

    #[test]
    fn old_crashes_are_reported_quietly_and_many_are_counted() {
        let mk = |id: &str, time: i64| Crash {
            id: id.into(),
            time,
            kind: Kind::Shell,
            code: "CRASH-001",
            reason: String::new(),
            place: vec![],
            qs_dir: None,
            dir: "/x".into(),
            reported: false,
            dismissed: false,
        };
        let now = 100_000;
        let all = [
            mk("a", now - 10),
            mk("b", now - 20),
            mk("c", now - 30),
            mk("old", now - ANNOUNCE_WITHIN - 1),
        ];
        let (named, more) = to_announce(&all, now);
        assert_eq!(
            named.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(more, 1);
        let (s, b) = notice(&all[0]);
        assert_eq!(s, "uQ's shell crashed");
        assert!(b.contains("Settings › Problems"));
    }
}
