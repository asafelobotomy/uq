use super::*;
use crate::env::Var;

const PATH: &str = "/home/u/user.lua";

fn state_with_env(vars: &[(&str, &str)]) -> State {
    let mut st = State::default();
    st.env.vars = vars
        .iter()
        .map(|(n, v)| Var {
            name: n.to_string(),
            value: v.to_string(),
        })
        .collect();
    st
}

#[test]
fn env_row_user_lua_line() {
    let st = state_with_env(&[("FOO", "x"), ("BAR", "y")]);
    let lua = "-- top\n\nhl.env(\"FOO\", \"x\")\n-- hl.env(\"BAR\", \"y\")\n";
    let v = explain_env(&st, Some(lua), None, &FileReport::default(), PATH);
    assert_eq!(v["vars.FOO"]["source"], "user_lua");
    assert_eq!(v["vars.FOO"]["detail"], json!({"line": 3, "path": PATH}));
    assert_eq!(v["vars.BAR"]["source"], "uq");
    assert_eq!(v["vars.BAR"]["live"], Value::Null);
    assert_eq!(v["vars.BAR"]["stored"], Value::Null);
    assert!(v.get("env").is_none());
}

#[test]
fn env_row_replaced_by_switch() {
    let mut st = state_with_env(&[("QT_QPA_PLATFORM", "xcb"), ("FOO", "x")]);
    st.compat.qt_wayland = true;
    let v = explain_env(&st, None, None, &FileReport::default(), PATH);
    assert_eq!(v["vars.QT_QPA_PLATFORM"]["source"], "uq");
    assert_eq!(
        v["vars.QT_QPA_PLATFORM"]["detail"],
        json!({"replaced_by": "qt_wayland"})
    );
    assert_eq!(v["vars.FOO"]["detail"], json!({}));
    st.compat.qt_wayland = false;
    let v = explain_env(&st, None, None, &FileReport::default(), PATH);
    assert_eq!(v["vars.QT_QPA_PLATFORM"]["detail"], json!({}));
}

#[test]
fn env_rejected_gives_section_row() {
    let v = explain_env(
        &State::default(),
        None,
        Some("bad"),
        &FileReport::default(),
        PATH,
    );
    assert_eq!(
        v,
        json!({"env": {"source": "error", "live": null, "stored": null,
            "detail": {"message": "bad"}}})
    );
    let st = state_with_env(&[("FOO", "x")]);
    let lua = "hl.env(\"FOO\", \"x\")\n";
    let v = explain_env(&st, Some(lua), Some("bad"), &FileReport::default(), PATH);
    assert_eq!(v["vars.FOO"]["source"], "error");
    assert_eq!(v["env"]["source"], "error");
}

#[test]
fn env_file_error_reported() {
    let st = state_with_env(&[("FOO", "x")]);
    let report = FileReport {
        env_file: Some("Permission denied".into()),
        ..FileReport::default()
    };
    let v = explain_env(&st, None, None, &report, PATH);
    assert_eq!(v["vars.FOO"]["source"], "error");
    assert_eq!(
        v["vars.FOO"]["detail"],
        json!({"message": "Permission denied"})
    );
}

#[test]
fn compat_switch_user_lua() {
    let mut st = State::default();
    st.compat.electron_wayland = true;
    st.compat.qt_wayland = true;
    let lua = "x = 1\nhl.env('ELECTRON_OZONE_PLATFORM_HINT', 'x11')\n";
    let v = explain_compat_extra(&st, Some(lua), None, &FileReport::default(), PATH);
    assert_eq!(v["electron_wayland"]["source"], "user_lua");
    assert_eq!(
        v["electron_wayland"]["detail"],
        json!({"line": 2, "path": PATH})
    );
    assert_eq!(v["qt_wayland"]["source"], "uq");
    assert_eq!(v["steam_scale"]["source"], "uq");
    assert!(v.get("sharp_x11").is_none());
    assert!(v.get("compat").is_none());
}

#[test]
fn compat_browser_details() {
    let report = FileReport {
        browsers: vec![
            ("Chromium".into(), BrowserFile::Ours),
            ("Brave".into(), BrowserFile::OwnFile),
            ("Vivaldi".into(), BrowserFile::None),
        ],
        ..FileReport::default()
    };
    let v = explain_compat_extra(&State::default(), None, None, &report, PATH);
    assert_eq!(v["chromium_wayland"]["source"], "uq");
    assert_eq!(
        v["chromium_wayland"]["detail"],
        json!({"browsers": [
            {"name": "Chromium", "file": "ours"},
            {"name": "Brave", "file": "own_file"},
            {"name": "Vivaldi", "file": "none"},
        ]})
    );
    let report = FileReport {
        browsers: vec![("Brave".into(), BrowserFile::Error("Read-only".into()))],
        ..FileReport::default()
    };
    let v = explain_compat_extra(&State::default(), None, None, &report, PATH);
    assert_eq!(v["chromium_wayland"]["source"], "error");
    assert_eq!(
        v["chromium_wayland"]["detail"]["browsers"][0],
        json!({"name": "Brave", "file": "error", "message": "Read-only"})
    );
}

#[test]
fn compat_app_override_states() {
    let mut st = State::default();
    let mut elsewhere = AppFix::new("e.desktop");
    elsewhere.elsewhere = true;
    st.compat.apps = vec![
        AppFix::new("w.desktop"),
        AppFix::new("o.desktop"),
        AppFix::new("n.desktop"),
        elsewhere,
        AppFix::new("x.desktop"),
        AppFix::new("f.desktop"),
    ];
    let mut report = FileReport::default();
    report
        .overrides
        .insert("w.desktop".into(), OverrideState::Written);
    report
        .overrides
        .insert("o.desktop".into(), OverrideState::OwnFile);
    report
        .overrides
        .insert("n.desktop".into(), OverrideState::NoSource);
    report
        .overrides
        .insert("e.desktop".into(), OverrideState::NoSource);
    report
        .overrides
        .insert("x.desktop".into(), OverrideState::Error("Disk full".into()));
    let v = explain_compat_extra(&st, None, None, &report, PATH);
    let row = |id: &str| &v[format!("apps.{id}")];
    assert_eq!(row("w.desktop")["source"], "uq");
    assert_eq!(row("w.desktop")["detail"], json!({"override": "written"}));
    assert_eq!(row("o.desktop")["source"], "uq");
    assert_eq!(row("o.desktop")["detail"]["override"], "own_file");
    assert_eq!(row("n.desktop")["source"], "uq");
    assert_eq!(row("n.desktop")["detail"]["override"], "no_source");
    assert_eq!(row("e.desktop")["source"], "error");
    assert_eq!(row("e.desktop")["detail"]["override"], "no_source");
    assert!(row("e.desktop")["detail"]["message"].is_string());
    assert_eq!(row("x.desktop")["source"], "error");
    assert_eq!(
        row("x.desktop")["detail"],
        json!({"override": "error", "message": "Disk full"})
    );
    assert_eq!(row("f.desktop")["detail"], json!({"override": "off"}));
    assert_eq!(row("f.desktop")["source"], "uq");
}

#[test]
fn compat_rejected_gives_section_row() {
    let mut st = State::default();
    st.compat.apps = vec![AppFix::new("a.desktop")];
    let v = explain_compat_extra(&st, None, Some("bad"), &FileReport::default(), PATH);
    for k in [
        "compat",
        "electron_wayland",
        "chromium_wayland",
        "apps.a.desktop",
    ] {
        assert_eq!(v[k]["source"], "error", "{k}");
        assert_eq!(v[k]["detail"]["message"], "bad", "{k}");
    }
}

#[test]
fn sharp_x11_off_is_default() {
    let row = |source: &str| {
        json!({"sharp_x11": {"source": source, "live": null,
        "stored": null, "detail": {}}, "other": {"source": "uq"}})
    };
    let mut st = State::default();
    let mut v = row("uq");
    default_when_off(&mut v, &st);
    assert_eq!(v["sharp_x11"]["source"], "default");
    assert_eq!(v["other"]["source"], "uq");
    st.compat.sharp_x11 = true;
    let mut v = row("uq");
    default_when_off(&mut v, &st);
    assert_eq!(v["sharp_x11"]["source"], "uq");
    st.compat.sharp_x11 = false;
    for s in ["user_lua", "error"] {
        let mut v = row(s);
        default_when_off(&mut v, &st);
        assert_eq!(v["sharp_x11"]["source"], s);
    }
}

#[test]
fn autostart_error_row() {
    let mut st = State::default();
    st.autostart.files = vec![crate::autostart::FileChoice {
        id: "nm-applet.desktop".into(),
        enabled: Some(false),
        delay: None,
        minimize: None,
    }];
    let v = explain_autostart(&st, &FileReport::default());
    assert_eq!(v["nm-applet.desktop"]["source"], "uq");
    assert!(v.get("autostart").is_none());
    let report = FileReport {
        autostart: Some("Permission denied".into()),
        ..FileReport::default()
    };
    let v = explain_autostart(&st, &report);
    assert_eq!(
        v["autostart"],
        json!({"source": "error", "live": null, "stored": null,
            "detail": {"message": "Permission denied"}})
    );
}
