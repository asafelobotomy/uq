//! Preview, picking a window and why a rule matched.

use super::*;

#[tokio::test]
async fn preview_lists_matching_windows_and_says_none() {
    let (_r, page) = rig().await;
    run(&page, Op::Refresh).await;
    let v = page.preview(&json!({"match": {"class": "^firefox$"}, "effects": {"float": true}}));
    assert_eq!(v["valid"], json!(true));
    assert_eq!(
        v["matches"],
        json!([{"title": "Mozilla Firefox", "class": "firefox", "verdict": "yes"}])
    );
    let v = page.preview(&json!({"match": {"class": "^nope$"}, "effects": {"float": true}}));
    assert_eq!(
        (v["valid"].clone(), v["matches"].clone()),
        (json!(true), json!([]))
    );
}

#[tokio::test]
async fn preview_with_bad_pattern_is_invalid_not_an_error() {
    // No bus at all: preview must not call D-Bus.
    let bus = private_bus();
    let page = RulesState::new(connect(&bus).await);
    for p in ["(?x)a", "[a&&b]"] {
        let v = page.preview(&json!({"match": {"class": p}, "effects": {"float": true}}));
        assert_eq!(v["valid"], json!(false), "{p}");
        let why = v["error"].as_str().unwrap();
        assert!(!why.is_empty() && !why.contains("regex"), "{why}");
        assert_eq!(v["matches"], json!([]));
    }
}

#[tokio::test]
async fn pick_for_match_prefills_exact_class() {
    let (r, page) = rig().await;
    r.windows
        .script_picked(&win("org.pulseaudio.pavucontrol", "Volume Control"));
    let out = run(
        &page,
        Op::Pick {
            purpose: "match".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let pick = &out.state["pick"];
    assert_eq!(pick["status"], "picked");
    assert_eq!(pick["purpose"], "match");
    assert_eq!(pick["window"]["class"], "org.pulseaudio.pavucontrol");
    assert_eq!(pick["class_pattern"], "^org\\.pulseaudio\\.pavucontrol$");
}

#[tokio::test]
async fn pick_cancel_and_no_shell_are_plain_statuses() {
    let (r, page) = rig().await;
    r.windows.script(json!({"status": "cancelled"}));
    r.windows.script(json!({"status": "no_shell"}));
    let a = run(
        &page,
        Op::Pick {
            purpose: "match".into(),
        },
    )
    .await;
    let b = run(
        &page,
        Op::Pick {
            purpose: "match".into(),
        },
    )
    .await;
    assert_eq!(
        (
            a.state["pick"]["status"].clone(),
            a.state["pick"]["seq"].clone()
        ),
        (json!("cancelled"), json!(1))
    );
    assert_eq!(
        (
            b.state["pick"]["status"].clone(),
            b.state["pick"]["seq"].clone()
        ),
        (json!("no_shell"), json!(2))
    );
    assert_eq!(b.state["pick"]["window"], Value::Null);
    assert_eq!(b.state["pick"]["class_pattern"], "");
    assert_eq!(b.error, None);
}

#[tokio::test]
async fn pick_failure_still_reports_a_status() {
    let (r, page) = rig().await;
    let before = run(&page, Op::Refresh).await.state["pick"]["seq"]
        .as_u64()
        .unwrap();
    r.windows.fail_picks();
    let out = run(
        &page,
        Op::Pick {
            purpose: "match".into(),
        },
    )
    .await;
    assert!(
        out.error
            .as_deref()
            .is_some_and(|e| e.contains("Already choosing")),
        "{:?}",
        out.error
    );
    assert_eq!(out.state["pick"]["status"], "error");
    assert_eq!(out.state["pick"]["seq"].as_u64().unwrap(), before + 1);
    assert_eq!(out.state["pick"]["window"], Value::Null);
}

#[tokio::test]
async fn why_lists_matching_rules_in_order_with_override_flag() {
    let st = state_with(json!({"window": [
        {"id": "r-0001", "name": "Pavu", "match": {"class": "^org\\.pulseaudio\\.pavucontrol$"}, "effects": {"float": true}},
        {"id": "r-0002", "name": "Kitty", "match": {"class": "^kitty$"}, "effects": {"float": true}},
        {"id": "r-0003", "name": "Any pulse", "match": {"class": "^org\\.pulseaudio\\..*"}, "effects": {"opacity": 0.9}},
    ]}));
    let (r, page) = rig_with(st).await;
    r.explain.set(
        "rules.window.r-0003",
        json!({"source": "user_lua", "detail": {"line": 4, "path": "/home/u/user.lua"}}),
    );
    r.windows
        .script_picked(&win("org.pulseaudio.pavucontrol", "Volume Control"));
    let out = run(
        &page,
        Op::Pick {
            purpose: "why".into(),
        },
    )
    .await;
    let rules = out.state["why"]["rules"].as_array().unwrap();
    let got: Vec<(&str, &str)> = rules
        .iter()
        .map(|x| (x["id"].as_str().unwrap(), x["source"].as_str().unwrap()))
        .collect();
    assert_eq!(got, [("r-0001", "uq"), ("r-0003", "user_lua")]);
    assert_eq!(rules[1]["line"], json!(4));
    assert_eq!(rules[1]["path"], "/home/u/user.lua");
    assert_eq!(rules[0]["line"], Value::Null);
}

#[tokio::test]
async fn unknown_pick_purpose_is_refused() {
    let (_r, page) = rig().await;
    let out = run(
        &page,
        Op::Pick {
            purpose: "x".into(),
        },
    )
    .await;
    assert!(out.error.is_some());
}
