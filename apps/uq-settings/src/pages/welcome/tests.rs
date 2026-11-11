//! Welcome against the fake uqd (and a fake fprintd on a private "system" bus).

use std::sync::{Arc, Mutex};

use serde_json::Value;
use uq_core::state::State;
use uq_core::welcome::{CURRENT_WELCOME_VERSION, Welcome};

use super::fingerprint::fake::{Calls, Device, serve};
use super::*;
use crate::pages::fake_system::SysRig;
use crate::pages::fake_uqd::rig;
use crate::pages::runner::run;

static LAUNCHED: Mutex<Vec<Vec<String>>> = Mutex::new(Vec::new());

fn record(argv: &[&str]) -> std::io::Result<()> {
    LAUNCHED
        .lock()
        .unwrap()
        .push(argv.iter().map(|s| s.to_string()).collect());
    Ok(())
}

struct T {
    _dir: tempfile::TempDir,
    _sys: SysRig,
    places: PathBuf,
}

/// Places under a temporary root; no fprintd unless `reader` is given.
async fn setup(reader: Option<&[&str]>) -> (T, Places, SystemBus) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let mut sys = SysRig::new();
    if let Some(fingers) = reader {
        let conn = sys.own("org.example.Unused").await;
        serve(
            &conn,
            Device {
                enrolled: Arc::new(Mutex::new(fingers.iter().map(|s| s.to_string()).collect())),
                script: vec![("enroll-completed", true)],
                calls: Calls::default(),
            },
        )
        .await;
    }
    let places = Places {
        state_home: root.join("state"),
        roots: Roots {
            widgets: root.join("uq/shell/widgets"),
            pages: root.join("uq/settings/pages"),
            providers: vec![root.join("uq/launcher")],
            actions: root.join("uq/launcher/actions"),
            integrations: root.join("uq/integrations"),
            bin_dir: root.join("bin"),
        },
        bin_dir: root.join("bin"),
    };
    let system = sys.system();
    (
        T {
            _dir: dir,
            _sys: sys,
            places: root,
        },
        places,
        system,
    )
}

fn ids(state: &Value) -> Vec<String> {
    state["flow"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn start_builds_the_steps_and_marks_welcome_done() {
    let r = rig(State::default()).await;
    let (_t, places, system) = setup(None).await;
    let w = WelcomeState::with(r.client.clone(), system, places, record);

    let before = run(&w, Op::Refresh).await;
    assert_eq!(before.state["flow"], Value::Null, "no steps before Start");
    assert!(r.state.lock().unwrap().welcome.pending());

    let out = run(&w, Op::Start { full: false }).await;
    assert_eq!(out.error, None);
    // The fake's fixture has two screens: Displays shows; no reader: no Fingerprint.
    assert_eq!(out.state["monitors"], 2);
    assert_eq!(
        ids(&out.state),
        [
            "hello",
            "look",
            "wallpaper",
            "layouts",
            "displays",
            "apps",
            "done"
        ]
    );
    assert_eq!(out.state["marked"], true);
    assert_eq!(out.state["status"]["pending"], false);
    assert_eq!(
        r.state.lock().unwrap().welcome,
        Welcome {
            done: true,
            version: CURRENT_WELCOME_VERSION
        },
        "shown counts as done: closing at any step needs nothing more"
    );
    assert_eq!(out.state["keybinds"]["first"].as_array().unwrap().len(), 3);
    assert_eq!(out.state["setup_problem"], false);

    // Start again (uqd came back, say) keeps the flow where it is.
    run(&w, Op::Next).await;
    let again = run(&w, Op::Start { full: false }).await;
    assert_eq!(again.state["flow"]["current"], "look");
}

#[tokio::test]
async fn navigation_next_back_skip() {
    let r = rig(State::default()).await;
    let (_t, places, system) = setup(None).await;
    let w = WelcomeState::with(r.client.clone(), system, places, record);
    // Before Start, moving does nothing.
    assert_eq!(run(&w, Op::Next).await.state["flow"], Value::Null);
    run(&w, Op::Start { full: false }).await;
    assert_eq!(run(&w, Op::Back).await.state["flow"]["current"], "hello");
    assert_eq!(run(&w, Op::Next).await.state["flow"]["current"], "look");
    let s = run(&w, Op::Skip).await.state;
    assert_eq!(s["flow"]["current"], "wallpaper");
    assert_eq!(s["flow"]["skipped"], serde_json::json!(["look"]));
    assert_eq!(run(&w, Op::Back).await.state["flow"]["current"], "look");
    for _ in 0..10 {
        run(&w, Op::Skip).await;
    }
    let s = run(&w, Op::Refresh).await.state;
    assert_eq!(s["flow"]["current"], "done");
    assert_eq!(s["flow"]["last"], true);
}

#[tokio::test]
async fn a_finished_welcome_shows_only_hello_and_done_unless_full() {
    let r = rig(State {
        welcome: Welcome {
            done: true,
            version: CURRENT_WELCOME_VERSION,
        },
        ..State::default()
    })
    .await;
    let (_t, places, system) = setup(None).await;
    let w = WelcomeState::with(r.client.clone(), system, places, record);
    let s = run(&w, Op::Start { full: false }).await.state;
    assert_eq!(ids(&s), ["hello", "done"]);

    let (_t, places, system) = setup(None).await;
    let w = WelcomeState::with(r.client.clone(), system, places, record);
    let s = run(&w, Op::Start { full: true }).await.state;
    assert_eq!(
        ids(&s).len(),
        7,
        "Settings → About → Show Welcome shows it all"
    );
}

#[tokio::test]
async fn uqd_down_shows_every_step_and_marks_later() {
    let r = rig(State::default()).await;
    let (_t, places, system) = setup(None).await;
    // A connection to a bus without uqd.
    let empty = uq_core::test_bus::private_bus();
    let conn = uq_core::test_bus::connect(&empty).await;
    let w = WelcomeState::with(conn, system, places, record);
    let out = run(&w, Op::Start { full: false }).await;
    assert_eq!(out.error, None, "uqd being down is not an error");
    assert_eq!(out.state["marked"], false);
    assert_eq!(out.state["monitors"], 1);
    assert_eq!(ids(&out.state).len(), 6);
    drop(r);
}

#[tokio::test]
async fn reader_without_fingers_adds_the_fingerprint_step_and_enrols() {
    let r = rig(State::default()).await;
    r.displays.live_outputs.lock().unwrap().truncate(1);
    let (_t, places, system) = setup(Some(&[])).await;
    let w = WelcomeState::with(r.client.clone(), system, places, record);
    let s = run(&w, Op::Start { full: false }).await.state;
    assert_eq!(
        ids(&s),
        [
            "hello",
            "look",
            "wallpaper",
            "layouts",
            "apps",
            "fingerprint",
            "done"
        ]
    );
    assert_eq!(
        s["reader"],
        serde_json::json!({"present": true, "enrolled": false})
    );
    assert_eq!(s["enroll"]["phase"], "idle");

    let changed = w.enrolment.changed.clone();
    run(&w, Op::Enroll).await;
    let mut phase = String::new();
    for _ in 0..50 {
        let _ =
            tokio::time::timeout(std::time::Duration::from_millis(100), changed.notified()).await;
        phase = run(&w, Op::Refresh).await.state["enroll"]["phase"]
            .as_str()
            .unwrap()
            .to_string();
        if phase == "done" {
            break;
        }
    }
    assert_eq!(phase, "done");
}

#[tokio::test]
async fn an_enrolled_finger_skips_the_fingerprint_step() {
    let r = rig(State::default()).await;
    let (_t, places, system) = setup(Some(&["left-thumb"])).await;
    let w = WelcomeState::with(r.client.clone(), system, places, record);
    let s = run(&w, Op::Start { full: false }).await.state;
    assert!(!ids(&s).contains(&"fingerprint".to_string()));
}

#[tokio::test]
async fn apps_bindle_and_the_setup_problem_card() {
    let r = rig(State::default()).await;
    let (t, places, system) = setup(None).await;
    std::fs::create_dir_all(t.places.join("bin")).unwrap();
    std::fs::write(t.places.join("bin/scdrctl"), "").unwrap();
    let log = t.places.join("state/uq/session.log");
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    std::fs::write(
        &log,
        "2026-11-11 08:00:00 uqctl setup exited 1; starting the session anyway\n",
    )
    .unwrap();
    let w = WelcomeState::with(r.client.clone(), system, places, record);
    let out = run(&w, Op::Start { full: false }).await;
    let s = &out.state;
    assert_eq!(s["setup_problem"], true);
    assert_eq!(s["session_log"], log.display().to_string());
    assert_eq!(s["apps"][0]["id"], "scdr");
    assert_eq!(s["apps"][0]["installed"], true);
    assert_eq!(s["bindle"], false);

    let out = run(&w, Op::OpenBindle).await;
    assert_eq!(out.error.as_deref(), Some("bindle isn't installed."));
    std::fs::write(t.places.join("bin/bindle"), "").unwrap();
    let out = run(&w, Op::OpenBindle).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["bindle"], true);
    assert!(LAUNCHED.lock().unwrap().contains(&vec![
        "uqctl".into(),
        "launch".into(),
        "bindle".into()
    ]));
}

#[test]
fn keybinds_match_base_lua() {
    let base = include_str!("../../../../../config/hypr/base.lua");
    for k in KEYBINDS {
        assert!(base.contains(k.lua), "{} is not bound as {}", k.keys, k.lua);
    }
    assert_eq!(
        KEYBINDS[..3].iter().map(|k| k.keys).collect::<Vec<_>>(),
        ["Super + Space", "Tap Super", "Super + A"]
    );
}
