//! Importing, exporting, deleting and the snapshot.

use super::*;

#[tokio::test]
async fn snapshot_lists_names_and_marks_user_and_active() {
    let (r, page, _d) = rig().await;
    r.themes.add(&text(&r, "ocean", "Ocean"));
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let s = &out.state;
    assert_eq!(ids(s), ["dracula", "gruvbox", "user-ocean"]);
    assert_eq!(row(s, "gruvbox")["name"], "Gruvbox");
    assert_eq!(row(s, "gruvbox")["active"], true);
    assert_eq!(row(s, "gruvbox")["user"], false);
    assert_eq!(row(s, "user-ocean")["name"], "Ocean");
    assert_eq!(row(s, "user-ocean")["user"], true);
    assert_eq!(row(s, "user-ocean")["active"], false);
    assert_eq!(row(s, "user-ocean")["missing"], false);
    assert_eq!(
        (&s["theme"], &s["variant"]),
        (&json!("gruvbox"), &json!("dark"))
    );
    assert_eq!(s["skipped"], json!([]));
    assert_eq!(s["rootIsLink"], false);
    assert_eq!(s["editor"], Value::Null);
    assert_eq!(s["conflict"], Value::Null);
}

#[tokio::test]
async fn import_then_use_it_now() {
    let (r, page, d) = rig().await;
    let path = file(&d, "ocean.uqtheme", &text(&r, "ocean", "Ocean"));
    let out = run(&page, import(&path, false, false)).await;
    assert_eq!(out.error, None);
    let imported = &out.state["imported"];
    assert_eq!(
        (&imported["id"], &imported["name"]),
        (&json!("user-ocean"), &json!("Ocean"))
    );
    assert_eq!(imported["unusedLook"], false);
    assert_eq!(
        out.state["theme"], "gruvbox",
        "an import never switches themes"
    );
    let out = run(
        &page,
        Op::Use {
            id: "user-ocean".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["theme"], "user-ocean");
    assert_eq!(out.state["variant"], "dark");
    assert_eq!(out.state["imported"], Value::Null);
    assert_eq!(
        r.history.origins().last().map(String::as_str),
        Some("settings")
    );
}

#[tokio::test]
async fn import_says_when_the_look_was_left_out() {
    let (r, page, d) = rig().await;
    let with_look = format!("{}\n[look]\ngaps_in = 3\n", text(&r, "ocean", "Ocean"));
    let path = file(&d, "ocean.uqtheme", &with_look);
    let out = run(&page, import(&path, false, false)).await;
    assert_eq!(out.state["imported"]["unusedLook"], true);
    assert_eq!(
        r.state.lock().unwrap().section("look").unwrap()["gaps_in"],
        8
    );
    let out = run(&page, import(&path, true, true)).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["imported"]["unusedLook"], false);
    assert_eq!(
        r.state.lock().unwrap().section("look").unwrap()["gaps_in"],
        3
    );
    assert_eq!(
        r.history.origins().last().map(String::as_str),
        Some("settings")
    );
}

#[tokio::test]
async fn import_conflict_then_replace() {
    let (r, page, d) = rig().await;
    let path = file(&d, "ocean.uqtheme", &text(&r, "ocean", "Ocean"));
    run(&page, import(&path, false, false)).await;
    let out = run(&page, import(&path, false, false)).await;
    assert_eq!(out.error, None, "a conflict is a question, not an error");
    let c = &out.state["conflict"];
    let message = c["message"].as_str().unwrap();
    assert!(message.contains("Ocean"), "{message}");
    assert!(!message.contains("Replace it?"), "{message}");
    assert_eq!(c["path"], path.display().to_string());
    assert_eq!(out.state["imported"], Value::Null);

    let out = run(&page, import(&path, true, false)).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["conflict"], Value::Null);
    assert_eq!(out.state["imported"]["id"], "user-ocean");
    assert_eq!(ids(&out.state), ["dracula", "gruvbox", "user-ocean"]);
}

#[tokio::test]
async fn import_refusals_are_plain() {
    let (r, page, d) = rig().await;
    let big = file(&d, "big.uqtheme", &"#".repeat(70_000));
    let out = run(&page, import(&big, false, false)).await;
    assert_eq!(
        out.error.as_deref(),
        Some("The file is too big to be a theme.")
    );
    assert!(r.themes.calls().is_empty(), "refused before uqd was asked");

    let hard = text(&r, "ocean", "Ocean").replacen("text = \"#ebdbb2\"", "text = \"#282828\"", 1);
    let path = file(&d, "hard.uqtheme", &hard);
    let out = run(&page, import(&path, false, false)).await;
    let error = out.error.unwrap();
    assert!(error.contains("hard to read"), "{error}");
    assert!(
        !error.contains("toml") && !error.contains("TOML") && !error.contains('{'),
        "{error}"
    );
    assert_eq!(r.themes.calls(), ["ImportTheme"]);
    assert!(!ids(&out.state).contains(&"user-ocean".to_string()));

    let out = run(
        &page,
        import(&d.path().join("missing.uqtheme"), false, false),
    )
    .await;
    assert!(out.error.unwrap().starts_with("Can't read the file"));

    *r.themes.refuse_next.lock().unwrap() = Some("Nope.".into());
    let ok = file(&d, "ok.uqtheme", &text(&r, "ocean", "Ocean"));
    let out = run(&page, import(&ok, false, false)).await;
    assert_eq!(out.error.as_deref(), Some("Nope."));
}

#[tokio::test]
async fn export_writes_the_file_and_can_include_look() {
    let (r, page, d) = rig().await;
    let path = d.path().join("out/gruvbox.uqtheme");
    let out = run(
        &page,
        Op::Export {
            id: "gruvbox".into(),
            path: path.clone(),
            with_look: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    let parsed = uqtheme::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(parsed.name, "Gruvbox");
    assert_eq!(parsed.look, None);

    let with_look = d.path().join("look.uqtheme");
    run(
        &page,
        Op::Export {
            id: "gruvbox".into(),
            path: with_look.clone(),
            with_look: true,
        },
    )
    .await;
    let parsed = uqtheme::parse(&std::fs::read_to_string(&with_look).unwrap()).unwrap();
    assert_eq!(parsed.look.unwrap()["gaps_in"], 8);

    let out = run(
        &page,
        Op::Export {
            id: "nope".into(),
            path: d.path().join("x.uqtheme"),
            with_look: false,
        },
    )
    .await;
    assert!(out.error.is_some());
    assert!(!d.path().join("x.uqtheme").exists());

    // A folder can't be written as a file.
    let out = run(
        &page,
        Op::Export {
            id: "gruvbox".into(),
            path: d.path().to_path_buf(),
            with_look: false,
        },
    )
    .await;
    assert!(out.error.unwrap().starts_with("Can't save the file: "));
    drop(r);
}

#[tokio::test]
async fn delete_active_reports_switch_and_profiles() {
    let (r, page, d) = rig().await;
    let path = file(&d, "ocean.uqtheme", &text(&r, "ocean", "Ocean"));
    run(&page, import(&path, false, false)).await;
    run(
        &page,
        Op::Use {
            id: "user-ocean".into(),
        },
    )
    .await;
    let gaming = uq_core::profile::Profile::parse(
        "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[appearance]\ntheme = \"user-ocean\"\n",
    )
    .unwrap();
    r.profiles
        .store
        .lock()
        .unwrap()
        .insert(gaming.id.clone(), gaming);

    let out = run(
        &page,
        Op::Delete {
            id: "user-ocean".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let deleted = &out.state["deleted"];
    assert_eq!(deleted["switched"], true);
    assert_eq!(deleted["profiles"], json!(["Gaming"]));
    assert_eq!(deleted["id"], "user-ocean");
    assert_eq!(out.state["theme"], "gruvbox");
    assert_eq!(ids(&out.state), ["dracula", "gruvbox"]);

    let out = run(
        &page,
        Op::Delete {
            id: "gruvbox".into(),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("Themes that come with uQ can't be deleted.")
    );
    assert_eq!(out.state["deleted"], Value::Null);
}

#[tokio::test]
async fn delete_inactive_does_not_switch() {
    let (r, page, d) = rig().await;
    let path = file(&d, "ocean.uqtheme", &text(&r, "ocean", "Ocean"));
    run(&page, import(&path, false, false)).await;
    let out = run(
        &page,
        Op::Delete {
            id: "user-ocean".into(),
        },
    )
    .await;
    assert_eq!(out.state["deleted"]["switched"], false);
    assert_eq!(out.state["deleted"]["profiles"], json!([]));
    assert_eq!(out.state["theme"], "gruvbox");
}

#[tokio::test]
async fn every_op_round_trips() {
    let (r, page, d) = rig().await;
    let path = file(&d, "ocean.uqtheme", &text(&r, "ocean", "Ocean"));
    let export = d.path().join("e.uqtheme");
    let open = Op::OpenEditor {
        from: "gruvbox".into(),
        edit: false,
    };
    let state = run(&page, open.clone()).await.state;
    let ops = vec![
        import(&path, false, false),
        Op::Use {
            id: "user-ocean".into(),
        },
        Op::Export {
            id: "user-ocean".into(),
            path: export,
            with_look: false,
        },
        Op::Save {
            draft: draft(&state, "Second"),
        },
        Op::Delete {
            id: "user-ocean".into(),
        },
        Op::Delete {
            id: "user-second".into(),
        },
    ];
    for op in ops {
        let label = format!("{op:?}");
        let out = run(&page, op).await;
        if label.starts_with("Save") {
            assert_eq!(out.error, None, "{label}");
        }
        let fresh = ThemesState::new(r.client.clone());
        let again = run(&fresh, Op::Refresh).await.state;
        for key in ["themes", "theme", "variant", "skipped"] {
            assert_eq!(again[key], out.state[key], "{label}: {key} reads back");
        }
    }
    let end = run(&page, Op::Refresh).await.state;
    assert_eq!(ids(&end), ["dracula", "gruvbox"]);
    assert_eq!(end["theme"], "gruvbox");
}

#[tokio::test]
async fn uqd_down_gives_empty_state_without_an_error() {
    let bus = private_bus();
    let page = ThemesState::new(connect(&bus).await);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(
        out,
        Outcome {
            state: json!({}),
            error: None
        }
    );
}

#[tokio::test]
async fn variants_keep_their_own_kind() {
    // The editor's two variants are the base's dark and light ones, never swapped.
    let (r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::OpenEditor {
            from: "dracula".into(),
            edit: false,
        },
    )
    .await;
    let dracula = r.themes.system_theme("dracula").unwrap();
    assert_eq!(dracula.dark.variant, Variant::Dark);
    assert_eq!(
        out.state["editor"]["colors"]["light"]["bg"],
        dracula.light.colors.bg.hex()
    );
}
