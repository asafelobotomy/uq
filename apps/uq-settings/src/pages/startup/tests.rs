//! The Startup page against the fake uqd.

use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::{Outcome, run};
use uq_core::state::State;
use uq_core::test_bus::{connect, private_bus};

/// A system entry, a skipped one, and the person's own one, as uqd lists them.
fn found_rows() -> Vec<Value> {
    vec![
        json!({"id": "nm-applet.desktop", "name": "Network", "icon": "nm-applet",
               "source": "system", "enabled": true, "can_toggle": true, "delay": 0,
               "status": "running"}),
        json!({"id": "tracker.desktop", "name": "Tracker", "icon": null,
               "source": "system", "enabled": false, "can_toggle": false, "delay": 0,
               "status": "skipped", "reason": "Turned off by the system"}),
        json!({"id": "notes.desktop", "name": "Notes", "icon": null,
               "source": "yours", "enabled": true, "can_toggle": true, "delay": 5,
               "status": "exited"}),
    ]
}

fn app_dir() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("syncthing-start.desktop"),
        "[Desktop Entry]\nType=Application\nName=Syncthing\nIcon=syncthing\nExec=syncthing\n",
    )
    .unwrap();
    let long = "L".repeat(150);
    std::fs::write(
        d.path().join("long.desktop"),
        format!("[Desktop Entry]\nType=Application\nName={long}\nExec=long\n"),
    )
    .unwrap();
    d
}

async fn rig_with(st: State) -> (Rig, StartupState, tempfile::TempDir) {
    let r = fake_uqd::rig(st).await;
    r.autostart.set(found_rows());
    let dir = app_dir();
    let page = StartupState::with_dirs(r.client.clone(), vec![dir.path().to_path_buf()]);
    (r, page, dir)
}

async fn rig() -> (Rig, StartupState, tempfile::TempDir) {
    rig_with(State::default()).await
}

fn app<'a>(state: &'a Value, id: &str) -> &'a Value {
    state["apps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
        .unwrap_or_else(|| panic!("no app {id} in {state}"))
}

fn on(id: &str, on: bool) -> Op {
    Op::SetEnabled { id: id.into(), on }
}

fn set_var(index: Option<usize>, name: &str, value: &str) -> Op {
    Op::SetVar {
        index,
        name: name.into(),
        value: value.into(),
    }
}

fn section(r: &Rig, domain: &str) -> Value {
    r.state.lock().unwrap().section(domain).unwrap()
}

fn added_ids(r: &Rig) -> Vec<String> {
    let st = r.state.lock().unwrap();
    st.autostart
        .added
        .iter()
        .map(|a| a.id().to_string())
        .collect()
}

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
async fn toggle_hidden_entry_refused_plainly() {
    let (r, page, _d) = rig().await;
    let out = run(&page, on("tracker.desktop", true)).await;
    let error = out.error.unwrap();
    assert!(
        !error.contains("autostart") && !error.contains('['),
        "{error}"
    );
    assert!(error.contains("can't be turned on or off"), "{error}");
    assert_eq!(section(&r, "autostart")["files"], json!([]));
    assert!(r.history.origins().is_empty());
    let row = app(&out.state, "tracker.desktop");
    assert_eq!(row["can_toggle"], false);
    assert_eq!(row["status_text"], "Turned off by the system");
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
        json!({"vars": 100, "value_bytes": 4096, "added": 50, "delay_max": 600})
    );
}

#[tokio::test]
async fn failed_entry_shows_exit_and_log() {
    let (r, page, _d) = rig().await;
    r.autostart.set(vec![
        json!({"id": "bad.desktop", "name": "Bad", "icon": null, "source": "system",
               "enabled": true, "can_toggle": true, "delay": 0, "status": "failed",
               "exit": 3, "log": ["no config", "giving up"]}),
        json!({"id": "killed.desktop", "name": "Killed", "icon": null, "source": "system",
               "enabled": true, "can_toggle": true, "delay": 0, "status": "failed",
               "signal": 9, "log": []}),
        json!({"id": "nospawn.desktop", "name": "No spawn", "icon": null, "source": "system",
               "enabled": true, "can_toggle": true, "delay": 0, "status": "failed",
               "reason": "uQ couldn't start it"}),
        json!({"id": "slow.desktop", "name": "Slow", "icon": null, "source": "system",
               "enabled": true, "can_toggle": true, "delay": 10, "status": "waiting"}),
        json!({"id": "old.desktop", "name": "Old", "icon": null, "source": "system",
               "enabled": true, "can_toggle": true, "delay": 0, "status": "unknown_exit",
               "reason": "Started earlier; finished"}),
        json!({"id": "dev.desktop", "name": "Dev", "icon": null, "source": "system",
               "enabled": true, "can_toggle": true, "delay": 0, "status": "not_started",
               "reason": "Not started in a test session"}),
        json!({"id": "ok.desktop", "name": "Ok", "icon": null, "source": "system",
               "enabled": true, "can_toggle": true, "delay": 0, "status": "exited"}),
    ]);
    let out = run(&page, Op::Refresh).await;
    let bad = app(&out.state, "bad.desktop");
    assert_eq!(bad["status_text"], "Failed (exit 3)");
    assert_eq!(bad["exit"], 3);
    assert_eq!(bad["log"], json!(["no config", "giving up"]));
    let killed = app(&out.state, "killed.desktop");
    assert_eq!(killed["status_text"], "Failed (stopped by signal 9)");
    assert_eq!(killed["exit"], Value::Null);
    let texts: Vec<&str> = [
        "nospawn.desktop",
        "slow.desktop",
        "old.desktop",
        "dev.desktop",
        "ok.desktop",
    ]
    .iter()
    .map(|id| app(&out.state, id)["status_text"].as_str().unwrap())
    .collect();
    assert_eq!(
        texts,
        [
            "Failed: uQ couldn't start it",
            "Waiting 10 s",
            "Started earlier; finished",
            "Not started in a test session",
            "Started, then finished"
        ]
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

#[tokio::test]
async fn shell_ready_is_counted() {
    let (r, page, _d) = rig().await;
    let proxy = DesktopProxy::new(&page.conn).await.unwrap();
    proxy.shell_ready().await.unwrap();
    assert_eq!(r.autostart.shell_ready_calls(), 1);
}
