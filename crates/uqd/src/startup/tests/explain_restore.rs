//! Explain, autostart, restore and safety.

use super::*;

#[tokio::test]
async fn restore_point_brings_flags_back() {
    let mut r = rig();
    put(
        &root(&r).join("share-dirs/applications/chromium.desktop"),
        SOURCE,
    );
    let flags = r
        .desktop
        .paths()
        .startup
        .config_home
        .join("chromium-flags.conf");
    write(&mut r, "compat", r#"{"chromium_wayland":true}"#)
        .await
        .unwrap();
    assert!(files::is_ours(&flags, MarkerKind::Flags));
    let point = r.desktop.save_restore_point("with flags").unwrap();
    write(&mut r, "compat", r#"{"chromium_wayland":false}"#)
        .await
        .unwrap();
    assert!(!files::present(&flags));
    r.desktop.restore(&point).await.unwrap();
    assert!(files::is_ours(&flags, MarkerKind::Flags));
    let rows = explain(&r, "compat").await;
    assert_eq!(
        rows["chromium_wayland"]["detail"]["browsers"],
        json!([{"name": "Chromium", "file": "ours"}])
    );
}

#[tokio::test]
async fn autostart_section_writes_hidden_copy_and_never_renders() {
    let mut r = rig();
    let applet = "[Desktop Entry]\nType=Application\nName=Applet\nExec=nm-applet\n";
    put(&root(&r).join("xdg/autostart/nm-applet.desktop"), applet);
    write(
        &mut r,
        "autostart",
        r#"{"files":[{"id":"nm-applet.desktop","enabled":false,"delay":null}],"added":[]}"#,
    )
    .await
    .unwrap();
    let copy = r
        .desktop
        .paths()
        .startup
        .user_autostart()
        .join("nm-applet.desktop");
    let t = std::fs::read_to_string(&copy).unwrap();
    assert!(
        t.contains("Hidden=true") && t.contains("X-uQ-Autostart=true"),
        "{t}"
    );
    let calls = r.comp.calls();
    assert!(!calls.iter().any(|c| c.starts_with("verify")), "{calls:?}");
    assert_eq!(r.desktop.history(10)[0].domain, "autostart");
    let rows = explain(&r, "autostart").await;
    assert_eq!(rows["nm-applet.desktop"]["source"], "uq");
}

#[tokio::test]
async fn tests_never_touch_the_real_home() {
    let mut r = rig();
    let s = r.desktop.paths().startup.clone();
    let mut all = vec![&s.config_home, &s.data_home, &s.home];
    all.extend(&s.config_dirs);
    all.extend(&s.data_dirs);
    all.extend(&s.runtime_dir);
    for p in all {
        assert!(p.starts_with(root(&r)), "{p:?}");
    }
    let real =
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/environment.d/60-uq.conf"));
    let real_before = real
        .as_ref()
        .map(|p| std::fs::metadata(p).ok().map(|m| m.modified().ok()));
    put(
        &root(&r).join("share-dirs/applications/chromium.desktop"),
        SOURCE,
    );
    put_source(&r);
    put(
        &root(&r).join("xdg/autostart/nm-applet.desktop"),
        "[Desktop Entry]\nType=Application\nName=A\nExec=a\n",
    );
    let mut st = r.desktop.state().clone();
    st.set_section("env", json!({"vars": [{"name": "FOO", "value": "1"}]}))
        .unwrap();
    st.set_section("compat", serde_json::from_str(ELSEWHERE).unwrap())
        .unwrap();
    st.compat.chromium_wayland = true;
    st.set_section(
        "autostart",
        json!({"files": [{"id": "nm-applet.desktop", "enabled": false}], "added": []}),
    )
    .unwrap();
    r.desktop.state = st;
    r.desktop.startup().await;
    // The full sync ran: every kind of file is there, inside the rig.
    let files = tree(root(&r));
    for want in [
        files::environment_d_path(&s),
        s.config_home.join("chromium-flags.conf"),
        override_path(&r),
        s.user_autostart().join("nm-applet.desktop"),
    ] {
        assert!(files.contains(&want), "{want:?} in {files:?}");
    }
    let real_after = real
        .as_ref()
        .map(|p| std::fs::metadata(p).ok().map(|m| m.modified().ok()));
    assert_eq!(real_before, real_after);
}

#[tokio::test]
async fn explain_says_from_the_profile() {
    let mut r = rig();
    put(
        &root(&r).join("xdg/autostart/nm-applet.desktop"),
        "[Desktop Entry]\nType=Application\nName=Applet\nExec=nm-applet\n",
    );
    put(&r.desktop.paths().user_lua, "hl.env(\"MINE\", \"x\")\n");
    let body = "format = 1\nid = \"game\"\nname = \"Gaming\"\n\
                [env]\nFOO = \"a\"\nMINE = \"y\"\n[compat]\nqt_wayland = true\n\
                [compat.apps.\"code.desktop\"]\nscale = 2.0\n\
                [autostart.\"nm-applet.desktop\"]\nenabled = false\n";
    r.desktop.set_profile(body, false).unwrap();
    r.desktop.apply_profile("game").await.unwrap();
    let from_profile =
        |row: &Value| row["source"] == "profile" && row["detail"] == json!({"name": "Gaming"});
    let env = explain(&r, "env").await;
    assert!(from_profile(&env["vars.FOO"]), "{env}");
    assert_eq!(env["vars.MINE"]["source"], "user_lua");
    let compat = explain(&r, "compat").await;
    assert!(from_profile(&compat["qt_wayland"]), "{compat}");
    assert!(from_profile(&compat["apps.code.desktop"]), "{compat}");
    assert_eq!(compat["electron_wayland"]["source"], "uq");
    let auto = explain(&r, "autostart").await;
    assert!(from_profile(&auto["nm-applet.desktop"]), "{auto}");
    // Changed by hand: uQ's again.
    write(
        &mut r,
        "env",
        r#"{"vars":[{"name":"FOO","value":"b"},{"name":"MINE","value":"y"}]}"#,
    )
    .await
    .unwrap();
    write(
        &mut r,
        "compat",
        r#"{"qt_wayland":false,"apps":[{"id":"code.desktop","scale":2.0}]}"#,
    )
    .await
    .unwrap();
    let env = explain(&r, "env").await;
    assert_eq!(env["vars.FOO"]["source"], "uq");
    let compat = explain(&r, "compat").await;
    assert_eq!(compat["qt_wayland"]["source"], "uq");
    assert!(from_profile(&compat["apps.code.desktop"]), "{compat}");
}

/// R19 on the runner path: an added app whose fix has `elsewhere` on gets the fix once, from
/// its own source, not again on top of uQ's override in the data home.
#[tokio::test]
async fn added_app_with_elsewhere_fix_gets_fix_once() {
    let mut r = rig();
    put(
        &root(&r).join("share-dirs/applications/foo.desktop"),
        "[Desktop Entry]\nType=Application\nName=Foo\nExec=foo\n",
    );
    write(
        &mut r,
        "compat",
        r#"{"apps":[{"id":"foo.desktop","env":[{"name":"FOO","value":"1"}],"args":["--bar"],"elsewhere":true}]}"#,
    )
    .await
    .unwrap();
    let target = files::override_target(&r.desktop.paths().startup, "foo.desktop").unwrap();
    assert!(files::is_ours(&target, MarkerKind::Override));
    write(
        &mut r,
        "autostart",
        r#"{"files":[],"added":[{"name":"Foo","app":"foo.desktop"}]}"#,
    )
    .await
    .unwrap();
    let (plan, _) = r.desktop.autostart_plan();
    let row = plan.iter().find(|e| e.name == "Foo").unwrap();
    let Plan::Run { argv, .. } = &row.plan else {
        panic!("{row:?}");
    };
    assert_eq!(argv, &["env", "--", "FOO=1", "foo", "--bar"], "{argv:?}");
    // The source is what runs, as with the bridge: a change to it shows before the 60 s check
    // rewrites the override.
    put(
        &root(&r).join("share-dirs/applications/foo.desktop"),
        "[Desktop Entry]\nType=Application\nName=Foo\nExec=foo2\n",
    );
    let (plan, _) = r.desktop.autostart_plan();
    let row = plan.iter().find(|e| e.name == "Foo").unwrap();
    assert!(
        matches!(&row.plan, Plan::Run { argv, .. } if argv == &["env", "--", "FOO=1", "foo2", "--bar"]),
        "{row:?}"
    );
}

/// A marked override with no valid source runs as written, without the fix again.
#[tokio::test]
async fn added_app_override_without_valid_source_runs_as_written() {
    let mut r = rig();
    write(
        &mut r,
        "compat",
        r#"{"apps":[{"id":"foo.desktop","env":[{"name":"FOO","value":"1"}],"args":["--bar"]}]}"#,
    )
    .await
    .unwrap();
    // Left from before (the sync removes it on the next compat write), naming a source outside
    // the data dirs.
    let target = files::override_target(&r.desktop.paths().startup, "foo.desktop").unwrap();
    put(
        &target,
        "[Desktop Entry]\nType=Application\nName=Foo\nExec=env -- FOO=1 foo --bar\nX-uQ-Override=true\nX-uQ-Source=/elsewhere/foo.desktop\n",
    );
    write(
        &mut r,
        "autostart",
        r#"{"files":[],"added":[{"name":"Foo","app":"foo.desktop"}]}"#,
    )
    .await
    .unwrap();
    let (plan, _) = r.desktop.autostart_plan();
    let row = plan.iter().find(|e| e.name == "Foo").unwrap();
    let Plan::Run { argv, .. } = &row.plan else {
        panic!("{row:?}");
    };
    assert_eq!(argv, &["env", "--", "FOO=1", "foo", "--bar"], "{argv:?}");
}
