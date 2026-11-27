//! Saving, editing and the page states.

use super::*;

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
