use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::run;
use uq_core::state::State;

async fn rig() -> (Rig, DisplaysState) {
    let r = fake_uqd::rig(State::default()).await;
    let page = DisplaysState::new(r.client.clone());
    (r, page)
}

fn edit(output: &str, field: &str, value: Value) -> Op {
    Op::Edit {
        output: output.into(),
        field: field.into(),
        value,
    }
}

fn stored(r: &Rig) -> Monitors {
    r.state.lock().unwrap().settings.monitors.clone()
}

#[tokio::test]
async fn edit_changes_only_the_draft() {
    let (r, page) = rig().await;
    let out = run(&page, edit("DP-1", "scale", json!(1.5))).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["dirty"], true);
    assert_eq!(out.state["draft"]["outputs"]["DP-1"]["scale"], 1.5);
    assert!(stored(&r).outputs.is_empty());
    assert!(r.displays.pending.lock().unwrap().is_none());
    // The view follows the draft: DP-1's logical size shrinks with the scale.
    let dp = out.state["view"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "DP-1")
        .unwrap()
        .clone();
    assert_eq!(dp["config"]["scale"], 1.5);
    let out = run(&page, Op::Discard).await;
    assert_eq!(out.state["dirty"], false);
    assert!(out.state["draft"].is_null());
}

#[tokio::test]
async fn apply_then_keep_saves() {
    let (r, page) = rig().await;
    run(&page, edit("DP-1", "scale", json!(1.5))).await;
    let out = run(&page, Op::Apply).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["pending"]["token"], "t1");
    assert!(stored(&r).outputs.is_empty(), "apply saves nothing");
    let out = run(&page, Op::Keep).await;
    assert_eq!(out.error, None);
    assert!(out.state["pending"].is_null());
    assert_eq!(out.state["dirty"], false);
    assert_eq!(stored(&r).outputs["DP-1"].scale, Some(1.5));
    assert_eq!(r.history.origins(), ["settings"]);
}

#[tokio::test]
async fn apply_then_revert_keeps_draft() {
    let (r, page) = rig().await;
    run(&page, edit("DP-1", "transform", json!(1))).await;
    run(&page, Op::Apply).await;
    let out = run(&page, Op::Revert).await;
    assert_eq!(out.error, None);
    assert!(out.state["pending"].is_null());
    assert_eq!(out.state["dirty"], true);
    assert_eq!(out.state["draft"]["outputs"]["DP-1"]["transform"], 1);
    assert!(stored(&r).outputs.is_empty());
}

#[tokio::test]
async fn timeout_reports_restore() {
    let (r, page) = rig().await;
    run(&page, edit("DP-1", "scale", json!(2.0))).await;
    run(&page, Op::Apply).await;
    fake_uqd::expire(&r).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error.as_deref(), Some(RESTORED));
    assert_eq!(out.state["draft"]["outputs"]["DP-1"]["scale"], 2.0);
    // Said once.
    assert_eq!(run(&page, Op::Refresh).await.error, None);
    // Keeping after the window is refused with the same words.
    assert_eq!(run(&page, Op::Keep).await.error.as_deref(), Some(RESTORED));
}

#[tokio::test]
async fn refused_apply_shows_reason() {
    let (_r, page) = rig().await;
    run(&page, edit("eDP-1", "enabled", json!(false))).await;
    run(&page, edit("DP-1", "enabled", json!(false))).await;
    let out = run(&page, Op::Apply).await;
    assert!(
        out.error
            .as_deref()
            .unwrap()
            .contains("at least one display must stay on"),
        "{out:?}"
    );
    assert!(out.state["pending"].is_null());
    assert_eq!(out.state["dirty"], true);
}

#[tokio::test]
async fn bad_values_snap_back() {
    let (_r, page) = rig().await;
    let out = run(&page, edit("DP-1", "scale", json!(9))).await;
    assert!(out.error.unwrap().contains("scale"));
    assert_eq!(out.state["dirty"], false);
    let out = run(&page, edit("DP-1", "mirror_of", json!("DP-1"))).await;
    assert!(out.error.is_some(), "a display can't mirror itself");
}

#[tokio::test]
async fn round_trip_every_field() {
    let (r, page) = rig().await;
    let writes = [
        ("mode", json!("1920x1080@60")),
        ("position", json!({"x": 2560, "y": 0})),
        ("scale", json!(1.25)),
        ("transform", json!(3)),
        ("enabled", json!(true)),
        ("mirror_of", json!("eDP-1")),
        ("vrr", json!(2)),
    ];
    assert_eq!(
        writes.len(),
        OutputConfig::FIELDS.iter().filter(|f| editable(f)).count(),
        "a field was added without a round trip"
    );
    for (field, value) in &writes {
        let out = run(&page, edit("DP-1", field, value.clone())).await;
        assert_eq!(out.error, None, "{field}");
    }
    run(&page, Op::Apply).await;
    let out = run(&page, Op::Keep).await;
    assert_eq!(out.error, None);
    let fresh = DisplaysState::new(r.client.clone());
    let state = run(&fresh, Op::Refresh).await.state;
    for (field, value) in &writes {
        assert_eq!(
            &state["stored"]["outputs"]["DP-1"][field], value,
            "{field} reads back"
        );
    }
}

#[tokio::test]
async fn unknown_field_refused() {
    let (_r, page) = rig().await;
    for field in ["description", "nope"] {
        let out = run(&page, edit("DP-1", field, json!("x"))).await;
        assert_eq!(
            out.error.as_deref(),
            Some(format!("unknown field {field:?}").as_str())
        );
        assert_eq!(out.state["dirty"], false);
    }
}

#[tokio::test]
async fn move_snaps_and_normalises() {
    let (_r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    let view = out.state["view"].as_array().unwrap().clone();
    let dp = view.iter().find(|v| v["name"] == "DP-1").unwrap();
    let dp_w = dp["rect"]["w"].as_i64().unwrap();
    // Drop DP-1 far left of eDP-1: it joins eDP-1's left edge, then all shift to start at 0.
    let out = run(
        &page,
        Op::Move {
            output: "DP-1".into(),
            x: -3000,
            y: 10,
        },
    )
    .await;
    assert_eq!(out.error, None);
    let d = &out.state["draft"]["outputs"];
    let dp_x = d["DP-1"]["position"]["x"].as_i64().unwrap();
    let edp_x = d["eDP-1"]["position"]["x"].as_i64().unwrap();
    assert_eq!(dp_x, 0);
    assert_eq!(edp_x, dp_w, "{d}");
    let out = run(
        &page,
        Op::Move {
            output: "HDMI-A-9".into(),
            x: 0,
            y: 0,
        },
    )
    .await;
    assert!(out.error.is_some());
}

#[tokio::test]
async fn view_groups_modes_and_labels() {
    let (_r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    let edp = out.state["view"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "eDP-1")
        .unwrap()
        .clone();
    assert_eq!(edp["label"], "BOE 0x0BCA");
    assert_eq!(edp["enabled"], true);
    let modes = edp["modes"].as_array().unwrap();
    assert_eq!(modes[0]["w"], 2560);
    assert_eq!(modes[0]["rates"], json!([165.0, 60.0]));
    assert_eq!(modes[1]["w"], 1920);
    // 2560x1600 at 1.6 is 1600x1000.
    assert!(
        modes[0]["scale_choices"]
            .as_array()
            .unwrap()
            .contains(&json!(1.6))
    );
}
