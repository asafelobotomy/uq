//! Pins, launch counts and the bar.

use super::*;

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
    // Pinned elsewhere (look=see>do_) since the page last read.
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
        ["clock", "notif", "security", "tray", "status"]
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
        ["clock", "notif", "security", "tray", "status"]
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
        super::super::EDIT_BAR_ARGV,
        ["hyprctl", "dispatch", "hl.dsp.global(\"uq:edit-bar\")"]
    );
    let base = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/hypr/base.lua"),
    )
    .unwrap();
    assert!(base.contains("hl.dsp.global(\"uq:edit-bar\")"));
}
