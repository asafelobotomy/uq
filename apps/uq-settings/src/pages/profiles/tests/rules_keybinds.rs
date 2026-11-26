//! Rules and shortcuts in profiles.

use super::*;

#[tokio::test]
async fn rules_offered_in_add_settings() {
    let (_r, page) = rules_rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    assert_eq!(
        rule_keys_of(&out.state["available"]),
        vec![
            json!({"domain": "rules", "path": ["r-000a"],
                "label": "Window rule \u{201c}Float A\u{201d}", "text": "On"}),
            json!({"domain": "rules", "path": ["r-000b"],
                "label": "Window rule \u{201c}B\u{201d}", "text": "Off"}),
            json!({"domain": "rules", "path": ["r-00c0"],
                "label": "Panel rule \u{201c}Blur notifications\u{201d}", "text": "On"}),
        ]
    );
}

#[tokio::test]
async fn create_from_rules_page() {
    let (r, page) = rules_rig().await;
    let out = run(&page, Op::Refresh).await;
    let pages = out.state["pages"].as_array().unwrap();
    assert!(pages.contains(&json!({"id": "rules", "name": "Window rules"})));
    let out = run(
        &page,
        Op::CreateFromPages {
            name: "Rules".into(),
            icon: String::new(),
            pages: s(&["rules"]),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "rules").unwrap();
    assert_eq!(p.values.len(), 1, "{:?}", p.values);
    assert_eq!(
        p.values["rules"],
        json!({"r-000a": true, "r-000b": false, "r-00c0": true})
    );
}

#[tokio::test]
async fn create_from_history_takes_rule_toggles() {
    let (r, page) = rules_rig().await;
    let st = rules_state().section("rules").unwrap();
    let mut off = st.clone();
    off["window"][0]["enabled"] = json!(false);
    let mut fewer = st.clone();
    fewer["layer"] = json!([]);
    // A turned off then on again (taken, with its value now), and the panel rule added.
    r.history.seed(Entry::new(
        200,
        "settings",
        "rules",
        "",
        st.clone(),
        off.clone(),
    ));
    r.history
        .seed(Entry::new(210, "settings", "rules", "", off, st.clone()));
    r.history
        .seed(Entry::new(220, "settings", "rules", "", fewer, st));
    let out = run(
        &page,
        Op::CreateFromHistory {
            name: "Evening".into(),
            icon: String::new(),
            since: 100,
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "evening").unwrap();
    assert_eq!(p.values.len(), 1, "{:?}", p.values);
    assert_eq!(p.values["rules"], json!({"r-000a": true}));
    assert_eq!(
        out.state["notice"]["text"],
        "1 of those changes can't go in a profile"
    );
}

#[tokio::test]
async fn missing_rule_key_is_labelled_and_removable() {
    let (r, page) = rules_rig().await;
    seed(&r, QUIET);
    let out = run(&page, Op::Refresh).await;
    let labels: Vec<Value> = rule_keys_of(&out.state["profiles"][0]["keys"])
        .iter()
        .map(|k| k["label"].clone())
        .collect();
    assert_eq!(
        labels,
        [
            json!("Window rule \u{201c}Float A\u{201d}"),
            json!("A rule that isn't on this computer")
        ]
    );
    let out = run(&page, Op::Apply { id: "quiet".into() }).await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["report"]["notices"],
        json!(["1 rule in this profile isn't on this computer, so it was left out."])
    );
    assert!(!r.state.lock().unwrap().rules.window[0].enabled);
    let out = run(
        &page,
        Op::RemoveKey {
            id: "quiet".into(),
            domain: "rules".into(),
            path: s(&["r-ffff"]),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(
        stored(&r, "quiet").unwrap().values["rules"],
        json!({"r-000a": false})
    );
    let out = run(&page, Op::Off).await;
    assert_eq!(out.error, None);
    assert!(r.state.lock().unwrap().rules.window[0].enabled);
}

#[tokio::test]
async fn import_and_export_rules_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("quiet.toml");
    let (r, page) = rules_rig().await;
    let original = seed(&r, QUIET);
    let out = run(
        &page,
        Op::Export {
            id: "quiet".into(),
            path: file.clone(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(Profile::parse(&text).unwrap(), original);
    r.profiles.store.lock().unwrap().clear();
    let out = run(
        &page,
        Op::Import {
            path: file.clone(),
            replace: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(stored(&r, "quiet").unwrap(), original);
    for (body, message) in [
        ("\"x\" = true\n", "A rule: isn't a rule id uQ makes"),
        (
            "\"r-000a\" = \"off\"\n",
            "A rule: a profile can only turn a rule on or off",
        ),
    ] {
        let hostile = format!("format = 1\nid = \"bad\"\nname = \"Bad\"\n[rules]\n{body}");
        std::fs::write(&file, hostile).unwrap();
        let out = run(
            &page,
            Op::Import {
                path: file.clone(),
                replace: true,
            },
        )
        .await;
        assert_eq!(out.error.as_deref(), Some(message), "{body}");
    }
    assert!(stored(&r, "bad").is_none());
}

#[tokio::test]
async fn shortcuts_offered_in_add_settings() {
    let (_r, page) = binds_rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let keys = bind_keys_of(&out.state["available"]);
    assert!(
        keys.contains(&json!({"domain": "keybinds", "path": ["uq.close"],
        "label": "Close the window", "text": "On"})),
        "{keys:?}"
    );
    assert!(
        keys.contains(&json!({"domain": "keybinds", "path": ["k-00a1"],
        "label": "Shortcut \u{201c}Shot\u{201d}", "text": "Off"})),
        "{keys:?}"
    );
    let pages = out.state["pages"].as_array().unwrap();
    assert!(pages.contains(&json!({"id": "keybinds", "name": "Keyboard shortcuts"})));
}

#[tokio::test]
async fn create_from_keybinds_page() {
    let (r, page) = binds_rig().await;
    let out = run(
        &page,
        Op::CreateFromPages {
            name: "Keys".into(),
            icon: String::new(),
            pages: s(&["keybinds"]),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "keys").unwrap();
    assert_eq!(p.values.len(), 1, "{:?}", p.values);
    let t = &p.values["keybinds"];
    assert_eq!(t["k-00a1"], json!(false));
    assert_eq!(t["uq.close"], json!(true));
    // Applying it turns your own shortcut off again and leaves the rest as they were.
    let out = run(&page, Op::Apply { id: "keys".into() }).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["report"]["notices"], json!([]));
}

#[tokio::test]
async fn missing_shortcut_is_labelled_and_a_notice() {
    let (r, page) = binds_rig().await;
    seed(&r, &BINDS_QUIET.replace("k-00a1", "k-ffff"));
    let out = run(&page, Op::Apply { id: "quiet".into() }).await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["report"]["notices"],
        json!(["1 shortcut in this profile isn't on this computer, so it was left out."])
    );
}

#[tokio::test]
async fn import_and_export_keybinds_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("quiet.toml");
    let (r, page) = binds_rig().await;
    let original = seed(&r, BINDS_QUIET);
    let out = run(
        &page,
        Op::Export {
            id: "quiet".into(),
            path: file.clone(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(Profile::parse(&text).unwrap(), original);
    r.profiles.store.lock().unwrap().clear();
    let out = run(
        &page,
        Op::Import {
            path: file.clone(),
            replace: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(stored(&r, "quiet").unwrap(), original);
    for body in [
        "\"uq.close\" = \"off\"\n",
        "\"uq.lock-recovery\" = false\n",
        "\"uq.nope\" = true\n",
    ] {
        let hostile = format!("format = 1\nid = \"bad\"\nname = \"Bad\"\n[keybinds]\n{body}");
        std::fs::write(&file, hostile).unwrap();
        let out = run(
            &page,
            Op::Import {
                path: file.clone(),
                replace: true,
            },
        )
        .await;
        let e = out.error.expect(body);
        assert!(
            e.starts_with("A shortcut") || e.starts_with("Keyboard shortcuts"),
            "{e}"
        );
        assert!(!e.contains('\n'), "{e:?}");
    }
    assert!(stored(&r, "bad").is_none());
}
