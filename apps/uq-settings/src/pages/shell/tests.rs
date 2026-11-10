//! The Shell page against the fake uqd.

use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::run;
use uq_core::state::State;

async fn rig_in(app_dirs: Vec<PathBuf>, provider_dirs: Vec<PathBuf>) -> (Rig, ShellState) {
    let mut state = State::default();
    state
        .set_section(
            "shell",
            json!({"islands": {"work": false}, "clock_24h": false,
                   "providers": [{"id": "run", "enabled": false}, {"id": "org.gone.P"}],
                   "terminal": "kitty.desktop"}),
        )
        .unwrap();
    state.settings.pinned = vec!["a.desktop".into(), "b.desktop".into()];
    let r = fake_uqd::rig(state).await;
    let page = ShellState::with_dirs(r.client.clone(), app_dirs, provider_dirs);
    (r, page)
}

async fn rig() -> (Rig, ShellState) {
    rig_in(vec![], vec![]).await
}

fn shell(field: &str, value: Value) -> Op {
    Op::SetShell {
        field: field.into(),
        value,
    }
}

fn setting(key: &str, value: Value) -> Op {
    Op::SetSetting {
        key: key.into(),
        value,
    }
}

#[tokio::test]
async fn set_shell_keeps_other_fields() {
    let (r, page) = rig().await;
    let out = run(&page, shell("clock_24h", json!(true))).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["shell"]["clock_24h"], true);
    assert_eq!(out.state["shell"]["terminal"], "kitty.desktop");
    assert_eq!(out.state["shell"]["islands"]["work"], false);
    assert_eq!(out.state["shell"]["providers"][0]["id"], "run");

    let out = run(&page, shell("terminal", json!("foot.desktop"))).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["shell"]["terminal"], "foot.desktop");
    assert_eq!(out.state["shell"]["clock_24h"], true);
    assert_eq!(
        r.state.lock().unwrap().settings.shell.terminal.as_deref(),
        Some("foot.desktop")
    );

    let out = run(&page, shell("terminal", json!("../x.desktop"))).await;
    assert!(out.error.unwrap().contains("terminal"));
    assert_eq!(out.state["shell"]["terminal"], "foot.desktop");

    let out = run(&page, shell("terminal", Value::Null)).await;
    assert_eq!(out.error, None);
    assert_eq!(r.state.lock().unwrap().settings.shell.terminal, None);

    let out = run(&page, shell("nope", json!(1))).await;
    assert_eq!(out.error.as_deref(), Some("unknown field \"nope\""));
    let out = run(&page, shell("islands.nope", json!(true))).await;
    assert!(out.error.unwrap().contains("unknown field"));
}

#[tokio::test]
async fn islands_patch_one() {
    let (r, page) = rig().await;
    let out = run(&page, shell("islands.notif", json!(false))).await;
    assert_eq!(out.error, None);
    let islands = r.state.lock().unwrap().settings.shell.islands;
    assert!(!islands.notif && !islands.work, "work was already off");
    assert!(islands.system && islands.time && islands.status);
    assert_eq!(out.state["shell"]["islands"]["notif"], false);
    assert_eq!(out.state["shell"]["islands"]["system"], true);
}

#[tokio::test]
async fn providers_list_marks_missing() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("x.provider"),
        "[Launcher Provider]\nBusName=org.x.Search\nName=X search\n",
    )
    .unwrap();
    let (_r, page) = rig_in(vec![], vec![d.path().to_path_buf()]).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["providers"],
        json!([
            {"id": "calc", "name": "Calculator", "builtin": true},
            {"id": "run", "name": "Run", "builtin": true},
            {"id": "org.x.Search", "name": "X search", "builtin": false},
            {"id": "org.gone.P", "name": "org.gone.P", "builtin": false, "missing": true},
        ])
    );
    assert_eq!(out.state["shell"]["providers"][0]["enabled"], false);
}

#[tokio::test]
async fn pins_go_through_set_setting() {
    let (r, page) = rig().await;
    let out = run(&page, setting("pinned", json!(["b.desktop", "a.desktop"]))).await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["settings"]["pinned"],
        json!(["b.desktop", "a.desktop"])
    );
    assert_eq!(
        r.state.lock().unwrap().settings.pinned,
        ["b.desktop", "a.desktop"]
    );

    let out = run(&page, setting("pinned", json!(["../evil"]))).await;
    let message = out.error.expect("a bad id is refused");
    assert!(message.contains("pinned"), "{message}");
    assert_eq!(
        out.state["settings"]["pinned"],
        json!(["b.desktop", "a.desktop"])
    );

    let out = run(&page, setting("shell", json!(1))).await;
    assert_eq!(out.error.as_deref(), Some("unknown setting \"shell\""));
}

#[tokio::test]
async fn idle_order_refusal_snaps_back() {
    let (r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["settings"]["idle"]["lock"], 300);
    let out = run(&page, setting("idle.dim", json!(400))).await;
    let message = out.error.expect("dim above lock is refused");
    assert!(message.contains("strictly increasing"), "{message}");
    assert_eq!(out.state["settings"]["idle"]["dim"], 240);
    assert_eq!(r.state.lock().unwrap().settings.idle.dim, Some(240));

    // Off is allowed, and so is a free value in order.
    let out = run(&page, setting("idle.dim", Value::Null)).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["settings"]["idle"]["dim"], Value::Null);
    let out = run(&page, setting("idle.locked_screen_off", json!(5))).await;
    assert!(out.error.is_some());
    assert_eq!(out.state["settings"]["idle"]["locked_screen_off"], 30);
}

fn pin_move(id: &str, from: usize, to: usize) -> Op {
    Op::PinMove {
        id: id.into(),
        from,
        to,
    }
}

#[tokio::test]
async fn concurrent_pin_edits_both_land() {
    let (r, page) = rig().await;
    r.state.lock().unwrap().settings.pinned =
        vec!["a.desktop".into(), "b.desktop".into(), "c.desktop".into()];
    // Both edits were made on the same snapshot; the fake's reads are slow.
    let (x, y) = tokio::join!(
        run(
            &page,
            Op::PinRemove {
                id: "a.desktop".into()
            }
        ),
        run(&page, pin_move("c.desktop", 2, 1))
    );
    assert_eq!((x.error, y.error), (None, None));
    assert_eq!(
        r.state.lock().unwrap().settings.pinned,
        ["c.desktop", "b.desktop"]
    );
}

#[tokio::test]
async fn pin_ops_apply_to_the_fresh_list() {
    let (r, page) = rig().await;
    // Pinned elsewhere (Spotlight) since the page last read.
    r.state.lock().unwrap().settings.pinned =
        vec!["new.desktop".into(), "a.desktop".into(), "b.desktop".into()];
    let out = run(&page, pin_move("b.desktop", 1, 0)).await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["settings"]["pinned"],
        json!(["new.desktop", "b.desktop", "a.desktop"]),
        "b moves one up from where it is now"
    );
    let out = run(
        &page,
        Op::PinRemove {
            id: "gone.desktop".into(),
        },
    )
    .await;
    assert_eq!(out.error, None, "removing what's already gone is a no-op");
    let out = run(&page, pin_move("b.desktop", 0, 9)).await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["settings"]["pinned"],
        json!(["new.desktop", "a.desktop", "b.desktop"]),
        "moves are clamped to the list"
    );
}

#[tokio::test]
async fn concurrent_provider_edits_both_land() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("x.provider"),
        "[Launcher Provider]\nBusName=org.x.Search\nName=X search\n",
    )
    .unwrap();
    let (r, page) = rig_in(vec![], vec![d.path().to_path_buf()]).await;
    // Rows: run (off), org.gone.P, calc, org.x.Search.
    let (x, y) = tokio::join!(
        run(
            &page,
            Op::ProviderToggle {
                id: "org.x.Search".into(),
                on: false
            }
        ),
        run(
            &page,
            Op::ProviderMove {
                id: "calc".into(),
                from: 2,
                to: 0
            }
        )
    );
    assert_eq!((x.error, y.error), (None, None));
    let shell = r.state.lock().unwrap().settings.shell.clone();
    let rows: Vec<(&str, bool)> = shell
        .providers
        .iter()
        .map(|p| (p.id.as_str(), p.enabled))
        .collect();
    assert_eq!(
        rows,
        [
            ("calc", true),
            ("run", false),
            ("org.gone.P", true),
            ("org.x.Search", false)
        ]
    );
    assert!(!shell.islands.work, "the rest of the section is kept");
}

#[tokio::test]
async fn clear_launch_counts_calls_uqd() {
    let (r, page) = rig().await;
    r.state
        .lock()
        .unwrap()
        .launch_counts
        .insert("a.desktop".into(), 3);
    let out = run(&page, Op::ClearLaunchCounts).await;
    assert_eq!(out.error, None);
    assert!(r.state.lock().unwrap().launch_counts.is_empty());
}
