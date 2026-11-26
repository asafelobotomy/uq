//! Switches, blurry apps and page states.

use super::*;

#[tokio::test]
async fn switch_round_trips_and_journals() {
    let (r, page, _d) = rig().await;
    for key in SwitchKey::ALL {
        let out = run(
            &page,
            Op::SetSwitch {
                key: key.key().into(),
                on: true,
            },
        )
        .await;
        assert_eq!(out.error, None, "{}", key.key());
        let row = switch(&out.state, key.key());
        assert_eq!(row["on"], true);
        assert_eq!(row["label"], key.label());
        assert_eq!(row["description"], key.description());
        assert_eq!(section(&r)[key.key()], true);
    }
    assert_eq!(r.history.origins(), ["settings"; 5]);
    let out = run(
        &page,
        Op::SetSwitch {
            key: "qt_wayland".into(),
            on: false,
        },
    )
    .await;
    assert_eq!(switch(&out.state, "qt_wayland")["on"], false);
    assert_eq!(section(&r)["qt_wayland"], false);
    assert_eq!(r.history.origins().len(), 6);
    let out = run(
        &page,
        Op::SetSwitch {
            key: "nope".into(),
            on: true,
        },
    )
    .await;
    assert!(out.error.is_some());
    assert_eq!(r.history.origins().len(), 6);
}

#[tokio::test]
async fn blurry_lists_only_xwayland_and_names_the_app() {
    let (r, page, _d) = rig().await;
    r.windows.set(vec![
        win("Code", "main.rs - Code", true),
        win("Code", "lib.rs - Code", true),
        win("firefox", "Mozilla Firefox", false),
        win("mystery-app", "Mystery", true),
    ]);
    let out = run(&page, Op::RefreshWindows).await;
    assert_eq!(out.error, None);
    let rows = out.state["blurry"]["windows"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0]["class"], "Code");
    assert_eq!(rows[0]["title"], "main.rs - Code");
    assert_eq!(rows[0]["app_id"], "code.desktop");
    assert_eq!(rows[0]["app_name"], "Visual Studio Code");
    assert_eq!(rows[0]["app_icon"], "vscode");
    assert_eq!(rows[0]["has_fix"], false);
    assert_eq!(rows[1]["class"], "mystery-app");
    assert_eq!(rows[1]["app_id"], Value::Null);
    assert_eq!(rows[1]["app_name"], Value::Null);
    assert_eq!(rows[1]["app_icon"], "");
    assert_eq!(rows[1]["has_fix"], false);

    run(&page, save(code_fix())).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["blurry"]["windows"][0]["has_fix"], true);
}

#[tokio::test]
async fn blurry_heading_states() {
    let (r, page, _d) = rig().await;
    // The fake's two outputs are scaled 1.6 and 1.5.
    let out = run(&page, Op::Refresh).await;
    let blurry = &out.state["blurry"];
    assert_eq!(blurry["scaled"], true);
    assert_eq!(blurry["sharp_on"], false);
    assert_eq!(blurry["heading"], BLURRY);

    let on = Op::SetSwitch {
        key: "sharp_x11".into(),
        on: true,
    };
    let out = run(&page, on).await;
    assert_eq!(out.state["blurry"]["sharp_on"], true);
    assert_eq!(out.state["blurry"]["heading"], SHARP_ON);

    let off = Op::SetSwitch {
        key: "sharp_x11".into(),
        on: false,
    };
    run(&page, off).await;
    for o in r.displays.live_outputs.lock().unwrap().iter_mut() {
        o.scale = 1.0;
    }
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["blurry"]["scaled"], false);
    assert_eq!(out.state["blurry"]["heading"], NOT_SCALED);
    assert_eq!(
        NOT_SCALED,
        "Your screens aren't scaled, so nothing is blurry."
    );
    assert_eq!(BLURRY, "These older apps may look blurry.");
    assert_eq!(SHARP_ON, "These older apps are open right now.");

    // A disabled output's scale doesn't count.
    {
        let mut outputs = r.displays.live_outputs.lock().unwrap();
        outputs[0].scale = 2.0;
        outputs[0].disabled = true;
    }
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["blurry"]["scaled"], false);
}

#[test]
fn app_for_class_order() {
    let id = |apps: &[AppEntry], class: &str| app_for_class(class, apps).map(|a| a.id.clone());
    // 1. StartupWMClass, ignoring ASCII case.
    let apps = [entry("a.desktop", Some("Thing"), None)];
    assert_eq!(id(&apps, "thing").as_deref(), Some("a.desktop"));
    // 2. The id without `.desktop`.
    let apps = [entry("Foo.desktop", None, None)];
    assert_eq!(id(&apps, "foo").as_deref(), Some("Foo.desktop"));
    // 3. The id's last dot part.
    let apps = [entry("org.gnome.Nautilus.desktop", None, None)];
    assert_eq!(
        id(&apps, "nautilus").as_deref(),
        Some("org.gnome.Nautilus.desktop")
    );
    assert_eq!(
        id(&apps, "org.gnome.nautilus").as_deref(),
        Some("org.gnome.Nautilus.desktop")
    );
    // 4. The program's file name, exactly.
    let apps = [entry("x.desktop", None, Some("bar"))];
    assert_eq!(id(&apps, "bar").as_deref(), Some("x.desktop"));
    assert_eq!(id(&apps, "Bar"), None);
    assert_eq!(id(&apps, "unknown"), None);

    // An earlier rule wins over a later one, whatever the list order.
    let apps = [
        entry("code.desktop", None, None),
        entry("q.desktop", Some("code"), None),
    ];
    assert_eq!(id(&apps, "Code").as_deref(), Some("q.desktop"));
    let apps = [
        entry("org.x.Foo.desktop", None, None),
        entry("foo.desktop", None, None),
    ];
    assert_eq!(id(&apps, "foo").as_deref(), Some("foo.desktop"));
    let apps = [
        entry("t.desktop", None, Some("foo")),
        entry("org.u.Foo.desktop", None, None),
    ];
    assert_eq!(id(&apps, "foo").as_deref(), Some("org.u.Foo.desktop"));
}

#[tokio::test]
async fn sharp_x11_off_reads_default() {
    let (_r, page, _d) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(
        switch(&out.state, "sharp_x11")["origin"]["source"],
        "default"
    );
    assert_eq!(
        switch(&out.state, "electron_wayland")["origin"]["source"],
        "uq"
    );
    let on = Op::SetSwitch {
        key: "sharp_x11".into(),
        on: true,
    };
    let out = run(&page, on).await;
    assert_eq!(switch(&out.state, "sharp_x11")["origin"]["source"], "uq");
    assert_eq!(out.state["section"], Value::Null);
}

#[tokio::test]
async fn uqd_down_gives_empty_state_without_an_error() {
    let bus = private_bus();
    let page = CompatState::new(connect(&bus).await);
    assert_eq!(
        run(&page, Op::Refresh).await,
        Outcome {
            state: json!({}),
            error: None
        }
    );
    assert_eq!(run(&page, Op::RefreshWindows).await.error, None);
    assert_eq!(run(&page, save(code_fix())).await.error, None);
}

#[tokio::test]
async fn row_says_from_the_profile() {
    let (r, page, _d) = rig().await;
    let gaming = "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[compat]\nqt_wayland = true\n\
                  [compat.apps.\"code.desktop\"]\nplatform = \"x11\"\n";
    let proxy = uq_core::client::DesktopProxy::new(&r.client).await.unwrap();
    proxy.set_profile(gaming, false).await.unwrap();
    proxy.apply_profile("gaming").await.unwrap();
    let out = run(&page, Op::Refresh).await;
    let qt = switch(&out.state, "qt_wayland");
    assert_eq!(qt["origin"]["source"], "profile");
    assert_eq!(qt["origin"]["detail"], json!({"name": "Gaming"}));
    let fix = &out.state["fixes"][0];
    assert_eq!(fix["origin"]["source"], "profile");
    assert_eq!(fix["origin"]["detail"], json!({"name": "Gaming"}));
    // The name is carried once, in the origin.
    for row in [qt, fix] {
        let text = row.to_string();
        assert_eq!(text.matches("Gaming").count(), 1, "{text}");
    }
    assert_eq!(
        switch(&out.state, "electron_wayland")["origin"]["source"],
        "uq"
    );
}
