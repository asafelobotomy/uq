//! The main display, bar choices and round trips.

use super::*;

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
