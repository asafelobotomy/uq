//! Rules from apps.

use super::*;

#[tokio::test]
async fn rules_from_apps_are_listed_off_and_hidden_when_none() {
    let (r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(
        out.state["apps"]["rules"],
        json!([]),
        "{}",
        out.state["apps"]
    );
    r.windows.install_rules(&[
        ("romforge", ROMFORGE),
        (
            "broken",
            "[[window_rule]]\nid = \"other.x\"\nclass = [\"a\"]\nfloat = true\n",
        ),
    ]);
    let out = run(&page, Op::Refresh).await;
    let apps = &out.state["apps"];
    assert_eq!(apps["rules"].as_array().unwrap().len(), 1, "{apps}");
    let rule = &apps["rules"][0];
    assert_eq!(rule["id"], "romforge.emulators");
    assert_eq!(rule["integration"], "romforge");
    assert_eq!(rule["name"], "Emulators open full screen");
    assert_eq!(rule["enabled"], false);
    assert!(!rule["matches_text"].as_array().unwrap().is_empty());
    assert!(!rule["effects_text"].as_array().unwrap().is_empty());
    assert_eq!(apps["problems"][0]["integration"], "broken");
}

#[tokio::test]
async fn rules_from_apps_turn_on_and_off_with_history() {
    let (r, page) = rig().await;
    r.windows.install_rules(&[("romforge", ROMFORGE)]);
    let out = run(
        &page,
        Op::SetAppRule {
            id: "romforge.emulators".into(),
            on: true,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["apps"]["rules"][0]["enabled"], true);
    assert_eq!(
        r.state.lock().unwrap().integration_rules.enabled,
        ["romforge.emulators"]
    );
    // Turning on again changes nothing.
    run(
        &page,
        Op::SetAppRule {
            id: "romforge.emulators".into(),
            on: true,
        },
    )
    .await;
    assert_eq!(r.state.lock().unwrap().integration_rules.enabled.len(), 1);
    let h = r.history.entries.lock().unwrap().clone();
    assert_eq!(h[0].domain, "integration_rules");
    assert_eq!(
        h[0].summary,
        "Rule from an app turned on: romforge.emulators"
    );
    let out = run(
        &page,
        Op::SetAppRule {
            id: "romforge.emulators".into(),
            on: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["apps"]["rules"][0]["enabled"], false);
    assert!(r.state.lock().unwrap().integration_rules.enabled.is_empty());
}

#[tokio::test]
async fn a_refused_rule_from_an_app_stays_off_with_the_reason() {
    let (r, page) = rig().await;
    r.windows.install_rules(&[("romforge", ROMFORGE)]);
    *r.refuse_next.lock().unwrap() = Some("The rule romforge.emulators isn't installed.".into());
    let out = run(
        &page,
        Op::SetAppRule {
            id: "romforge.emulators".into(),
            on: true,
        },
    )
    .await;
    assert!(
        out.error
            .as_deref()
            .is_some_and(|e| e.contains("isn't installed")),
        "{:?}",
        out.error
    );
    assert_eq!(out.state["apps"]["rules"][0]["enabled"], false);
    // Hyprland refusing the rendered config: off, with Hyprland's words.
    *r.verify_error.lock().unwrap() = Some("bad rule".into());
    let out = run(
        &page,
        Op::SetAppRule {
            id: "romforge.emulators".into(),
            on: true,
        },
    )
    .await;
    assert!(
        out.error.as_deref().is_some_and(|e| e.contains("bad rule")),
        "{:?}",
        out.error
    );
    assert_eq!(out.state["apps"]["rules"][0]["enabled"], false);
    assert!(r.state.lock().unwrap().integration_rules.enabled.is_empty());
}

#[tokio::test]
async fn a_missing_rule_from_an_app_is_listed() {
    let mut st = State::default();
    st.set_section(
        "integration_rules",
        json!({"enabled": ["gone.rule", "romforge.emulators"]}),
    )
    .unwrap();
    let (r, page) = rig_with(st).await;
    r.windows.install_rules(&[("romforge", ROMFORGE)]);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["apps"]["missing"], json!(["gone.rule"]));
    assert_eq!(out.state["apps"]["rules"][0]["enabled"], true);
    // Turning another off keeps the missing id.
    run(
        &page,
        Op::SetAppRule {
            id: "romforge.emulators".into(),
            on: false,
        },
    )
    .await;
    assert_eq!(
        r.state.lock().unwrap().integration_rules.enabled,
        ["gone.rule"]
    );
}
