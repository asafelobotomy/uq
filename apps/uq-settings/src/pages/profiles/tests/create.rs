//! Creating and editing profiles.

use super::*;

#[tokio::test]
async fn create_from_pages_captures_current_values() {
    let (r, page) = rig().await;
    let out = run(
        &page,
        Op::CreateFromPages {
            name: "Focus Mode".into(),
            icon: "applications-office".into(),
            pages: s(&["look", "appearance"]),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "focus-mode").expect("saved under an id made from the name");
    assert_eq!(p.name, "Focus Mode");
    assert_eq!(p.icon.as_deref(), Some("applications-office"));
    assert_eq!(p.values["look"]["gaps_in"], 8);
    assert_eq!(p.values["look"]["blur"], true);
    assert_eq!(p.values["appearance"]["theme"], "gruvbox");
    assert_eq!(p.values["appearance"]["variant"], "light");
    assert!(!p.values.contains_key("shell"), "only the pages picked");

    let shown = &out.state["profiles"][0];
    assert_eq!(shown["id"], "focus-mode");
    assert_eq!(shown["active"], false);
    let gaps = shown["keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["path"] == json!(["gaps_in"]))
        .unwrap();
    assert_eq!(gaps["label"], "Gaps between windows");
    assert_eq!(gaps["text"], "8");
    assert_eq!(out.state["active"], "");
    assert_eq!(out.state["notice"]["text"], "");

    // The same name again gets an id of its own.
    let out = run(
        &page,
        Op::CreateFromPages {
            name: "Focus Mode".into(),
            icon: String::new(),
            pages: s(&["look"]),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "focus-mode-2").unwrap();
    assert_eq!(p.icon, None);
}

#[tokio::test]
async fn create_from_history_takes_keys_changed_since() {
    let (r, page) = rig().await;
    r.history.seed(Entry::new(
        100,
        "settings",
        "look",
        "gaps",
        json!({"gaps_in": 5}),
        json!({"gaps_in": 8}),
    ));
    r.history.seed(Entry::new(
        200,
        "settings",
        "look",
        "blur",
        json!({}),
        json!({"blur": true}),
    ));
    r.history.seed(Entry::new(
        210,
        "shell",
        "settings",
        "idle",
        json!({"idle.dim": 60}),
        json!({"idle.dim": 120}),
    ));
    let out = run(
        &page,
        Op::CreateFromHistory {
            name: "Evening".into(),
            icon: "weather-clear-night".into(),
            since: 150,
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "evening").unwrap();
    assert_eq!(p.values.len(), 1, "{:?}", p.values);
    assert_eq!(p.values["look"], json!({"blur": true}));
    assert_eq!(
        out.state["notice"]["text"],
        "1 of those changes can't go in a profile"
    );
    assert!(out.state["notice"]["seq"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn create_from_history_skips_keys_that_are_now_unset() {
    let (r, page) = rig().await;
    r.history.seed(Entry::new(
        200,
        "settings",
        "look",
        "rounding",
        json!({"rounding": 4}),
        json!({}),
    ));
    r.history.seed(Entry::new(
        200,
        "settings",
        "look",
        "blur",
        json!({}),
        json!({"blur": true}),
    ));
    let out = run(
        &page,
        Op::CreateFromHistory {
            name: "Evening".into(),
            icon: String::new(),
            since: 0,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(
        stored(&r, "evening").unwrap().values["look"],
        json!({"blur": true})
    );
    assert_eq!(
        out.state["notice"]["text"],
        "1 of those changes can't go in a profile"
    );
}

#[tokio::test]
async fn create_with_nothing_to_hold_is_an_error() {
    let (r, page) = rig().await;
    let out = run(
        &page,
        Op::CreateFromPages {
            name: "Empty".into(),
            icon: String::new(),
            pages: s(&["displays", "history"]),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("A profile needs at least one setting.")
    );

    let out = run(
        &page,
        Op::CreateFromHistory {
            name: "Empty".into(),
            icon: String::new(),
            since: 0,
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("A profile needs at least one setting.")
    );
    assert!(r.profiles.store.lock().unwrap().is_empty());
    assert_eq!(out.state["profiles"], json!([]));
}

#[tokio::test]
async fn add_and_remove_keys() {
    let (r, page) = rig().await;
    seed(
        &r,
        "format = 1\nid = \"work\"\nname = \"Work\"\n[look]\ngaps_in = 2\n",
    );
    let out = run(
        &page,
        Op::AddKeys {
            id: "work".into(),
            keys: vec![
                ("look".into(), s(&["blur"])),
                ("appearance".into(), s(&["variant"])),
                // Unset now, so it can't go in.
                ("look".into(), s(&["rounding"])),
            ],
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "work").unwrap();
    assert_eq!(p.values["look"], json!({"gaps_in": 2, "blur": true}));
    assert_eq!(p.values["appearance"], json!({"variant": "light"}));
    assert_eq!(
        out.state["notice"]["text"],
        "1 of those changes can't go in a profile"
    );
    assert_eq!(
        shown_keys(&out.state, 0),
        ["look.blur", "look.gaps_in", "appearance.variant"]
    );

    for (domain, path) in [("look", "gaps_in"), ("appearance", "variant")] {
        let out = run(
            &page,
            Op::RemoveKey {
                id: "work".into(),
                domain: domain.into(),
                path: s(&[path]),
            },
        )
        .await;
        assert_eq!(out.error, None);
    }
    let p = stored(&r, "work").unwrap();
    assert_eq!(p.values.len(), 1);
    assert_eq!(p.values["look"], json!({"blur": true}));

    let out = run(
        &page,
        Op::RemoveKey {
            id: "work".into(),
            domain: "look".into(),
            path: s(&["blur"]),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("A profile needs at least one setting.")
    );
    assert_eq!(
        stored(&r, "work").unwrap().values["look"],
        json!({"blur": true})
    );
}

#[tokio::test]
async fn rename_keeps_the_keys() {
    let (r, page) = rig().await;
    seed(&r, GAMING);
    let out = run(
        &page,
        Op::Rename {
            id: "gaming".into(),
            name: "  Play time ".into(),
            icon: "input-gaming".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "gaming").unwrap();
    assert_eq!(
        (p.name.as_str(), p.icon.as_deref()),
        ("Play time", Some("input-gaming"))
    );
    assert_eq!(p.values["look"], json!({"gaps_in": 0, "blur": false}));
    assert_eq!(out.state["profiles"][0]["name"], "Play time");

    let out = run(
        &page,
        Op::Rename {
            id: "gaming".into(),
            name: " ".into(),
            icon: String::new(),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("A profile needs a name of 1 to 100 characters.")
    );
}
