use super::extras::{NEEDS_NAME, NOT_THERE};
use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::run;
use uq_core::state::State;

async fn rig() -> (Rig, DisplaysState) {
    let r = fake_uqd::rig(State::default()).await;
    let page = DisplaysState::new(r.client.clone());
    (r, page)
}

fn stored(r: &Rig) -> Monitors {
    r.state.lock().unwrap().settings.monitors.clone()
}

fn names(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

fn ok(out: &crate::pages::runner::Outcome) {
    assert_eq!(out.error, None, "{out:?}");
}

fn bar(show: &str, outputs: &[&str]) -> Op {
    Op::SetBar {
        show: show.into(),
        outputs: outputs.iter().map(|s| s.to_string()).collect(),
    }
}

fn plan_op(strategy: &str, count: u8) -> Op {
    Op::SetPlan {
        strategy: strategy.into(),
        count,
        move_back: true,
    }
}

fn remember(name: &str) -> Op {
    Op::Remember { name: name.into() }
}

/// A scale change on DP-1, applied and kept: the layout the saved arrangement no longer has.
async fn keep_scale(page: &DisplaysState) {
    let edit = Op::Edit {
        output: "DP-1".into(),
        field: "scale".into(),
        value: json!(1.5),
    };
    ok(&run(page, edit).await);
    ok(&run(page, Op::Apply).await);
    ok(&run(page, Op::Keep).await);
}

#[tokio::test]
async fn set_main_by_output_name() {
    let (r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert!(out.state["main"].is_null());
    let out = run(
        &page,
        Op::SetMain {
            output: Some("DP-1".into()),
        },
    )
    .await;
    ok(&out);
    let id = stored(&r).primary.unwrap();
    assert_eq!(id.model, "DELL U2723QE");
    assert_eq!(out.state["main"]["output"], "DP-1");
    assert_eq!(out.state["main"]["label"], "Dell Inc. DELL U2723QE ABC1234");
    assert_eq!(out.state["main"]["connected"], true);
    assert_eq!(r.history.origins(), ["settings"]);
    // A name that isn't live is refused and nothing changes.
    let out = run(
        &page,
        Op::SetMain {
            output: Some("HDMI-A-9".into()),
        },
    )
    .await;
    assert_eq!(out.error.as_deref(), Some("That display isn't connected"));
    assert_eq!(stored(&r).primary, Some(id));
}

#[tokio::test]
async fn clear_main() {
    let (r, page) = rig().await;
    let set = Op::SetMain {
        output: Some("eDP-1".into()),
    };
    ok(&run(&page, set).await);
    let out = run(&page, Op::SetMain { output: None }).await;
    ok(&out);
    assert!(out.state["main"].is_null());
    assert_eq!(stored(&r).primary, None);
}

#[tokio::test]
async fn main_that_is_unplugged_is_reported() {
    let (r, page) = rig().await;
    let set = Op::SetMain {
        output: Some("DP-1".into()),
    };
    ok(&run(&page, set).await);
    r.displays.live_outputs.lock().unwrap().truncate(1);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["main"]["connected"], false);
    assert!(out.state["main"]["output"].is_null());
    assert_eq!(out.state["main"]["label"], "DELL U2723QE");
}

#[tokio::test]
async fn bar_choices() {
    let (r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["bar"], json!({"show": "all", "outputs": []}));
    let out = run(&page, bar("main", &[])).await;
    ok(&out);
    assert_eq!(out.state["bar"]["show"], "main");
    let out = run(&page, bar("chosen", &["eDP-1", "DP-1"])).await;
    ok(&out);
    assert_eq!(out.state["bar"]["show"], "chosen");
    assert_eq!(names(&out.state["bar"]["outputs"]), ["eDP-1", "DP-1"]);
    assert_eq!(stored(&r).bar.on.len(), 2);
    // Going back to all keeps the choice for next time.
    let out = run(&page, bar("all", &[])).await;
    ok(&out);
    assert_eq!(out.state["bar"]["show"], "all");
    assert_eq!(out.state["bar"]["outputs"].as_array().unwrap().len(), 2);
    // Refusals say why and change nothing.
    let out = run(&page, bar("chosen", &["HDMI-A-9"])).await;
    assert_eq!(out.error.as_deref(), Some("That display isn't connected"));
    let out = run(&page, bar("chosen", &[])).await;
    assert!(out.error.unwrap().contains("at least one display"));
    assert!(run(&page, bar("sideways", &[])).await.error.is_some());
    assert_eq!(stored(&r).bar.show, uq_core::settings::BarShow::All);
}

#[tokio::test]
async fn auto_switch_round_trips() {
    let (r, page) = rig().await;
    assert_eq!(run(&page, Op::Refresh).await.state["auto_switch"], true);
    let out = run(&page, Op::SetAutoSwitch(false)).await;
    ok(&out);
    assert_eq!(out.state["auto_switch"], false);
    assert!(!stored(&r).auto_profiles);
    let out = run(&page, Op::SetAutoSwitch(true)).await;
    assert_eq!(out.state["auto_switch"], true);
    assert!(stored(&r).auto_profiles);
}

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
async fn every_key_round_trips() {
    let (r, page) = rig().await;
    let set = Op::SetMain {
        output: Some("DP-1".into()),
    };
    ok(&run(&page, set).await);
    ok(&run(&page, Op::SetAutoSwitch(false)).await);
    ok(&run(&page, bar("chosen", &["eDP-1"])).await);
    ok(&run(&page, plan_op("manual", 8)).await);
    let slot = Op::SetManual {
        workspace: 2,
        output: Some("eDP-1".into()),
    };
    ok(&run(&page, slot).await);
    let set = Op::SetPlan {
        strategy: "manual".into(),
        count: 8,
        move_back: false,
    };
    ok(&run(&page, set).await);
    ok(&run(&page, remember("Desk")).await);
    let id = stored(&r).profiles[0].id.clone();
    let rename = Op::Rename {
        id,
        name: "Home".into(),
    };
    ok(&run(&page, rename).await);
    // A fresh page reads it all back.
    let fresh = DisplaysState::new(r.client.clone());
    let s = run(&fresh, Op::Refresh).await.state;
    assert_eq!(s["main"]["output"], "DP-1");
    assert_eq!(s["auto_switch"], false);
    assert_eq!(s["bar"], json!({"show": "chosen", "outputs": ["eDP-1"]}));
    assert_eq!(s["plan"]["strategy"], "manual");
    assert_eq!(s["plan"]["count"], 8);
    assert_eq!(s["plan"]["move_back"], false);
    assert_eq!(s["plan"]["manual"][0]["workspace"], 2);
    assert_eq!(s["arrangements"][0]["name"], "Home");
    // Every key of the section is covered.
    for key in Monitors::FIELDS {
        assert!(
            key == "outputs" || !s["stored"][key].is_null(),
            "{key} was not written"
        );
    }
}

#[tokio::test]
async fn layout_write_through_set_section_refused_plainly() {
    let (r, page) = rig().await;
    let proxy = DesktopProxy::new(&r.client).await.unwrap();
    let mut section = serde_json::to_value(stored(&r)).unwrap();
    section["outputs"]["DP-1"] = json!({"scale": 2.0});
    let e = proxy
        .set_section("monitors", &section.to_string(), "settings")
        .await
        .unwrap_err();
    assert!(
        crate::pages::runner::message(&e).contains("displays change through ApplyMonitors"),
        "{e}"
    );
    // While a display change is waiting, every write is refused with the same words.
    let edit = Op::Edit {
        output: "DP-1".into(),
        field: "scale".into(),
        value: json!(1.5),
    };
    run(&page, edit).await;
    ok(&run(&page, Op::Apply).await);
    let out = run(&page, Op::SetAutoSwitch(false)).await;
    assert_eq!(
        out.error.as_deref(),
        Some("keep or revert the display change first")
    );
    assert!(stored(&r).auto_profiles);
}

#[tokio::test]
async fn bad_values_read_plainly() {
    let (_r, page) = rig().await;
    let out = run(&page, plan_op("sequential", 0)).await;
    let text = out.error.unwrap();
    assert!(!text.starts_with("invalid value for "), "{text}");
    let out = run(&page, plan_op("backwards", 10)).await;
    assert!(out.error.is_some());
}

#[tokio::test]
async fn bar_keeps_screens_that_are_unplugged() {
    let (r, page) = rig().await;
    ok(&run(&page, bar("chosen", &["eDP-1", "DP-1"])).await);
    let dell = r.displays.live_outputs.lock().unwrap().remove(1);
    // Away from the desk: ticking the laptop's own screen again changes only it.
    ok(&run(&page, bar("chosen", &["eDP-1"])).await);
    assert_eq!(
        stored(&r).bar.on.len(),
        2,
        "the unplugged screen stays chosen"
    );
    r.displays.live_outputs.lock().unwrap().push(dell);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(names(&out.state["bar"]["outputs"]), ["eDP-1", "DP-1"]);
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
