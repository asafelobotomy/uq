//! App fixes: saving, refusals, previews and browsers.

use super::*;

#[tokio::test]
async fn save_fix_round_trips() {
    let (r, page, _d) = rig().await;
    let out = run(&page, save(code_fix())).await;
    assert_eq!(out.error, None);
    assert_eq!(
        section(&r)["apps"],
        json!([{"id": "code.desktop", "platform": "x11", "scale": 1.5,
            "env": [{"name": "FOO", "value": "1"}], "args": ["--x"], "elsewhere": false}])
    );
    let fix = &out.state["fixes"][0];
    assert_eq!(fix["id"], "code.desktop");
    assert_eq!(fix["name"], "Visual Studio Code");
    assert_eq!(fix["icon"], "vscode");
    assert_eq!(fix["platform"], "x11");
    assert_eq!(fix["scale"], 1.5);
    assert_eq!(fix["env"], json!([{"name": "FOO", "value": "1"}]));
    assert_eq!(fix["args"], json!(["--x"]));
    assert_eq!(fix["elsewhere"], false);
    assert_eq!(fix["override"], "off");
    assert_eq!(fix["message"], Value::Null);
    assert_eq!(fix["origin"]["source"], "uq");

    // A save of the same app replaces its fix.
    let out = run(
        &page,
        save(json!({"id": "code.desktop", "platform": "wayland"})),
    )
    .await;
    assert_eq!(out.error, None);
    let fixes = out.state["fixes"].as_array().unwrap();
    assert_eq!(fixes.len(), 1);
    assert_eq!(fixes[0]["platform"], "wayland");
    assert_eq!(fixes[0]["scale"], Value::Null);

    // What uqd's file sync found reaches the row.
    r.files
        .lock()
        .unwrap()
        .overrides
        .insert("code.desktop".into(), OverrideState::OwnFile);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["fixes"][0]["override"], "own_file");
    assert_eq!(
        out.state["fixes"][0]["message"],
        "This app's launcher file isn't uQ's, so uQ left it alone."
    );

    let out = run(
        &page,
        Op::RemoveFix {
            id: "code.desktop".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["fixes"], json!([]));
    assert_eq!(section(&r)["apps"], json!([]));
    assert_eq!(r.history.origins(), ["settings"; 3]);

    // An empty fix is refused plainly.
    let out = run(&page, save(json!({"id": "code.desktop"}))).await;
    assert_eq!(
        out.error.as_deref(),
        Some("This fix doesn't change anything.")
    );
}

#[tokio::test]
async fn fix_with_blocked_env_shows_message() {
    let (r, page, _d) = rig().await;
    let fix = json!({"id": "code.desktop",
        "env": [{"name": "WAYLAND_DISPLAY", "value": "x"}]});
    let out = run(&page, save(fix)).await;
    let error = out.error.expect("refused");
    assert_eq!(
        error,
        "uQ needs this one to run your session, so it can't be changed here"
    );
    assert!(
        !error.contains("compat.apps") && !error.contains('['),
        "{error}"
    );
    assert!(r.history.origins().is_empty());
    assert_eq!(out.state["fixes"], json!([]));
}

#[tokio::test]
async fn elsewhere_refusal_snaps_back_with_reason() {
    let (r, page, _d) = rig().await;
    run(&page, save(code_fix())).await;
    let own = "You already have your own copy of this app's file, so uQ won't replace it.";
    *r.refuse_next.lock().unwrap() = Some(own.into());
    let on = || Op::SetElsewhere {
        id: "code".into(),
        on: true,
    };
    let out = run(&page, on()).await;
    assert_eq!(out.error.as_deref(), Some(own));
    assert_eq!(out.state["fixes"][0]["elsewhere"], false);
    assert_eq!(r.history.origins().len(), 1);

    // uqd's key is dropped from its refusal (F21).
    *r.refuse_next.lock().unwrap() =
        Some("invalid value for compat.apps[0].elsewhere: Not available in a test session".into());
    let out = run(&page, on()).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Not available in a test session")
    );

    let out = run(&page, on()).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["fixes"][0]["elsewhere"], true);

    let out = run(
        &page,
        Op::SetElsewhere {
            id: "chromium.desktop".into(),
            on: true,
        },
    )
    .await;
    assert!(out.error.is_some());
}

#[tokio::test]
async fn save_fix_asks_to_reopen_open_app() {
    let (r, page, _d) = rig().await;
    r.windows.set(vec![
        win("Code", "main.rs - Code", true),
        win("firefox", "Mozilla Firefox", false),
    ]);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(
        out.state["reopen"],
        json!({"seq": 0, "names": [], "more": 0})
    );
    let out = run(&page, save(code_fix())).await;
    assert_eq!(
        out.state["reopen"],
        json!({"seq": 1, "names": ["main.rs - Code"], "more": 0})
    );
    // No open window: no note.
    let out = run(
        &page,
        save(json!({"id": "chromium", "platform": "wayland"})),
    )
    .await;
    assert_eq!(out.state["reopen"]["seq"], 1);
    let out = run(
        &page,
        Op::RemoveFix {
            id: "code.desktop".into(),
        },
    )
    .await;
    assert_eq!(out.state["reopen"]["seq"], 2);
    // A refused save asks nothing.
    let out = run(&page, save(json!({"id": "code"}))).await;
    assert!(out.error.is_some());
    assert_eq!(out.state["reopen"]["seq"], 2);
}

#[tokio::test]
async fn fields_come_from_the_table() {
    let f = fields();
    let switches = f["switches"].as_array().unwrap();
    assert_eq!(switches.len(), SwitchKey::ALL.len());
    for (row, key) in switches.iter().zip(SwitchKey::ALL) {
        assert_eq!(row["key"], key.key());
        assert_eq!(row["label"], key.label());
        assert_eq!(row["description"], key.description());
    }
    let platforms: Vec<&str> = f["platforms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["key"].as_str().unwrap())
        .collect();
    let mut expected = vec!["auto"];
    expected.extend(PLATFORM_ENV.iter().map(|(p, _)| *p));
    assert_eq!(platforms, expected);
    let scales: Vec<f64> = f["scales"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_f64().unwrap())
        .collect();
    assert_eq!(scales.len(), 9);
    assert_eq!(scales.first(), Some(&1.0));
    assert_eq!(scales.last(), Some(&3.0));
    assert!(scales.iter().all(|s| valid_scale(*s)));

    let (_r, page, _d) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["fields"], f);
}

#[test]
fn preview_names_env_and_args() {
    let p = preview(
        &json!({"id": "chromium", "platform": "wayland", "scale": 1.5,
        "env": [{"name": "FOO", "value": "1"}], "args": ["--x"]}),
    );
    assert_eq!(
        p,
        json!({
            "env": ["GDK_BACKEND", "QT_QPA_PLATFORM", "SDL_VIDEODRIVER",
                "ELECTRON_OZONE_PLATFORM_HINT", "GDK_SCALE", "QT_SCALE_FACTOR", "FOO"],
            "args": ["--ozone-platform=wayland", "--force-device-scale-factor=1.5", "--x"],
            "problem": null,
        })
    );
    let p = preview(&json!({"id": "code.desktop", "args": ["--y"]}));
    assert_eq!(p, json!({"env": [], "args": ["--y"], "problem": null}));
    // The user's own entry for a name the platform sets is listed once.
    let p = preview(&json!({"id": "code", "platform": "x11",
        "env": [{"name": "GDK_BACKEND", "value": "wayland"}]}));
    assert_eq!(p["env"].as_array().unwrap().len(), 4);

    let p = preview(&json!({"id": "code", "env": [{"name": "bad name", "value": "1"}]}));
    let problem = p["problem"].as_str().expect("a problem");
    assert!(
        !problem.contains("compat") && !problem.contains('['),
        "{problem}"
    );
    assert_eq!(p["env"], json!([]));
    assert_eq!(p["args"], json!([]));
    let p = preview(&json!({"id": "code"}));
    assert_eq!(p["problem"], "This fix doesn't change anything.");
}

#[tokio::test]
async fn browsers_from_explain_detail() {
    let (r, page, _d) = rig().await;
    r.files.lock().unwrap().browsers = vec![
        ("Chromium".into(), BrowserFile::Ours),
        ("Brave".into(), BrowserFile::OwnFile),
        (
            "Vivaldi".into(),
            BrowserFile::Error("Read-only file system".into()),
        ),
    ];
    let out = run(&page, Op::Refresh).await;
    assert_eq!(
        out.state["browsers"],
        json!([
            {"name": "Chromium", "file": "ours", "message": null},
            {"name": "Brave", "file": "own_file",
                "message": "Brave has its own flags file, so uQ left it alone."},
            {"name": "Vivaldi", "file": "error", "message": "Read-only file system"},
        ])
    );
}
