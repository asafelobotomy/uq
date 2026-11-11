//! The Window rules page against the fake uqd.

use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::run;
use uq_core::rules::hypr;
use uq_core::state::State;
use uq_core::test_bus::{connect, private_bus};

fn win(class: &str, title: &str) -> WindowInfo {
    WindowInfo {
        class: class.into(),
        title: title.into(),
        initial_class: class.into(),
        initial_title: title.into(),
        workspace_id: 1,
        workspace_name: "1".into(),
        ..Default::default()
    }
}

fn windows() -> Vec<WindowInfo> {
    vec![
        win("firefox", "Mozilla Firefox"),
        win("kitty", "~"),
        win("org.pulseaudio.pavucontrol", "Volume Control"),
    ]
}

fn state_with(rules: Value) -> State {
    let mut st = State::default();
    st.set_section("rules", rules).unwrap();
    st
}

async fn rig_with(st: State) -> (Rig, RulesState) {
    let r = fake_uqd::rig(st).await;
    r.windows.set(windows());
    let page = RulesState::new(r.client.clone());
    (r, page)
}

async fn rig() -> (Rig, RulesState) {
    rig_with(State::default()).await
}

fn rule(name: &str, class: &str, effects: Value) -> Value {
    json!({"name": name, "match": {"class": class}, "effects": effects})
}

fn ids(r: &Rig) -> Vec<String> {
    r.state
        .lock()
        .unwrap()
        .rules
        .window
        .iter()
        .map(|x| x.id.clone())
        .collect()
}

fn names(state: &Value, list: &str) -> Vec<String> {
    state[list]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_string())
        .collect()
}

/// Save as the editor does: a window or panel rule with an id replaces that rule; anything else
/// (and every workspace rule) is an add.
async fn save(page: &RulesState, kind: &str, rule: Value) -> crate::pages::runner::Outcome {
    let replaces = match rule.get("id").and_then(Value::as_str) {
        Some(id) if kind != "workspace" && !id.is_empty() => Some(id.to_string()),
        _ => None,
    };
    save_as(page, kind, rule, replaces).await
}

async fn save_as(
    page: &RulesState,
    kind: &str,
    rule: Value,
    replaces: Option<String>,
) -> crate::pages::runner::Outcome {
    run(
        page,
        Op::Save {
            kind: kind.into(),
            rule,
            replaces,
        },
    )
    .await
}

#[tokio::test]
async fn save_new_rule_gets_id_and_journals() {
    let (r, page) = rig().await;
    let out = save(
        &page,
        "window",
        rule("Pavucontrol floats", "^pavu$", json!({"float": true})),
    )
    .await;
    assert_eq!(out.error, None);
    let id = ids(&r).remove(0);
    assert!(id.starts_with("r-"), "{id}");
    assert_eq!(out.state["window"][0]["id"], json!(id));
    assert_eq!(r.history.origins(), ["settings"]);
    let entries = r.history.entries.lock().unwrap();
    assert_eq!(entries[0].domain, "rules");
    assert!(
        entries[0].summary.contains("Pavucontrol floats"),
        "{}",
        entries[0].summary
    );
}

#[tokio::test]
async fn edit_move_toggle_delete_round_trip() {
    let (r, page) = rig().await;
    for (n, c) in [("A", "^a$"), ("B", "^b$"), ("C", "^c$")] {
        save(&page, "window", rule(n, c, json!({"float": true}))).await;
    }
    let all = ids(&r);
    // Edit B by id.
    let mut b = serde_json::to_value(&r.state.lock().unwrap().rules.window[1]).unwrap();
    b["name"] = json!("B2");
    assert_eq!(save(&page, "window", b).await.error, None);
    // Move C to the front, switch A off, delete B.
    let mv = Op::Move {
        kind: "window".into(),
        key: all[2].clone(),
        to: 0,
    };
    assert_eq!(run(&page, mv).await.error, None);
    let off = Op::SetEnabled {
        kind: "window".into(),
        key: all[0].clone(),
        on: false,
    };
    assert_eq!(run(&page, off).await.error, None);
    let del = Op::Delete {
        kind: "window".into(),
        key: all[1].clone(),
    };
    let out = run(&page, del).await;
    assert_eq!(out.error, None);
    assert_eq!(names(&out.state, "window"), ["C", "A"]);
    assert_eq!(out.state["window"][1]["enabled"], json!(false));
    // Reopening the page gives the same lists.
    let again = RulesState::new(r.client.clone());
    let out2 = run(&again, Op::Refresh).await;
    assert_eq!(out2.state["window"], out.state["window"]);
}

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
async fn rejected_write_shows_section_error() {
    let (r, page) = rig().await;
    *r.verify_error.lock().unwrap() = Some("bad rule".into());
    let out = save(&page, "window", rule("A", "^a$", json!({"float": true}))).await;
    assert!(
        out.error.as_deref().is_some_and(|e| e.contains("bad rule")),
        "{:?}",
        out.error
    );
    assert_eq!(out.state["section"]["source"], "error");
    assert!(
        out.state["section"]["message"]
            .as_str()
            .unwrap()
            .contains("bad rule")
    );
    // A good write clears it.
    let out = save(&page, "window", rule("A", "^a$", json!({"float": true}))).await;
    assert_eq!(out.state["section"], Value::Null);
}

#[tokio::test]
async fn static_effect_save_asks_to_reopen() {
    let (_r, page) = rig().await;
    let out = save(
        &page,
        "window",
        rule("Float", "^kitty$", json!({"float": true})),
    )
    .await;
    assert_eq!(out.state["reopen"]["names"], json!(["~"]));
    assert_eq!(out.state["reopen"]["seq"], json!(1));
    assert_eq!(out.state["window"][0]["static"], json!(true));
    assert_eq!(out.state["window"][0]["matches_now"], json!(1));
}

#[tokio::test]
async fn dynamic_only_save_does_not() {
    let (_r, page) = rig().await;
    let out = save(
        &page,
        "window",
        rule("Faint", "^kitty$", json!({"opacity": 0.9})),
    )
    .await;
    assert_eq!(out.state["reopen"]["names"], json!([]));
    assert_eq!(out.state["reopen"]["seq"], json!(0));
}

#[tokio::test]
async fn templates_build_and_save() {
    let (r, page) = rig().await;
    let listed = run(&page, Op::Refresh).await.state["templates"].clone();
    assert_eq!(listed.as_array().unwrap().len(), 5);
    for t in TEMPLATES {
        let op = Op::FromTemplate {
            template: t.id.into(),
            app: Some("org.pulseaudio.pavucontrol".into()),
            workspace: Some("3".into()),
            silent: true,
        };
        let out = run(&page, op).await;
        assert_eq!(out.error, None, "{}", t.id);
    }
    assert_eq!(ids(&r).len(), 5);
    let out = run(&page, Op::Refresh).await;
    assert!(
        out.state["notice"]["text"]
            .as_str()
            .unwrap()
            .starts_with("Added")
    );
    // A template that needs an app says so.
    let bad = Op::FromTemplate {
        template: "no_blur".into(),
        app: None,
        workspace: None,
        silent: false,
    };
    assert!(run(&page, bad).await.error.is_some());
}

#[tokio::test]
async fn fields_come_from_the_table() {
    let (_r, page) = rig().await;
    let f = run(&page, Op::Refresh).await.state["fields"].clone();
    let keys: Vec<&str> = f["match"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["key"].as_str().unwrap())
        .collect();
    let want: Vec<&str> = hypr::MATCH_KEYS.iter().map(|k| k.uq).collect();
    assert_eq!(f["match"].as_array().unwrap().len(), 11);
    assert_eq!(keys, want);
    for (item, e) in f["window_effects"]
        .as_array()
        .unwrap()
        .iter()
        .zip(hypr::WINDOW_EFFECTS.iter())
    {
        assert_eq!(item["lua"], e.field.lua);
        assert_eq!(item["static"], json!(e.when == hypr::When::Static));
    }
    assert_eq!(f["match"][0]["kind"], "pattern");
    assert_eq!(f["match"][5]["kind"], "bool");
    assert_eq!(f["layer_match"][0]["kind"], "pattern");
    assert_eq!(
        f["layer_effects"][3]["choices"],
        json!(["fade", "slide", "popin"])
    );
}

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
async fn limits_refusal_shows_uqd_message() {
    let many: Vec<Value> = (0..200)
        .map(|i| json!({"id": format!("r-{i:04x}"), "name": "x", "match": {"class": "^a$"}, "effects": {"float": true}}))
        .collect();
    let (_r, page) = rig_with(state_with(json!({"window": many}))).await;
    let out = save(
        &page,
        "window",
        rule("one more", "^b$", json!({"float": true})),
    )
    .await;
    assert!(
        out.error
            .as_deref()
            .is_some_and(|e| e.contains("at most 200")),
        "{:?}",
        out.error
    );
    assert_eq!(out.state["window"].as_array().unwrap().len(), 200);
    assert_eq!(out.state["limits"]["window"], 200);
}

#[tokio::test]
async fn uqd_down_shows_unavailable_not_silence() {
    let bus = private_bus();
    let page = RulesState::new(connect(&bus).await);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["error"], "uQ isn't running");
    let out = run(
        &page,
        Op::Pick {
            purpose: "match".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["pick"]["status"], "unavailable");
    assert_eq!(out.state["pick"]["seq"], 1);
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

#[tokio::test]
async fn editing_through_the_snapshot_keeps_every_other_key() {
    let st = state_with(json!({
        "window": [{"id": "r-0001", "name": "W", "enabled": false,
            "match": {"class": "^a$", "title": "t", "xwayland": true},
            "effects": {"float": true, "size": [300, 200], "opacity": 0.5, "workspace": "3", "silent": true}}],
        "layer": [{"id": "r-0002", "name": "L", "match": {"namespace": "bar"},
            "effects": {"blur": true, "ignore_alpha": 0.3, "animation": "fade"}}],
        "workspace": [{"workspace": "3", "persistent": true, "gaps_in": 4, "monitor": "DP-1"}],
    }));
    let (r, page) = rig_with(st).await;
    let state = run(&page, Op::Refresh).await.state;
    for (kind, change) in [
        ("window", "name"),
        ("layer", "name"),
        ("workspace", "gaps_in"),
    ] {
        let before = state[kind][0]["raw"].clone();
        let mut edited = before.clone();
        edited[change] = if change == "name" {
            json!("Renamed")
        } else {
            json!(9)
        };
        let key = key_of(kind, &before).to_string();
        let out = save_as(&page, kind, edited.clone(), Some(key)).await;
        assert_eq!(out.error, None, "{kind}");
        let after = out.state[kind][0]["raw"].clone();
        assert_eq!(after, edited, "{kind}");
        let mut a = after.clone();
        a[change] = before[change].clone();
        assert_eq!(a, before, "{kind}: only {change} changed");
    }
    assert_eq!(ids(&r), ["r-0001"]);
}

#[tokio::test]
async fn reopen_lists_at_most_three_and_counts_the_rest() {
    for (n, shown, more) in [(5usize, 3usize, 2usize), (3, 3, 0), (4, 3, 1)] {
        let (r, page) = rig().await;
        r.windows
            .set((0..n).map(|i| win("app", &format!("w{i}"))).collect());
        let out = save(&page, "window", rule("F", "^app$", json!({"float": true}))).await;
        assert_eq!(
            out.state["reopen"]["names"].as_array().unwrap().len(),
            shown,
            "{n}"
        );
        assert_eq!(out.state["reopen"]["more"], json!(more), "{n}");
    }
}

#[tokio::test]
async fn row_says_from_the_profile() {
    let st = state_with(
        json!({"window": [rule("Float A", "a", json!({"float": true}))],
        "layer": [{"name": "Blur", "match": {"namespace": "n"}, "effects": {"blur": true}}]}),
    );
    let (r, page) = rig_with(st).await;
    let (wid, lid) = {
        let st = r.state.lock().unwrap();
        (st.rules.window[0].id.clone(), st.rules.layer[0].id.clone())
    };
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["window"][0]["source"], "uq");
    assert_eq!(out.state["window"][0]["profile"], "");
    let toml = format!(
        "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[rules]\n\"{wid}\" = false\n\"{lid}\" = false\n"
    );
    let proxy = DesktopProxy::new(&r.client).await.unwrap();
    proxy.set_profile(&toml, false).await.unwrap();
    proxy.apply_profile("gaming").await.unwrap();
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    for list in ["window", "layer"] {
        let row = &out.state[list][0];
        assert_eq!(row["enabled"], false, "{list}");
        assert_eq!(row["source"], "profile", "{list}");
        assert_eq!(row["profile"], "Gaming", "{list}");
    }
}

fn ws_state() -> State {
    state_with(json!({"workspace": [
        {"workspace": "3", "monitor": "DP-1", "persistent": true},
        {"workspace": "name:web", "persistent": true},
    ]}))
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

const ROMFORGE: &str = r#"
[[window_rule]]
id = "romforge.emulators"
name = "Emulators open full screen"
class = ["^(retroarch|rpcs3)$"]
workspace = "name:game"
fullscreen = true
"#;

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
