use std::sync::{Arc, Mutex};

use super::*;

const LIST: &str = "\
backup.service enabled enabled
syncthing.service disabled enabled
dbus.service static -
getty@.service static -
blocked.service masked enabled
uqd.service enabled enabled
";

const SHOW: &str = "\
Id=backup.service
Description=Nightly backup
ActiveState=failed
SubState=failed

Id=syncthing.service
Description=Syncthing - Open Source Continuous File Synchronization
ActiveState=active
SubState=running

Id=blocked.service
Description=blocked.service
ActiveState=inactive
SubState=dead

Id=uqd.service
Description=uQ desktop service
ActiveState=active
SubState=running
";

type Calls = Arc<Mutex<Vec<String>>>;

fn fake(fail_with: Option<&'static str>) -> (ServicesState, Calls) {
    let calls: Calls = Arc::default();
    let seen = calls.clone();
    let run: Runner = Arc::new(move |program: &str, args: &[String]| {
        let line = format!("{program} {}", args.join(" "));
        seen.lock().unwrap().push(line.clone());
        let out = if line.contains("list-unit-files") {
            LIST
        } else if line.contains(" show ") {
            SHOW
        } else if program == "journalctl" {
            "-- Boot 1 --\nNov 26 10:00:00 host backup[1]: disk full\n"
        } else {
            ""
        };
        Ok(match fail_with {
            Some(err) if line.contains("enable") || line.contains("start") => Ran {
                ok: false,
                out: String::new(),
                err: err.into(),
            },
            _ => Ran {
                ok: true,
                out: out.into(),
                err: String::new(),
            },
        })
    });
    (ServicesState::with(run), calls)
}

#[tokio::test]
async fn lists_switchable_and_blocked_services_with_uqs_last() {
    let (s, _) = fake(None);
    let snap = s.snapshot().await.unwrap();
    let names: Vec<&str> = snap["services"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["backup", "blocked", "syncthing", "uqd"]);
    assert_eq!(snap["hidden"], 1, "dbus is static, getty@ is a template");
    let backup = &snap["services"][0];
    assert_eq!(backup["on"], true);
    assert_eq!(backup["state"], "failed");
    assert_eq!(backup["description"], "Nightly backup");
    assert_eq!(snap["services"][1]["blocked"], true);
    assert_eq!(
        snap["services"][1]["description"], "",
        "a name repeated isn't a description"
    );
    assert_eq!(snap["services"][3]["locked"], true);
    assert_eq!(snap["services"][3]["switchable"], false);
}

#[tokio::test]
async fn show_all_adds_static_services() {
    let (s, _) = fake(None);
    s.apply(Op::SetShowAll { on: true }).await.unwrap();
    let snap = s.snapshot().await.unwrap();
    assert_eq!(snap["services"].as_array().unwrap().len(), 5);
    assert_eq!(snap["hidden"], 0);
    let dbus = snap["services"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "dbus")
        .unwrap();
    assert_eq!(dbus["switchable"], false);
}

#[tokio::test]
async fn enable_start_and_log_run_the_right_commands() {
    let (s, calls) = fake(None);
    s.apply(Op::SetEnabled {
        unit: "syncthing.service".into(),
        on: true,
    })
    .await
    .unwrap();
    s.apply(Op::Act {
        unit: "backup.service".into(),
        action: "restart".into(),
    })
    .await
    .unwrap();
    s.apply(Op::ShowLog {
        unit: "backup.service".into(),
    })
    .await
    .unwrap();
    let snap = s.snapshot().await.unwrap();
    assert_eq!(
        snap["log"]["lines"],
        json!(["Nov 26 10:00:00 host backup[1]: disk full"])
    );
    let calls = calls.lock().unwrap();
    assert!(calls.contains(&"systemctl --user --no-pager enable -- syncthing.service".to_string()));
    assert!(
        calls.contains(
            &"systemctl --user --no-pager --no-block restart -- backup.service".to_string()
        )
    );
    assert!(
        calls
            .iter()
            .any(|c| c.starts_with("journalctl --user -u backup.service -n 40"))
    );
}

#[tokio::test]
async fn refuses_uqs_own_odd_names_and_unknown_actions() {
    let (s, calls) = fake(None);
    for op in [
        Op::SetEnabled {
            unit: "uqd.service".into(),
            on: false,
        },
        Op::Act {
            unit: "-x.service".into(),
            action: "start".into(),
        },
        Op::Act {
            unit: "a;b.service".into(),
            action: "start".into(),
        },
        Op::Act {
            unit: "backup.service".into(),
            action: "mask".into(),
        },
        Op::ShowLog {
            unit: "backup".into(),
        },
    ] {
        assert!(s.apply(op).await.is_err());
    }
    assert!(calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn the_managers_refusal_is_shown() {
    let (s, _) = fake(Some("Unit file backup.service does not exist."));
    let e = s
        .apply(Op::Act {
            unit: "backup.service".into(),
            action: "start".into(),
        })
        .await
        .unwrap_err();
    assert!(e.to_string().contains("does not exist"));
}

#[tokio::test]
async fn a_missing_service_manager_reads_as_a_sentence() {
    let run: Runner = Arc::new(|_: &str, _: &[String]| Err(io::ErrorKind::NotFound.into()));
    let snap = ServicesState::with(run).snapshot().await.unwrap();
    assert_eq!(snap["error"], "systemctl isn't installed");
    assert_eq!(snap["services"], json!([]));
}

#[test]
fn status_words_and_settling() {
    assert_eq!(parse::status("active", "exited").1, "Ran and finished");
    assert_eq!(parse::status("activating", "start").0, "starting");
    assert!(parse::settling(&[json!({"state": "stopping"})]));
    assert!(!parse::settling(&[json!({"state": "running"})]));
}
