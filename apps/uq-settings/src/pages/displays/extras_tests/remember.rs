//! Saved arrangements, the planner and manual slots.

use super::*;

#[tokio::test]
async fn plan_preview_before_save() {
    let (_r, page) = rig().await;
    // Off: nothing is planned, so nothing is previewed.
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["plan"]["strategy"], "off");
    assert!(out.state["plan"]["preview"].as_array().unwrap().is_empty());
    let out = run(&page, plan_op("sequential", 10)).await;
    ok(&out);
    let rows = out.state["plan"]["preview"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["label"], "BOE 0x0BCA");
    assert_eq!(rows[0]["workspaces"], json!([1, 2, 3, 4, 5]));
    assert_eq!(rows[1]["label"], "Dell Inc. DELL U2723QE ABC1234");
    assert_eq!(rows[1]["workspaces"], json!([6, 7, 8, 9, 10]));
    let out = run(&page, plan_op("interleave", 6)).await;
    let rows = out.state["plan"]["preview"].as_array().unwrap();
    assert_eq!(rows[0]["workspaces"], json!([1, 3, 5]));
    assert_eq!(rows[1]["workspaces"], json!([2, 4, 6]));
}

#[tokio::test]
async fn one_screen_previews_nothing() {
    let (r, page) = rig().await;
    r.displays.live_outputs.lock().unwrap().truncate(1);
    let out = run(&page, plan_op("sequential", 10)).await;
    ok(&out);
    assert!(out.state["plan"]["preview"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn manual_slot_set_and_cleared() {
    let (r, page) = rig().await;
    ok(&run(&page, plan_op("manual", 10)).await);
    let set = Op::SetManual {
        workspace: 3,
        output: Some("DP-1".into()),
    };
    let out = run(&page, set).await;
    ok(&out);
    let slots = out.state["plan"]["manual"].as_array().unwrap();
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0]["workspace"], 3);
    assert_eq!(slots[0]["output"], "DP-1");
    let rows = out.state["plan"]["preview"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["workspaces"], json!([3]));
    // Moving it to the other screen replaces the slot.
    let set = Op::SetManual {
        workspace: 3,
        output: Some("eDP-1".into()),
    };
    let out = run(&page, set).await;
    assert_eq!(out.state["plan"]["manual"][0]["output"], "eDP-1");
    assert_eq!(stored(&r).workspaces.manual.len(), 1);
    // Cleared.
    let clear = Op::SetManual {
        workspace: 3,
        output: None,
    };
    let out = run(&page, clear).await;
    ok(&out);
    assert!(out.state["plan"]["manual"].as_array().unwrap().is_empty());
    // Outside the planned workspaces is refused plainly.
    let bad = Op::SetManual {
        workspace: 11,
        output: Some("DP-1".into()),
    };
    let out = run(&page, bad).await;
    let text = out.error.unwrap();
    assert!(!text.starts_with("invalid value for "), "{text}");
}

#[tokio::test]
async fn remember_offered_then_saved() {
    let (r, page) = rig().await;
    keep_scale(&page).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["offer"], "remember");
    assert!(out.state["arrangements"].as_array().unwrap().is_empty());
    let out = run(&page, remember("Desk")).await;
    ok(&out);
    assert!(out.state["offer"].is_null());
    let list = out.state["arrangements"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["name"], "Desk");
    assert_eq!(list[0]["current"], true);
    assert_eq!(
        names(&list[0]["screens"]),
        ["BOE 0x0BCA", "Dell Inc. DELL U2723QE ABC1234"]
    );
    assert_eq!(r.history.origins().last().unwrap(), "settings");
    assert_eq!(stored(&r).profiles.len(), 1);
}

#[tokio::test]
async fn remember_needs_a_name_and_no_waiting_change() {
    let (_r, page) = rig().await;
    let out = run(&page, remember("   ")).await;
    assert_eq!(out.error.as_deref(), Some(NEEDS_NAME));
    let edit = Op::Edit {
        output: "DP-1".into(),
        field: "scale".into(),
        value: json!(1.5),
    };
    run(&page, edit).await;
    ok(&run(&page, Op::Apply).await);
    // While the change waits, nothing offers saving and saving is refused.
    let out = run(&page, Op::Refresh).await;
    assert!(out.state["offer"].is_null());
    let out = run(&page, remember("Desk")).await;
    assert_eq!(
        out.error.as_deref(),
        Some("keep or revert the display change first")
    );
}

#[tokio::test]
async fn update_offered_after_change() {
    let (r, page) = rig().await;
    ok(&run(&page, remember("Desk")).await);
    assert!(run(&page, Op::Refresh).await.state["offer"].is_null());
    keep_scale(&page).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["offer"], "update");
    assert_eq!(out.state["offer_name"], "Desk");
    // The kept change left the saved arrangements alone.
    assert_eq!(stored(&r).profiles.len(), 1);
    let out = run(&page, remember("Desk")).await;
    ok(&out);
    assert!(out.state["offer"].is_null());
    assert_eq!(out.state["arrangements"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn failed_switch_is_reported() {
    let (r, page) = rig().await;
    let out = run(&page, remember("Desk")).await;
    assert_eq!(out.state["arrangement_failed"], false);
    let id = stored(&r).profiles[0].id.clone();
    *r.displays.failed.lock().unwrap() = Some(id);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["arrangement_failed"], true);
}

#[tokio::test]
async fn rename_and_forget() {
    let (r, page) = rig().await;
    ok(&run(&page, remember("Desk")).await);
    let id = stored(&r).profiles[0].id.clone();
    let out = run(
        &page,
        Op::Rename {
            id: id.clone(),
            name: "  Home <b>office</b> ".into(),
        },
    )
    .await;
    ok(&out);
    // The name is kept as plain text, trimmed.
    assert_eq!(out.state["arrangements"][0]["name"], "Home <b>office</b>");
    let out = run(
        &page,
        Op::Rename {
            id: id.clone(),
            name: " ".into(),
        },
    )
    .await;
    assert_eq!(out.error.as_deref(), Some(NEEDS_NAME));
    let out = run(
        &page,
        Op::Rename {
            id: "m-0000".into(),
            name: "x".into(),
        },
    )
    .await;
    assert_eq!(out.error.as_deref(), Some(NOT_THERE));
    let out = run(&page, Op::Forget { id: id.clone() }).await;
    ok(&out);
    assert!(out.state["arrangements"].as_array().unwrap().is_empty());
    assert_eq!(out.state["offer"], "remember");
    let out = run(&page, Op::Forget { id }).await;
    assert_eq!(out.error.as_deref(), Some(NOT_THERE));
}

#[tokio::test]
async fn shrinking_below_a_manual_slot_says_what_to_do() {
    let (r, page) = rig().await;
    ok(&run(&page, plan_op("manual", 10)).await);
    let slot = Op::SetManual {
        workspace: 10,
        output: Some("DP-1".into()),
    };
    ok(&run(&page, slot).await);
    let out = run(&page, plan_op("manual", 9)).await;
    let text = out.error.unwrap();
    assert!(text.contains("Set it to Not chosen first"), "{text}");
    assert_eq!(stored(&r).workspaces.count, 10);
}
