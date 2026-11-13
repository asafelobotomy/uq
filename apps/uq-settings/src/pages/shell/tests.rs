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

    let out = run(&page, shell("edge_indicators", json!(false))).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["shell"]["edge_indicators"], false);

    let out = run(&page, shell("edge_style", json!("peek"))).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["shell"]["edge_style"], "peek");
    let out = run(&page, shell("edge_style", json!("slice"))).await;
    assert!(out.error.unwrap().contains("edge_style"));
    assert_eq!(out.state["shell"]["edge_style"], "peek");
    assert_eq!(out.state["shell"]["terminal"], Value::Null);
    assert!(!r.state.lock().unwrap().settings.shell.edge_indicators);

    let out = run(&page, shell("edge_titles", json!(true))).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["shell"]["edge_titles"], true);
    assert_eq!(out.state["shell"]["edge_indicators"], false);

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

/// Every setting the page writes keeps its value: a fresh page object on the same uqd reads back
/// what each `Op` wrote, so no control resets itself.
#[tokio::test]
async fn every_field_round_trips() {
    let (r, page) = rig().await;
    let mut writes = vec![
        (
            setting("pinned", json!(["c.desktop", "a.desktop"])),
            "/settings/pinned",
            json!(["c.desktop", "a.desktop"]),
        ),
        (
            setting("idle.dim", json!(200)),
            "/settings/idle/dim",
            json!(200),
        ),
        (
            setting("idle.lock", json!(310)),
            "/settings/idle/lock",
            json!(310),
        ),
        (
            setting("idle.screen_off", json!(400)),
            "/settings/idle/screen_off",
            json!(400),
        ),
        (
            setting("idle.suspend_battery", json!(1300)),
            "/settings/idle/suspend_battery",
            json!(1300),
        ),
        (
            setting("idle.locked_screen_off", json!(45)),
            "/settings/idle/locked_screen_off",
            json!(45),
        ),
        (
            setting("idle.media_blocks_lock", json!(true)),
            "/settings/idle/media_blocks_lock",
            json!(true),
        ),
        (
            setting("notif.backend", json!("mako")),
            "/settings/notif/backend",
            json!("mako"),
        ),
        (
            setting("notif.show_summary", json!(true)),
            "/settings/notif/show_summary",
            json!(true),
        ),
        (
            setting("privacy.public_ip_lookup", json!(false)),
            "/settings/privacy/public_ip_lookup",
            json!(false),
        ),
        (
            setting("privacy.record_launches", json!(false)),
            "/settings/privacy/record_launches",
            json!(false),
        ),
        (
            shell("clock_24h", json!(true)),
            "/shell/clock_24h",
            json!(true),
        ),
        (
            shell("edge_indicators", json!(false)),
            "/shell/edge_indicators",
            json!(false),
        ),
        (
            shell("edge_style", json!("peek")),
            "/shell/edge_style",
            json!("peek"),
        ),
        (
            shell("edge_titles", json!(true)),
            "/shell/edge_titles",
            json!(true),
        ),
        (
            shell("terminal", json!("foot.desktop")),
            "/shell/terminal",
            json!("foot.desktop"),
        ),
        (
            Op::ProviderToggle {
                id: "run".into(),
                on: true,
            },
            "/shell/providers/0",
            json!({"id": "run", "enabled": true}),
        ),
    ];
    // Every setting key, plus clock, the three edge-indicator settings, terminal and a provider
    // switch (the islands follow).
    assert_eq!(
        writes.len(),
        SETTING_KEYS.len() + 6,
        "a setting was added without a round trip"
    );
    for island in Islands::FIELDS {
        // The rig turns `work` off; every other island starts on.
        let on = island == "work";
        writes.push((
            shell(&format!("islands.{island}"), json!(on)),
            Box::leak(format!("/shell/islands/{island}").into_boxed_str()),
            json!(on),
        ));
    }

    let before = run(&page, Op::Refresh).await.state;
    for (op, pointer, want) in &writes {
        assert_ne!(
            before.pointer(pointer),
            Some(want),
            "{pointer} starts at the value"
        );
        let out = run(&page, op.clone()).await;
        assert_eq!(out.error, None, "{pointer}");
    }
    let fresh = ShellState::with_dirs(r.client.clone(), vec![], vec![]);
    let state = run(&fresh, Op::Refresh).await.state;
    for (_, pointer, want) in &writes {
        assert_eq!(state.pointer(pointer), Some(want), "{pointer} reads back");
    }
}

fn bar(v: Value) -> Op {
    Op::Bar {
        edit: serde_json::from_value(v).unwrap(),
    }
}

fn zone_ids(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn bar_edits_apply_to_uqds_layout() {
    let (r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(
        out.state["islands"][0]["id"], "system",
        "the island catalogue"
    );
    assert_eq!(
        zone_ids(&out.state["shell"]["bar"]["end"]),
        ["clock", "notif", "status"]
    );

    let out = run(
        &page,
        bar(
            json!({"op": "move", "id": "clock", "zone": "end", "index": 0,
                   "to_zone": "center", "to_index": 0}),
        ),
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(zone_ids(&out.state["shell"]["bar"]["center"]), ["clock"]);
    assert_eq!(
        out.state["shell"]["terminal"], "kitty.desktop",
        "the rest is kept"
    );

    let out = run(
        &page,
        bar(json!({"op": "add", "id": "stocks", "zone": "end"})),
    )
    .await;
    assert_eq!(out.error, None);
    let out = run(
        &page,
        bar(json!({"op": "options", "island": "stocks", "value": {"symbols": ["NVDA"]}})),
    )
    .await;
    assert_eq!(out.error, None);
    {
        let st = r.state.lock().unwrap();
        let b = st.settings.shell.bar.as_ref().unwrap();
        assert!(b.has("stocks"));
        assert_eq!(b.options.stocks.symbols, ["NVDA"]);
    }

    // An edit made on an older view is refused, and nothing changes.
    let out = run(
        &page,
        bar(json!({"op": "remove", "id": "clock", "zone": "end", "index": 0})),
    )
    .await;
    assert!(out.error.unwrap().contains("changed"));
    assert_eq!(zone_ids(&out.state["shell"]["bar"]["center"]), ["clock"]);

    let out = run(&page, bar(json!({"op": "reset"}))).await;
    assert_eq!(out.error, None);
    assert_eq!(
        zone_ids(&out.state["shell"]["bar"]["end"]),
        ["clock", "notif", "status"]
    );
    assert_eq!(
        out.state["shell"]["bar"]["options"]["stocks"]["symbols"][0],
        "NVDA"
    );
}

/// Settings' "Edit the bar" fires the shell's `uq:edit-bar` shortcut, as Super+Alt+B does.
#[test]
fn edit_bar_dispatches_the_shells_shortcut() {
    assert_eq!(
        super::EDIT_BAR_ARGV,
        ["hyprctl", "dispatch", "hl.dsp.global(\"uq:edit-bar\")"]
    );
    let base = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/hypr/base.lua"),
    )
    .unwrap();
    assert!(base.contains("hl.dsp.global(\"uq:edit-bar\")"));
}
