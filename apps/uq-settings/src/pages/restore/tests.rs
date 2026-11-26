//! The Restore page's logic against the fake uqd and a fake `bindle` (a shell script that
//! answers as Bindle does and records each call).

use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::run;
use uq_core::history::{AutoReason, PointKind};
use uq_core::state::State;

/// A fake bindle in `dir`: `recovery --status` says snapper is set up, the snapshot reads need
/// the password unless `ask`, and every call is appended to `calls`.
fn fake_bindle(dir: &std::path::Path, locked: bool) -> PathBuf {
    let exe = dir.join("bindle");
    let calls = dir.join("calls");
    let locked = if locked { "exit 3" } else { ":" };
    let script = format!(
        r#"#!/bin/sh
echo "$*" >> {calls}
case "$*" in
  "recovery --status") echo '{{"rollback":null,"restore":null,"imageKept":false,"snapper":true}}' ;;
  "snapshot list --json --no-ask"|"snapshot settings --json --no-ask") {locked}; echo '[]' ;;
  "snapshot list --json") echo '[{{"number":4,"type":"single","date":"2026-11-26 10:00:00","description":"mine"}}]' ;;
  "snapshot settings --json") echo '{{"TIMELINE_CREATE":"yes","TIMELINE_LIMIT_DAILY":"10"}}' ;;
  "snapshot create"*) echo 5 ;;
  "snapshot set TIMELINE_LIMIT_DAILY x") echo "Error: \"x\" isn't a value for TIMELINE_LIMIT_DAILY" >&2; exit 1 ;;
  "rebase --yes") echo "Reinstalling 812 packages"; echo done ;;
esac
"#,
        calls = calls.display()
    );
    crate::pages::restore::tests::write_exe(&exe, &script);
    exe
}

pub fn write_exe(path: &std::path::Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn calls(dir: &std::path::Path) -> String {
    std::fs::read_to_string(dir.join("calls")).unwrap_or_default()
}

async fn rig(locked: bool) -> (Rig, tempfile::TempDir, RestoreState) {
    let r = fake_uqd::rig(State::default()).await;
    let dir = tempfile::tempdir().unwrap();
    let exe = fake_bindle(dir.path(), locked);
    let page = RestoreState::with(r.client.clone(), Some(exe), vec!["true".into()]);
    (r, dir, page)
}

#[tokio::test]
async fn opening_asks_bindle_once_and_never_for_the_password() {
    let (_r, dir, page) = rig(true).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["bindle"]["status"]["snapper"], true);
    assert_eq!(out.state["needsPassword"], true);
    assert!(out.state["snapshots"].is_null());
    run(&page, Op::Refresh).await;
    assert_eq!(calls(dir.path()).matches("recovery --status").count(), 1);
    let out = run(&page, Op::LoadSnapshots).await;
    assert_eq!(out.state["needsPassword"], false);
    assert_eq!(out.state["snapshots"][0]["number"], 4);
    assert_eq!(out.state["snapper"]["TIMELINE_LIMIT_DAILY"], "10");
}

#[tokio::test]
async fn snapshot_changes_go_to_bindle_and_its_refusal_shows() {
    let (_r, dir, page) = rig(false).await;
    run(&page, Op::Refresh).await;
    let out = run(
        &page,
        Op::CreateSnapshot {
            description: "  ".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["notice"]["text"], "Snapshot taken");
    run(&page, Op::DeleteSnapshot { number: 4 }).await;
    let bad = Op::SetSnapper {
        key: "TIMELINE_LIMIT_DAILY".into(),
        value: "x".into(),
    };
    let out = run(&page, bad).await;
    assert_eq!(
        out.error.as_deref(),
        Some("\"x\" isn't a value for TIMELINE_LIMIT_DAILY")
    );
    let c = calls(dir.path());
    assert!(c.contains("snapshot create Saved in Settings"), "{c}");
    assert!(c.contains("snapshot delete 4 --yes"), "{c}");
}

#[tokio::test]
async fn a_recovery_run_streams_and_unknown_actions_are_refused() {
    let (_r, _dir, page) = rig(false).await;
    let out = run(
        &page,
        Op::Recover {
            kind: "format".into(),
            number: 0,
        },
    )
    .await;
    assert!(out.error.is_some());
    run(
        &page,
        Op::Recover {
            kind: "rebase".into(),
            number: 0,
        },
    )
    .await;
    while page.run.running() {
        page.run.changed.notified().await;
    }
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["run"]["state"], "done");
    assert_eq!(out.state["run"]["log"][0], "Reinstalling 812 packages");
    assert_eq!(out.state["run"]["restart"], false);
}

#[tokio::test]
async fn without_bindle_the_page_still_resets_and_backs_up() {
    let r = fake_uqd::rig(State::default()).await;
    let page = RestoreState::with(r.client.clone(), None, vec![]);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["bindle"]["found"], false);
    assert!(run(&page, Op::LoadSnapshots).await.error.is_none());
    assert!(
        run(
            &page,
            Op::CreateSnapshot {
                description: "x".into()
            }
        )
        .await
        .error
        .is_some()
    );

    r.state.lock().unwrap().settings.shell.clock_24h = false;
    let dir = tempfile::tempdir().unwrap();
    let url = format!("file://{}/my%20backup.toml", dir.path().display());
    let out = run(&page, Op::SaveBackup { url: url.clone() }).await;
    assert_eq!(out.error, None);
    assert!(dir.path().join("my backup.toml").is_file());

    run(&page, Op::ResetAll).await;
    assert!(r.state.lock().unwrap().settings.shell.clock_24h);
    let out = run(&page, Op::RestoreBackup { url }).await;
    assert_eq!(out.error, None, "{out:?}");
    assert!(!r.state.lock().unwrap().settings.shell.clock_24h);
    let kinds: Vec<PointKind> = r
        .history
        .points
        .lock()
        .unwrap()
        .iter()
        .map(|(p, _)| p.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            PointKind::Auto(AutoReason::BeforeReset),
            PointKind::Auto(AutoReason::BeforeBackup)
        ]
    );

    std::fs::write(dir.path().join("other.toml"), "theme = \"x\"\n").unwrap();
    let other = format!("file://{}/other.toml", dir.path().display());
    let out = run(&page, Op::RestoreBackup { url: other }).await;
    assert!(out.error.unwrap().contains("isn't a uQ settings backup"));
    assert!(
        run(
            &page,
            Op::SaveBackup {
                url: "https://x/y".into()
            }
        )
        .await
        .error
        .is_some()
    );
}
