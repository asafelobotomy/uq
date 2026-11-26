//! The environment section and the retry tick.

use super::*;

#[tokio::test]
async fn env_section_renders_hl_env_and_journals() {
    let mut r = rig();
    write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"bar"}]}"#)
        .await
        .unwrap();
    assert!(r.comp.calls().iter().any(|c| c.starts_with("verify")));
    let g = generated(&r);
    assert!(g.contains(r#"hl.env("FOO", "bar", true)"#), "{g}");
    let h = r.desktop.history(10);
    assert_eq!(h[0].domain, "env");
    assert_eq!(h[0].origin, "settings");
    assert!(h[0].summary.contains("FOO"), "{}", h[0].summary);
    let f = env_file(&r).unwrap();
    assert!(f.contains("FOO='bar'"), "{f}");
}

#[tokio::test]
async fn env_dev_session_does_not_export() {
    let mut r = rig_dev(true);
    write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"bar"}]}"#)
        .await
        .unwrap();
    let g = generated(&r);
    assert!(g.contains(r#"hl.env("FOO", "bar", false)"#), "{g}");
}

#[tokio::test]
async fn explain_env_user_lua_and_file_error() {
    let mut r = rig();
    put(
        &r.desktop.paths().user_lua,
        "-- mine\nhl.env(\"FOO\", \"mine\")\n",
    );
    let two = r#"{"vars":[{"name":"FOO","value":"a"},{"name":"BAR","value":"b"}]}"#;
    write(&mut r, "env", two).await.unwrap();
    let rows = explain(&r, "env").await;
    assert_eq!(rows["vars.FOO"]["source"], "user_lua");
    assert_eq!(rows["vars.FOO"]["detail"]["line"], 2);
    assert_eq!(rows["vars.BAR"]["source"], "uq");
    assert!(rows.get("env").is_none());

    // A folder where the file goes: the write can't land.
    let path = files::environment_d_path(&r.desktop.paths().startup);
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir_all(&path).unwrap();
    write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"c"}]}"#)
        .await
        .unwrap();
    let rows = explain(&r, "env").await;
    assert_eq!(rows["env"]["source"], "error");
    assert_eq!(rows["vars.FOO"]["source"], "error");
}

#[tokio::test]
async fn rejected_env_keeps_previous_config() {
    let mut r = rig();
    write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"a"}]}"#)
        .await
        .unwrap();
    let before = generated(&r);
    *r.comp.verify_error.lock().unwrap() = Some("bad env".into());
    let err = write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"b"}]}"#)
        .await
        .unwrap_err();
    assert!(matches!(err, DesktopError::ConfigInvalid(_)), "{err:?}");
    assert_eq!(generated(&r), before);
    assert!(env_file(&r).unwrap().contains("FOO='a'"));
    let rows = explain(&r, "env").await;
    assert_eq!(rows["env"]["source"], "error");
    assert_eq!(rows["vars.FOO"]["source"], "error");
}

#[tokio::test]
async fn env_not_in_settings_changed() {
    let mut r = rig();
    write(
        &mut r,
        "env",
        r#"{"vars":[{"name":"FOO_SECRET","value":"x"}]}"#,
    )
    .await
    .unwrap();
    let events: Vec<Event> = std::iter::from_fn(|| r.events.try_recv().ok()).collect();
    assert!(events.iter().any(
        |e| matches!(e, Event::SectionChanged { domain, json } if domain == "env" && json.contains("FOO_SECRET"))
    ));
    let settings: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            Event::SettingsChanged(s) => Some(serde_json::to_string(s).unwrap()),
            _ => None,
        })
        .collect();
    assert!(!settings.is_empty());
    assert!(settings.iter().all(|s| !s.contains("FOO_SECRET")));
}

/// The 60 s check retries a failed session variables file (task 7 L2).
#[tokio::test]
async fn tick_retries_a_failed_env_file() {
    let mut r = rig();
    let env_d = r.desktop.paths().startup.config_home.join("environment.d");
    // A file where the folder should be: the write fails.
    put(&env_d, "not a folder");
    write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"1"}]}"#)
        .await
        .unwrap();
    assert!(r.desktop.files.env_file.is_some());
    r.desktop.refresh_files();
    assert!(r.desktop.files.env_file.is_some(), "still failing");
    std::fs::remove_file(&env_d).unwrap();
    r.desktop.refresh_files();
    assert_eq!(r.desktop.files.env_file, None);
    assert!(env_file(&r).unwrap().contains("FOO"));
}

/// The 60 s check rebuilds the hidden copies, so a change in the folders alone is picked up
/// (task 7 L3): the user's own file that shadowed an entry that is off goes away.
#[tokio::test]
async fn tick_rebuilds_hidden_copies_after_a_folder_change() {
    let mut r = rig();
    let applet = "[Desktop Entry]\nType=Application\nName=Applet\nExec=nm-applet\n";
    put(&root(&r).join("xdg/autostart/nm-applet.desktop"), applet);
    let own = r
        .desktop
        .paths()
        .startup
        .user_autostart()
        .join("nm-applet.desktop");
    put(&own, MINE_APPLET);
    write(
        &mut r,
        "autostart",
        r#"{"files":[{"id":"nm-applet.desktop","enabled":false,"delay":null}],"added":[]}"#,
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read_to_string(&own).unwrap(), MINE_APPLET);
    std::fs::remove_file(&own).unwrap();
    r.desktop.refresh_files();
    let t = std::fs::read_to_string(&own).unwrap();
    assert!(
        t.contains("Hidden=true") && t.contains("X-uQ-Autostart=true"),
        "{t}"
    );
}

/// When nothing changed, the 60 s check rewrites nothing.
#[tokio::test]
async fn tick_rewrites_nothing_when_nothing_changed() {
    let mut r = rig();
    let applet = "[Desktop Entry]\nType=Application\nName=Applet\nExec=nm-applet\n";
    put(&root(&r).join("xdg/autostart/nm-applet.desktop"), applet);
    write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"1"}]}"#)
        .await
        .unwrap();
    write(
        &mut r,
        "autostart",
        r#"{"files":[{"id":"nm-applet.desktop","enabled":false,"delay":null}],"added":[]}"#,
    )
    .await
    .unwrap();
    let env = files::environment_d_path(&r.desktop.paths().startup);
    let copy = r
        .desktop
        .paths()
        .startup
        .user_autostart()
        .join("nm-applet.desktop");
    let before = (inode(&env), inode(&copy));
    let mtimes = (
        std::fs::metadata(&env).unwrap().modified().unwrap(),
        std::fs::metadata(&copy).unwrap().modified().unwrap(),
    );
    r.desktop.refresh_files();
    r.desktop.refresh_files();
    assert_eq!((inode(&env), inode(&copy)), before);
    assert_eq!(
        (
            std::fs::metadata(&env).unwrap().modified().unwrap(),
            std::fs::metadata(&copy).unwrap().modified().unwrap(),
        ),
        mtimes
    );
    assert_eq!(r.desktop.files.env_file, None);
    assert_eq!(r.desktop.files.autostart, None);
}

#[test]
fn a_lasting_failure_is_logged_once() {
    let m = Some("uQ couldn't write it".to_string());
    assert_eq!(new_failure(&None, &m), m.as_deref());
    assert_eq!(new_failure(&m, &m), None);
    let other = Some("another".to_string());
    assert_eq!(new_failure(&m, &other), other.as_deref());
    assert_eq!(new_failure(&m, &None), None);
}
