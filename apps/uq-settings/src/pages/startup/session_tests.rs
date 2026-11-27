//! Startup › Session and "Start when" against the fake uqd.

use super::*;
use crate::pages::fake_uqd;
use crate::pages::runner::run;
use uq_core::state::State;

async fn page() -> (fake_uqd::Rig, StartupState) {
    let r = fake_uqd::rig(State::default()).await;
    r.autostart
        .set(vec![json!({"id": "nm-applet.desktop", "name": "Network",
        "source": "system", "enabled": true, "can_toggle": true, "delay": 0,
        "status": "running"})]);
    let page = StartupState::with_dirs(r.client.clone(), vec![]);
    (r, page)
}

fn section(r: &fake_uqd::Rig) -> Value {
    r.state.lock().unwrap().section("autostart").unwrap()
}

#[tokio::test]
async fn when_round_trips_and_clears() {
    let (r, page) = page().await;
    let id = "nm-applet.desktop".to_string();
    let out = run(
        &page,
        Op::SetWhen {
            id: id.clone(),
            when: "network".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(section(&r)["when"], json!({"nm-applet.desktop": "network"}));
    assert_eq!(out.state["apps"][0]["when"], "network");
    let out = run(
        &page,
        Op::SetWhen {
            id: id.clone(),
            when: "moon".into(),
        },
    )
    .await;
    assert!(out.error.is_some());
    run(
        &page,
        Op::SetWhen {
            id,
            when: String::new(),
        },
    )
    .await;
    assert!(section(&r).get("when").is_none(), "an empty set is dropped");
}

#[tokio::test]
async fn restore_switch_round_trips() {
    let (r, page) = page().await;
    assert_eq!(run(&page, Op::Refresh).await.state["restore"], false);
    let out = run(&page, Op::SetRestore { on: true }).await;
    assert_eq!(
        (out.error, out.state["restore"].clone()),
        (None, json!(true))
    );
    assert_eq!(section(&r)["restore"], true);
}

#[tokio::test]
async fn logout_commands_add_switch_and_remove() {
    let (r, page) = page().await;
    let add = Op::AddLogout {
        name: String::new(),
        command: r#"rsync -a "/home/me/My Notes" /mnt"#.into(),
    };
    let out = run(&page, add).await;
    assert_eq!(out.error, None);
    let row = out.state["logout"][0].clone();
    assert_eq!(row["name"], "rsync");
    assert_eq!(row["enabled"], true);
    assert!(row["command_text"].as_str().unwrap().contains("My Notes"));
    let id = row["id"].as_str().unwrap().to_string();
    let out = run(
        &page,
        Op::SetLogoutEnabled {
            id: id.clone(),
            on: false,
        },
    )
    .await;
    assert_eq!(out.state["logout"][0]["enabled"], false);
    let out = run(
        &page,
        Op::AddLogout {
            name: "x".into(),
            command: "  ".into(),
        },
    )
    .await;
    assert!(out.error.is_some(), "an empty command is refused");
    let out = run(&page, Op::RemoveLogout { id: id.clone() }).await;
    assert_eq!(out.state["logout"], json!([]));
    assert!(run(&page, Op::RemoveLogout { id }).await.error.is_some());
    assert!(section(&r).get("logout").is_none_or(|l| l == &json!([])));
}
