//! Workspace rules, editing and checks.

use super::*;

#[tokio::test]
async fn workspace_rule_shows_layout_from_layouts() {
    let mut st = state_with(json!({"workspace": [{"workspace": "3", "persistent": true}]}));
    st.set_section(
        "layouts",
        json!({"default": "dwindle", "workspaces": {"3": "master"}}),
    )
    .unwrap();
    let (_r, page) = rig_with(st).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["workspace"][0]["workspace"], "3");
    assert_eq!(out.state["workspace"][0]["layout"], "master");
    assert_eq!(
        out.state["workspace"][0]["fields_text"],
        json!(["Is kept when empty"])
    );
    // And a workspace rule saves by selector.
    let out = save(
        &page,
        "workspace",
        json!({"workspace": "4", "persistent": true}),
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["workspace"].as_array().unwrap().len(), 2);
    assert_eq!(out.state["workspace"][1]["layout"], "");
}

#[tokio::test]
async fn new_workspace_rule_never_replaces_one() {
    let (r, page) = rig_with(ws_state()).await;
    run(&page, Op::Refresh).await;
    for (sel, shown) in [
        ("3", "3"),
        ("web", "web"),
        ("name:web", "web"),
        (" 3 ", "3"),
    ] {
        let out = save(&page, "workspace", json!({"workspace": sel, "gaps_in": 4})).await;
        assert_eq!(
            out.error.as_deref(),
            Some(format!("Workspace {shown} already has a rule; edit that one.").as_str()),
            "{sel}"
        );
    }
    let st = r.state.lock().unwrap();
    assert_eq!(st.rules.workspace.len(), 2);
    assert_eq!(st.rules.workspace[0].monitor.as_deref(), Some("DP-1"));
    assert_eq!(st.rules.workspace[0].gaps_in, None);
}

#[tokio::test]
async fn editing_a_workspace_rule_replaces_it() {
    let (r, page) = rig_with(ws_state()).await;
    // Either spelling of the rule being edited finds it.
    for (key, gaps) in [("name:web", 4), ("web", 5)] {
        let out = save_as(
            &page,
            "workspace",
            json!({"workspace": "web", "gaps_in": gaps}),
            Some(key.into()),
        )
        .await;
        assert_eq!(out.error, None, "{key}");
        let st = r.state.lock().unwrap();
        assert_eq!(st.rules.workspace.len(), 2);
        assert_eq!(st.rules.workspace[1].workspace, "name:web");
        assert_eq!(st.rules.workspace[1].gaps_in, Some(gaps));
        assert_eq!(st.rules.workspace[1].persistent, None);
    }
}

#[tokio::test]
async fn editing_a_workspace_rule_can_change_its_workspace() {
    let (r, page) = rig_with(ws_state()).await;
    let out = save_as(
        &page,
        "workspace",
        json!({"workspace": "5", "persistent": true}),
        Some("3".into()),
    )
    .await;
    assert_eq!(out.error, None);
    {
        let st = r.state.lock().unwrap();
        let sels: Vec<&str> = st
            .rules
            .workspace
            .iter()
            .map(|w| w.workspace.as_str())
            .collect();
        assert_eq!(sels, ["5", "name:web"], "renamed in place, not added");
    }
    // But not onto another rule's workspace.
    let out = save_as(
        &page,
        "workspace",
        json!({"workspace": "web", "persistent": true}),
        Some("5".into()),
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("Workspace web already has a rule; edit that one.")
    );
    assert_eq!(r.state.lock().unwrap().rules.workspace[0].workspace, "5");
}

#[tokio::test]
async fn editing_a_rule_that_is_gone_says_so() {
    let (_r, page) = rig().await;
    let out = save_as(
        &page,
        "window",
        rule("A", "^a$", json!({"float": true})),
        Some("r-00ff".into()),
    )
    .await;
    assert_eq!(out.error.as_deref(), Some("that rule isn't there any more"));
}

#[tokio::test]
async fn check_finds_problems_before_sending() {
    let mut many: Vec<Value> = (0..50)
        .map(|i| json!({"id": format!("r-{i:04x}"), "name": "x", "match": {"namespace": "^n$"}, "effects": {"blur": true}}))
        .collect();
    many.truncate(49);
    let (_r, page) = rig_with(state_with(json!({
        "layer": many,
        "workspace": [{"workspace": "3", "persistent": true}],
    })))
    .await;
    run(&page, Op::Refresh).await;
    let layer =
        |ns: &str| json!({"name": "P", "match": {"namespace": ns}, "effects": {"blur": true}});
    let ok = page.check("layer", &layer("^waybar$"), None);
    assert_eq!(ok, json!({"valid": true, "problems": []}));
    for (kind, draft, replaces, says) in [
        ("layer", layer("[a&&b]"), None, "pattern"),
        ("layer", layer(""), None, ""),
        (
            "layer",
            json!({"name": "P", "match": {"namespace": "^w$"}, "effects": {}}),
            None,
            "at least one effect",
        ),
        (
            "workspace",
            json!({"workspace": "4"}),
            None,
            "at least one setting",
        ),
        (
            "workspace",
            json!({"workspace": "3", "gaps_in": 2}),
            None,
            "Workspace 3 already has a rule",
        ),
        (
            "window",
            json!({"name": "W", "match": {}, "effects": {"float": true}}),
            None,
            "at least one thing to match",
        ),
        (
            "window",
            rule("W", "^a$", json!({"float": true})),
            Some("r-ffff"),
            "isn't there any more",
        ),
    ] {
        let v = page.check(kind, &draft, replaces.map(String::from));
        assert_eq!(v["valid"], false, "{kind} {draft}");
        let problems = v["problems"].as_array().unwrap();
        assert_eq!(problems.len(), 1, "{v}");
        assert!(problems[0].as_str().unwrap().contains(says), "{v}");
    }
    // The 51st panel rule: the limit, before anything is sent.
    let mut fifty = page.check("layer", &layer("^a$"), None);
    assert_eq!(fifty["valid"], true);
    run(
        &page,
        Op::Save {
            kind: "layer".into(),
            rule: layer("^a$"),
            replaces: None,
        },
    )
    .await;
    fifty = page.check("layer", &layer("^b$"), None);
    assert!(
        fifty["problems"][0]
            .as_str()
            .unwrap()
            .contains("at most 50"),
        "{fifty}"
    );
}

#[tokio::test]
async fn rejected_write_rows_carry_the_message() {
    let (r, page) = rig_with(state_with(
        json!({"window": [rule("A", "^a$", json!({"float": true}))]}),
    ))
    .await;
    *r.verify_error.lock().unwrap() = Some("bad rule".into());
    let out = save(&page, "window", rule("B", "^b$", json!({"float": true}))).await;
    let row = &out.state["window"][0];
    assert_eq!(row["source"], "error");
    assert!(
        row["message"].as_str().unwrap().contains("bad rule"),
        "{row}"
    );
}
