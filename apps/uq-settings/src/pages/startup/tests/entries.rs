//! Autostart entries.

use super::*;

#[tokio::test]
async fn toggle_system_entry_round_trips() {
    let (r, page, _d) = rig().await;
    let out = run(&page, on("nm-applet.desktop", false)).await;
    assert_eq!(out.error, None);
    assert_eq!(
        section(&r, "autostart")["files"],
        json!([{"id": "nm-applet.desktop", "enabled": false, "delay": null}])
    );
    let row = app(&out.state, "nm-applet.desktop");
    assert_eq!(row["enabled"], false);
    assert_eq!(row["status"], "off");
    assert_eq!(row["status_text"], "Off");
    assert_eq!(row["origin"]["source"], "uq");

    let again = StartupState::with_dirs(r.client.clone(), page.app_dirs.clone());
    assert_eq!(run(&again, Op::Refresh).await.state, out.state);

    assert_eq!(r.history.origins(), ["settings"]);
    let entries = r.history.entries.lock().unwrap();
    assert_eq!(entries[0].domain, "autostart");
    assert!(
        entries[0].summary.contains("nm-applet"),
        "{}",
        entries[0].summary
    );
}

#[tokio::test]
async fn add_app_and_command_round_trip() {
    let (r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::AddApp {
            desktop_id: "syncthing-start".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let out = run(
        &page,
        Op::AddCommand {
            name: "Backup check".into(),
            command: r#"backup-check --quiet "a b" 100%"#.into(),
        },
    )
    .await;
    assert_eq!(out.error, None);

    let st = section(&r, "autostart");
    assert_eq!(st["added"][0]["name"], "Syncthing");
    assert_eq!(st["added"][0]["app"], "syncthing-start.desktop");
    assert_eq!(
        st["added"][1]["argv"],
        json!(["backup-check", "--quiet", "a b", "100%"])
    );
    let ids = added_ids(&r);
    let syncthing = app(&out.state, &ids[0]);
    assert_eq!(syncthing["source"], "added");
    assert_eq!(syncthing["command_text"], Value::Null);
    let cmd = app(&out.state, &ids[1]);
    assert_eq!(cmd["name"], "Backup check");
    assert_eq!(cmd["command_text"], r#"backup-check --quiet "a b" 100%"#);
    assert_eq!(cmd["status_text"], "Starts next time you log in");
    let installed = out.state["installed"].as_array().unwrap();
    assert!(
        installed
            .iter()
            .any(|a| a["id"] == "syncthing-start.desktop"
                && a["name"] == "Syncthing"
                && a["icon"] == "syncthing"),
        "{installed:?}"
    );

    // Its own entries can be switched, delayed and removed.
    assert_eq!(run(&page, on(&ids[1], false)).await.error, None);
    let out = run(
        &page,
        Op::SetDelay {
            id: ids[0].clone(),
            delay: Some(10),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let st = section(&r, "autostart");
    assert_eq!(st["added"][1]["enabled"], false);
    assert_eq!(st["added"][0]["delay"], 10);
    let out = run(&page, Op::Remove { id: ids[0].clone() }).await;
    assert_eq!(out.error, None);
    assert_eq!(added_ids(&r), [ids[1].clone()]);
}

#[tokio::test]
async fn add_app_long_name_is_cut() {
    let (r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::AddApp {
            desktop_id: "long.desktop".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(
        section(&r, "autostart")["added"][0]["name"],
        "L".repeat(100)
    );

    let out = run(
        &page,
        Op::AddApp {
            desktop_id: "gone.desktop".into(),
        },
    )
    .await;
    assert!(out.error.is_some());
    assert_eq!(added_ids(&r).len(), 1);
}

#[tokio::test]
async fn bad_command_is_a_plain_error() {
    let (r, page, _d) = rig().await;
    for command in [r#"backup-check "a b"#, "  ", "sh -c 'x'"] {
        let out = run(
            &page,
            Op::AddCommand {
                name: "Backup".into(),
                command: command.into(),
            },
        )
        .await;
        let error = out.error.unwrap();
        assert!(
            !error.contains("Exec") && !error.contains("autostart"),
            "{error}"
        );
        assert!(!error.is_empty());
    }
    let out = run(
        &page,
        Op::AddCommand {
            name: "Backup".into(),
            command: r#"backup-check "a b"#.into(),
        },
    )
    .await;
    assert_eq!(out.error.as_deref(), Some(UNCLOSED_QUOTE));
    assert!(added_ids(&r).is_empty());
}

#[tokio::test]
async fn remove_file_entry_refused() {
    let (r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::Remove {
            id: "nm-applet.desktop".into(),
        },
    )
    .await;
    assert!(
        out.error
            .as_deref()
            .unwrap()
            .contains("Turn it off instead"),
        "{:?}",
        out.error
    );
    assert!(r.history.origins().is_empty());
}

#[tokio::test]
async fn delay_round_trips_and_range_refused() {
    let (r, page, _d) = rig().await;
    let delay = |d| Op::SetDelay {
        id: "nm-applet.desktop".into(),
        delay: d,
    };
    let out = run(&page, delay(Some(30))).await;
    assert_eq!(out.error, None);
    assert_eq!(
        section(&r, "autostart")["files"],
        json!([{"id": "nm-applet.desktop", "enabled": null, "delay": 30}])
    );
    assert_eq!(app(&out.state, "nm-applet.desktop")["delay"], 30);

    let out = run(&page, delay(Some(601))).await;
    let error = out.error.unwrap();
    assert!(
        !error.contains("autostart.") && !error.contains('['),
        "{error}"
    );
    assert!(error.contains("600"), "{error}");
    assert_eq!(section(&r, "autostart")["files"][0]["delay"], 30);
}

#[tokio::test]
async fn clearing_delay_and_enabled_removes_choice() {
    let (r, page, _d) = rig().await;
    let delay = |id: &str, d| Op::SetDelay {
        id: id.into(),
        delay: d,
    };
    assert_eq!(
        run(&page, delay("nm-applet.desktop", Some(30))).await.error,
        None
    );
    assert_eq!(run(&page, on("notes.desktop", false)).await.error, None);
    assert_eq!(
        run(&page, delay("notes.desktop", Some(5))).await.error,
        None
    );

    // A choice left with neither field goes; one that still says on or off stays.
    assert_eq!(
        run(&page, delay("nm-applet.desktop", None)).await.error,
        None
    );
    assert_eq!(run(&page, delay("notes.desktop", None)).await.error, None);
    assert_eq!(
        section(&r, "autostart")["files"],
        json!([{"id": "notes.desktop", "enabled": false, "delay": null}])
    );
}

#[tokio::test]
async fn yours_off_says_off_in_uq() {
    let (_r, page, _d) = rig().await;
    let out = run(&page, on("notes.desktop", false)).await;
    assert_eq!(out.error, None);
    let notes = app(&out.state, "notes.desktop");
    assert_eq!(notes["status"], "off");
    assert_eq!(notes["status_text"], "Off in uQ");
    let out = run(&page, on("nm-applet.desktop", false)).await;
    assert_eq!(app(&out.state, "nm-applet.desktop")["status_text"], "Off");
}

#[tokio::test]
async fn uqd_down_gives_empty_state_without_an_error() {
    let bus = private_bus();
    // `new` reads only the environment for its folders; with uqd down no folder is read.
    let page = StartupState::new(connect(&bus).await);
    assert_eq!(
        run(&page, Op::Refresh).await,
        Outcome {
            state: json!({}),
            error: None
        }
    );
    assert_eq!(run(&page, set_var(None, "FOO", "1")).await.error, None);
}

#[tokio::test]
async fn row_says_from_the_profile() {
    let (r, page, _d) = rig().await;
    run(&page, set_var(None, "FOO", "1")).await;
    let gaming = "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[env]\nFOO = \"2\"\n\
                  [autostart.\"nm-applet.desktop\"]\nenabled = false\n";
    let proxy = client::DesktopProxy::new(&r.client).await.unwrap();
    proxy.set_profile(gaming, false).await.unwrap();
    proxy.apply_profile("gaming").await.unwrap();
    let out = run(&page, Op::Refresh).await;
    let from_gaming = json!({"source": "profile", "detail": {"name": "Gaming"}});
    let foo = &out.state["vars"][0];
    assert_eq!(foo["origin"]["source"], from_gaming["source"]);
    assert_eq!(foo["origin"]["detail"], from_gaming["detail"]);
    let row = app(&out.state, "nm-applet.desktop");
    assert_eq!(row["origin"]["source"], "profile");
    assert_eq!(row["origin"]["detail"], json!({"name": "Gaming"}));
    // The name is carried once, in the origin.
    for r in [foo, row] {
        let text = r.to_string();
        assert_eq!(text.matches("Gaming").count(), 1, "{text}");
    }
    proxy.profile_off().await.unwrap();
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["vars"][0]["origin"]["source"], "uq");
}
