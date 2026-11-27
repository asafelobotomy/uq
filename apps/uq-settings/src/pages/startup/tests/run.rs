//! Starting hidden, staggering, retry, run now and what the rows say about a run.

use super::*;

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
async fn start_hidden_round_trips_and_clears() {
    let (r, page, _d) = rig().await;
    let hide = |id: &str, on| Op::SetMinimize { id: id.into(), on };
    let out = run(&page, hide("nm-applet.desktop", true)).await;
    assert_eq!(out.error, None);
    assert_eq!(
        section(&r, "autostart")["files"],
        json!([{"id": "nm-applet.desktop", "enabled": null, "delay": null, "minimize": true}])
    );
    assert_eq!(app(&out.state, "nm-applet.desktop")["minimize"], true);

    // Off again: a choice that sets nothing goes.
    let out = run(&page, hide("nm-applet.desktop", false)).await;
    assert_eq!(out.error, None);
    assert_eq!(section(&r, "autostart")["files"], json!([]));
    assert_eq!(app(&out.state, "nm-applet.desktop")["minimize"], false);

    // It is kept beside the other choices.
    assert_eq!(run(&page, on("notes.desktop", false)).await.error, None);
    assert_eq!(run(&page, hide("notes.desktop", true)).await.error, None);
    assert_eq!(run(&page, hide("notes.desktop", false)).await.error, None);
    assert_eq!(
        section(&r, "autostart")["files"],
        json!([{"id": "notes.desktop", "enabled": false, "delay": null}])
    );
}

#[tokio::test]
async fn added_entry_can_start_hidden() {
    let (r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::AddCommand {
            name: "Sync".into(),
            command: "syncthing --no-browser".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let id = section(&r, "autostart")["added"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let out = run(
        &page,
        Op::SetMinimize {
            id: id.clone(),
            on: true,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(section(&r, "autostart")["added"][0]["minimize"], true);
    assert_eq!(app(&out.state, &id)["minimize"], true);
}

#[test]
fn status_names_how_a_hidden_app_was_hidden() {
    use super::super::rows::status_text;
    let row = |h: Value| json!({"status": "running", "hidden": h});
    assert_eq!(
        status_text(&row(json!("tray")), false),
        "Running, hidden in the tray"
    );
    assert_eq!(
        status_text(&row(json!("minimised")), false),
        "Running, minimised"
    );
    assert_eq!(status_text(&row(Value::Null), false), "Running");
}

#[tokio::test]
async fn stagger_and_retry_round_trip_and_stagger_is_limited() {
    let (r, page, _d) = rig().await;
    let out = run(&page, Op::SetStagger { secs: 4 }).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["stagger"], 4);
    let out = run(&page, Op::SetRetry { on: true }).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["retry"], true);
    let section = section(&r, "autostart");
    assert_eq!(
        (section["stagger"].clone(), section["retry"].clone()),
        (json!(4), json!(true))
    );
    let out = run(&page, Op::SetStagger { secs: 31 }).await;
    let error = out.error.unwrap();
    assert!(
        error.contains("30") && !error.contains("autostart."),
        "{error}"
    );
    assert_eq!(
        run(&page, Op::SetStagger { secs: 0 }).await.state["stagger"],
        0
    );
    assert_eq!(out.state["limits"]["stagger_max"], 30);
}

#[tokio::test]
async fn run_now_and_stop_now_go_to_uqd() {
    let (r, page, _d) = rig().await;
    let id = "nm-applet.desktop";
    assert_eq!(run(&page, Op::RunNow { id: id.into() }).await.error, None);
    assert_eq!(run(&page, Op::StopNow { id: id.into() }).await.error, None);
    assert_eq!(
        r.autostart.actions(),
        vec![format!("run:{id}"), format!("stop:{id}")]
    );
}

#[test]
fn timing_says_when_an_app_started() {
    use super::super::rows::app_row;
    let row = |extra: Value| {
        let mut r = json!({"id": "a.desktop", "status": "running", "source": "system"});
        for (k, v) in extra.as_object().unwrap() {
            r[k] = v.clone();
        }
        app_row(&r, &json!({}), &json!({}))["timing_text"].clone()
    };
    assert_eq!(row(json!({})), "");
    assert_eq!(row(json!({"started_after": 8})), "Started 8 s after login");
    assert_eq!(
        row(json!({"started_after": 0})),
        "Started as soon as you logged in"
    );
    assert_eq!(
        row(json!({"started_after": 12, "retried": true})),
        "Started 12 s after login, again after it failed"
    );
    // A staggered app that is still waiting says its real wait.
    let waiting = json!({"status": "waiting", "delay": 0, "start_in": 6});
    assert_eq!(
        super::super::rows::status_text(&waiting, false),
        "Waiting 6 s"
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
async fn shell_ready_is_counted() {
    let (r, page, _d) = rig().await;
    let proxy = DesktopProxy::new(&page.conn).await.unwrap();
    proxy.shell_ready().await.unwrap();
    assert_eq!(r.autostart.shell_ready_calls(), 1);
}
