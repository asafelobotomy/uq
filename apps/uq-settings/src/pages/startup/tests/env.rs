//! Environment variables and switches.

use super::*;

#[tokio::test]
async fn set_var_blocked_name_shows_message() {
    let (r, page, _d) = rig().await;
    assert_eq!(run(&page, set_var(None, "FOO", "1")).await.error, None);
    let before = section(&r, "env");
    let journalled = r.history.origins().len();

    let out = run(&page, set_var(None, "XDG_SESSION_TYPE", "x11")).await;
    let error = out.error.unwrap();
    assert!(error.contains("uQ needs this one"), "{error}");
    assert!(!error.contains("env.vars"), "{error}");
    assert_eq!(section(&r, "env"), before);
    assert_eq!(r.history.origins().len(), journalled);
    assert_eq!(out.state["vars"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn set_var_warns_ld_preload() {
    let (_r, page, _d) = rig().await;
    run(&page, set_var(None, "FOO", "1")).await;
    let out = run(&page, set_var(None, "LD_PRELOAD", "/lib/x.so")).await;
    assert_eq!(out.error, None);
    let vars = out.state["vars"].as_array().unwrap();
    assert_eq!(vars[0]["warn"], false);
    assert_eq!(vars[1]["name"], "LD_PRELOAD");
    assert_eq!(vars[1]["warn"], true);

    // Editing in place keeps its place; a rename too.
    let out = run(&page, set_var(Some(0), "BAR", "2")).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["vars"][0]["name"], "BAR");
    assert_eq!(out.state["vars"][0]["value"], "2");
    let out = run(
        &page,
        Op::MoveVar {
            name: "LD_PRELOAD".into(),
            to: 0,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["vars"][0]["name"], "LD_PRELOAD");
    assert!(run(&page, set_var(Some(7), "X", "1")).await.error.is_some());
}

#[tokio::test]
async fn var_with_dollar_refused() {
    let (r, page, _d) = rig().await;
    let out = run(&page, set_var(None, "FOO", "$HOME/bin")).await;
    let error = out.error.unwrap();
    assert!(error.contains("$"), "{error}");
    assert!(!error.contains("env."), "{error}");
    assert_eq!(section(&r, "env"), json!({"vars": []}));
}

#[tokio::test]
async fn notice_after_env_write_and_removal() {
    let (_r, page, _d) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["notice"], json!({"seq": 0, "text": ""}));

    let out = run(&page, set_var(None, "FOO", "1")).await;
    assert_eq!(out.state["notice"]["seq"], 1);
    assert_eq!(out.state["notice"]["text"], APPLY_NOTE);

    let out = run(&page, Op::RemoveVar { name: "FOO".into() }).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["vars"], json!([]));
    assert_eq!(out.state["notice"]["seq"], 2);
    let text = out.state["notice"]["text"].as_str().unwrap();
    assert!(text.starts_with(APPLY_NOTE), "{text}");
    assert!(
        text.ends_with("Removed variables stay for apps started before you log out."),
        "{text}"
    );

    // A refused write leaves the notice alone; a startup write doesn't set it.
    let out = run(&page, set_var(None, "HOME", "/x")).await;
    assert_eq!(out.state["notice"]["seq"], 2);
    let out = run(&page, on("nm-applet.desktop", false)).await;
    assert_eq!(out.state["notice"]["seq"], 2);
}

#[tokio::test]
async fn vars_show_user_lua_override() {
    let (r, page, _d) = rig().await;
    run(&page, set_var(None, "FOO", "1")).await;
    run(&page, set_var(None, "QT_QPA_PLATFORM", "xcb")).await;
    r.state.lock().unwrap().compat.qt_wayland = true;
    r.explain.set(
        "env.vars.FOO",
        json!({"source": "user_lua", "detail": {"line": 4, "path": "user.lua"}}),
    );
    let out = run(&page, Op::Refresh).await;
    let foo = &out.state["vars"][0];
    assert_eq!(foo["origin"]["source"], "user_lua");
    assert_eq!(foo["origin"]["detail"]["line"], 4);
    assert_eq!(foo["line"], 4);
    assert_eq!(foo["replaced_by_label"], Value::Null);
    let qt = &out.state["vars"][1];
    assert_eq!(qt["origin"]["source"], "uq");
    assert_eq!(qt["line"], Value::Null);
    assert_eq!(qt["replaced_by_label"], "Qt apps prefer Wayland");
}

#[tokio::test]
async fn switches_listed_read_only() {
    let mut st = State::default();
    st.compat.electron_wayland = true;
    let (_r, page, _d) = rig_with(st).await;
    let out = run(&page, Op::Refresh).await;
    let switches = out.state["switches"].as_array().unwrap();
    assert_eq!(switches.len(), 5);
    assert_eq!(
        switches[1],
        json!({"key": "electron_wayland", "label": "Run Electron apps on Wayland", "on": true})
    );
    assert_eq!(switches[0]["on"], false);
    assert_eq!(
        out.state["limits"],
        json!({"vars": 100, "value_bytes": 4096, "added": 50, "delay_max": 600, "stagger_max": 30})
    );
}

#[tokio::test]
async fn list_autostart_failure_still_shows_vars() {
    let (r, page, _d) = rig().await;
    run(&page, set_var(None, "FOO", "1")).await;
    r.autostart.fail();
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["apps"], json!([]));
    assert!(
        out.state["apps_error"]
            .as_str()
            .is_some_and(|e| !e.is_empty())
    );
    assert_eq!(out.state["vars"][0]["name"], "FOO");
}

#[tokio::test]
async fn rejected_env_write_shows_section_error() {
    let (r, page, _d) = rig().await;
    *r.verify_error.lock().unwrap() = Some("bad env".into());
    let out = run(&page, set_var(None, "FOO", "1")).await;
    assert!(out.error.as_deref().is_some_and(|e| e.contains("bad env")));
    assert_eq!(out.state["section"]["env"]["source"], "error");
    assert_eq!(out.state["section"]["autostart"], Value::Null);
    r.files.lock().unwrap().autostart = Some("can't write the copy".into());
    let out = run(&page, Op::Refresh).await;
    assert_eq!(
        out.state["section"]["autostart"],
        json!({"source": "error", "message": "can't write the copy"})
    );
}
