//! Shell fields, islands, providers and idle.

use super::*;

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
            {"id": "web", "name": "Web addresses", "builtin": true},
            {"id": "calc", "name": "Calculator", "builtin": true},
            {"id": "run", "name": "Run", "builtin": true},
            {"id": "windows", "name": "Open windows", "builtin": true},
            {"id": "commands", "name": "Commands", "builtin": true},
            {"id": "settings", "name": "Settings", "builtin": true},
            {"id": "files", "name": "Files and folders", "builtin": true},
            {"id": "org.x.Search", "name": "X search", "builtin": false},
            {"id": "org.gone.P", "name": "org.gone.P", "builtin": false, "missing": true},
        ])
    );
    assert_eq!(out.state["shell"]["providers"][0]["enabled"], false);
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

#[tokio::test]
async fn concurrent_provider_edits_both_land() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("x.provider"),
        "[Launcher Provider]\nBusName=org.x.Search\nName=X search\n",
    )
    .unwrap();
    let (r, page) = rig_in(vec![], vec![d.path().to_path_buf()]).await;
    // Rows: run (off), org.gone.P, web, calc, windows, commands, settings, files, org.x.Search.
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
                from: 3,
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
            ("web", true),
            ("windows", true),
            ("commands", true),
            ("settings", true),
            ("files", true),
            ("org.x.Search", false)
        ]
    );
    assert!(!shell.islands.work, "the rest of the section is kept");
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
            setting("ui.print_action", json!("screen")),
            "/settings/ui/print_action",
            json!("screen"),
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
