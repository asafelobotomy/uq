//! Applying, importing, exporting and the snapshot.

use super::*;

#[tokio::test]
async fn apply_then_off_reports_kept_keys() {
    let (r, page) = rig().await;
    seed(&r, GAMING);
    let out = run(
        &page,
        Op::Apply {
            id: "gaming".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(gaps(&r), Some(0));
    assert_eq!(out.state["active"], "gaming");
    assert_eq!(out.state["profiles"][0]["active"], true);
    assert_eq!(out.state["report"]["kept"], json!([]));
    let applied_seq = out.state["report"]["seq"].as_u64().unwrap();
    assert!(applied_seq > 0);

    // Gaps changed by hand while the profile is on: kept and named by "Back to normal".
    let proxy = DesktopProxy::new(&r.client).await.unwrap();
    proxy
        .set_section("look", r#"{"gaps_in": 3, "blur": false}"#, "settings")
        .await
        .unwrap();
    let out = run(&page, Op::Off).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["report"]["kept"], json!(["Gaps between windows"]));
    assert_eq!(out.state["report"]["notices"], json!([]));
    assert!(out.state["report"]["seq"].as_u64().unwrap() > applied_seq);
    assert_eq!(out.state["active"], "");
    assert_eq!(gaps(&r), Some(3));
    assert_eq!(
        r.state.lock().unwrap().section("look").unwrap()["blur"],
        true,
        "the untouched key comes back"
    );
    let origins = r.history.origins();
    assert_eq!(origins.first().map(String::as_str), Some("profile:gaming"));
    assert_eq!(origins.last().map(String::as_str), Some("profile:gaming"));
}

#[tokio::test]
async fn apply_with_power_shows_notice() {
    let (r, page) = rig().await;
    seed(&r, TURBO);
    let out = run(&page, Op::Apply { id: "turbo".into() }).await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["report"]["notices"],
        json!([power_notice(
            "power modes aren't available on this computer"
        )])
    );
    assert_eq!(out.state["profiles"][0]["power"], "Performance");
    assert_eq!(gaps(&r), Some(0), "the settings still apply");
}

#[tokio::test]
async fn export_then_import_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("gaming.toml");
    let (r, page) = rig().await;
    let original = seed(&r, GAMING);
    let out = run(
        &page,
        Op::Export {
            id: "gaming".into(),
            path: file.clone(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(Profile::parse(&text).unwrap(), original);

    // Change the stored one, then bring the file back.
    run(
        &page,
        Op::Rename {
            id: "gaming".into(),
            name: "Changed".into(),
            icon: String::new(),
        },
    )
    .await;
    let out = run(
        &page,
        Op::Import {
            path: file.clone(),
            replace: false,
        },
    )
    .await;
    assert_eq!(out.error, None, "a conflict is asked about, not an error");
    let conflict = &out.state["conflict"];
    assert_eq!(conflict["path"], file.display().to_string());
    // The page asks the question; uqd's own "Replace it?" would ask it twice.
    assert_eq!(
        conflict["message"],
        "A profile called \u{201c}Changed\u{201d} already exists."
    );
    assert!(conflict["seq"].as_u64().unwrap() > 0);
    assert_eq!(stored(&r, "gaming").unwrap().name, "Changed");

    let out = run(
        &page,
        Op::Import {
            path: file.clone(),
            replace: true,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["conflict"], Value::Null);
    assert_eq!(stored(&r, "gaming").unwrap(), original);

    // A new id imports without asking.
    std::fs::write(&file, TURBO).unwrap();
    let out = run(
        &page,
        Op::Import {
            path: file,
            replace: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["conflict"], Value::Null);
    assert!(stored(&r, "turbo").is_some());
}

#[tokio::test]
async fn import_refuses_hostile_file() {
    let dir = tempfile::tempdir().unwrap();
    let (r, page) = rig().await;
    let cases: [(&str, Vec<u8>, &str); 4] = [
        (
            "big.toml",
            format!("{GAMING}# {}\n", "x".repeat(100 * 1024)).into_bytes(),
            "The file is too big to be a profile.",
        ),
        (
            "binary.toml",
            vec![0xff, 0xfe, 0x00, 0x01],
            "This file isn't a uQ profile.",
        ),
        (
            "escape.toml",
            b"format = 1\nid = \"../x\"\nname = \"X\"\n[look]\ngaps_in = 1\n".to_vec(),
            "The profile's id can only use a\u{2013}z, 0\u{2013}9 and dashes.",
        ),
        (
            "binds.toml",
            b"format = 1\nid = \"x\"\nname = \"X\"\n[keybinds]\nterminal = true\n".to_vec(),
            "Keyboard shortcuts: \u{201c}terminal\u{201d} isn't one of uQ's shortcuts",
        ),
    ];
    for (name, bytes, message) in cases {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let out = run(
            &page,
            Op::Import {
                path,
                replace: true,
            },
        )
        .await;
        assert_eq!(out.error.as_deref(), Some(message), "{name}");
        assert_eq!(out.state["conflict"], Value::Null);
    }
    assert!(r.profiles.store.lock().unwrap().is_empty());
}

#[tokio::test]
async fn delete_active_is_refused_with_message() {
    let (r, page) = rig().await;
    seed(&r, GAMING);
    seed(&r, TURBO);
    run(
        &page,
        Op::Apply {
            id: "gaming".into(),
        },
    )
    .await;
    let out = run(
        &page,
        Op::Delete {
            id: "gaming".into(),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("Turn this profile off before deleting it.")
    );
    assert!(stored(&r, "gaming").is_some());

    let out = run(&page, Op::Delete { id: "turbo".into() }).await;
    assert_eq!(out.error, None);
    assert!(stored(&r, "turbo").is_none());
    assert_eq!(out.state["profiles"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn snapshot_lists_available_keys_pages_and_icons() {
    let (_r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let st = &out.state;
    assert_eq!(st["icons"], json!(ICONS));
    assert_eq!(
        st["pages"],
        json!([
            {"id": "appearance", "name": "Appearance"},
            {"id": "look", "name": "Look and layouts"},
            {"id": "wallpaper", "name": "Wallpaper"},
            {"id": "apps-scaling", "name": "Apps and scaling"},
            {"id": "shell", "name": "Shell"},
            {"id": "rules", "name": "Window rules"},
            {"id": "keybinds", "name": "Keyboard shortcuts"},
            {"id": "startup", "name": "Startup"},
        ])
    );
    let available = st["available"].as_array().unwrap();
    let find = |domain: &str, path: Value| {
        available
            .iter()
            .find(|k| k["domain"] == domain && k["path"] == path)
            .cloned()
    };
    assert_eq!(
        find("look", json!(["gaps_in"])),
        Some(
            json!({"domain": "look", "path": ["gaps_in"], "label": "Gaps between windows", "text": "8"})
        )
    );
    assert_eq!(
        find("appearance", json!(["theme"])).unwrap()["text"],
        "gruvbox"
    );
    assert_eq!(
        find("appearance", json!(["variant"])).unwrap()["text"],
        "light"
    );
    assert_eq!(find("look", json!(["blur"])).unwrap()["text"], "On");
    assert!(
        find("look", json!(["rounding"])).is_none(),
        "unset keys aren't offered"
    );
    assert!(available.iter().all(|k| k["domain"] != "power"));
    assert_eq!(st["conflict"], Value::Null);
    assert_eq!(st["report"], json!({"seq": 0, "kept": [], "notices": []}));
}

#[tokio::test]
async fn profiles_survive_reopening_the_model() {
    let (r, page) = rig().await;
    let every_kind = "format = 1\nid = \"all\"\nname = \"All kinds\"\nicon = \"media-playback-start\"\n\
        [look]\ngaps_in = 4\nblur = false\nanimation_speed = 1.5\n\
        [shell]\nclock_24h = false\n[shell.islands]\ntime = false\n\
        [appearance]\ntheme = \"dracula\"\nvariant = \"dark\"\nui_font = \"Inter\"\n\
        [power]\nprofile = \"power-saver\"\n";
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("all.toml");
    std::fs::write(&file, every_kind).unwrap();
    let out = run(
        &page,
        Op::Import {
            path: file,
            replace: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    let out = run(
        &page,
        Op::AddKeys {
            id: "all".into(),
            keys: vec![("shell".into(), s(&["providers"]))],
        },
    )
    .await;
    assert_eq!(out.error, None);
    let before = out.state["profiles"].clone();

    let reopened = ProfilesState::new(r.client.clone());
    let out = run(&reopened, Op::Refresh).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["profiles"], before);
    let mut expected = Profile::parse(every_kind).unwrap();
    expected.with_key(uq_core::profile::Key {
        domain: "shell".into(),
        path: s(&["providers"]),
        value: json!([]),
    });
    assert_eq!(stored(&r, "all").unwrap(), expected);
    assert_eq!(
        shown_keys(&out.state, 0),
        [
            "look.animation_speed",
            "look.blur",
            "look.gaps_in",
            "shell.clock_24h",
            "shell.islands.time",
            "shell.providers",
            "appearance.theme",
            "appearance.ui_font",
            "appearance.variant",
            "power.profile",
        ]
    );
    assert_eq!(out.state["profiles"][0]["power"], "Power saver");
}

#[tokio::test]
async fn uqd_down_gives_empty_state_without_an_error() {
    let bus = private_bus();
    let page = ProfilesState::new(connect(&bus).await);
    let out = run(&page, Op::Refresh).await;
    assert_eq!((out.state, out.error), (json!({}), None));
    let out = run(
        &page,
        Op::Apply {
            id: "gaming".into(),
        },
    )
    .await;
    assert_eq!((out.state, out.error), (json!({}), None));
}
