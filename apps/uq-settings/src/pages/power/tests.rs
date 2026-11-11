use std::time::Duration;

use super::*;
use crate::pages::fake_system::{FakeHelper, FakePower, SysRig};
use crate::pages::runner::run;

async fn setup(
    ppd: bool,
    battery: bool,
    helper: bool,
) -> (SysRig, FakePower, FakeHelper, PowerState) {
    let mut rig = SysRig::new();
    let power = FakePower::start(&mut rig, ppd, battery).await;
    let helper = if helper {
        FakeHelper::start(&mut rig, &[]).await
    } else {
        FakeHelper::files_only()
    };
    let page = PowerState::with(rig.system(), helper.root.path().to_path_buf());
    (rig, power, helper, page)
}

fn actions(lid: &str, ac: &str, key: &str) -> Op {
    Op::SetActions {
        lid: lid.into(),
        lid_on_ac: ac.into(),
        power_key: key.into(),
    }
}

#[tokio::test]
async fn reads_profiles_battery_and_actions() {
    let (_rig, _p, _h, page) = setup(true, true, true).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let s = out.state;
    assert_eq!(s["profiles"]["active"], "balanced");
    assert_eq!(
        s["profiles"]["list"],
        json!(["power-saver", "balanced", "performance"])
    );
    let b = &s["power"]["battery"];
    assert_eq!(b["percentage"], 81.0);
    assert_eq!(b["state"], "On battery");
    assert_eq!(b["time"], "About 3 h 20 min left");
    assert_eq!(b["health"], 93.0);
    assert_eq!(s["power"]["lid"], true);
    assert_eq!(s["actions"]["helper"], true);
    assert_eq!(
        s["actions"]["saved"],
        json!({"lid": "suspend", "lid_on_ac": "suspend", "power_key": "poweroff"})
    );
    assert_eq!(s["actions"]["pending"], false);
}

#[tokio::test]
async fn profile_round_trip_and_quiet_refusal() {
    let (_rig, p, _h, page) = setup(true, false, true).await;
    let out = run(
        &page,
        Op::SetProfile {
            name: "performance".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["profiles"]["active"], "performance");
    assert!(
        out.state["power"]["battery"].is_null(),
        "a desktop has none"
    );

    p.deny.set(true);
    let out = run(
        &page,
        Op::SetProfile {
            name: "power-saver".into(),
        },
    )
    .await;
    assert_eq!(out.error, None, "a refusal is quiet");
    assert_eq!(out.state["profiles"]["active"], "performance");
}

#[tokio::test]
async fn actions_go_through_the_helper_and_read_back_pending() {
    let (_rig, _p, h, page) = setup(true, true, true).await;
    let out = run(&page, actions("lock", "ignore", "suspend")).await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["actions"]["saved"],
        json!({"lid": "lock", "lid_on_ac": "ignore", "power_key": "suspend"})
    );
    assert_eq!(
        out.state["actions"]["pending"], true,
        "logind still runs the old ones"
    );
    assert_eq!(out.state["notice"]["text"], PENDING_NOTE);
    assert!(
        h.read("etc/systemd/logind.conf.d/50-uq.conf")
            .unwrap()
            .contains("HandleLidSwitch=lock")
    );
    // A new page reads the same values (nothing resets itself).
    let again = PowerState::with(_rig.system(), h.root.path().to_path_buf());
    let out = run(&again, Op::Refresh).await;
    assert_eq!(out.state["actions"]["saved"]["lid_on_ac"], "ignore");
}

#[tokio::test]
async fn refused_and_odd_actions() {
    let (_rig, _p, h, page) = setup(true, true, true).await;
    h.deny.set(true);
    let out = run(&page, actions("lock", "lock", "lock")).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["actions"]["saved"]["lid"], "suspend");
    let out = run(&page, actions("suspend-then-hibernate", "lock", "lock")).await;
    assert!(out.error.unwrap().contains("Choose an action for each"));
}

#[tokio::test]
async fn missing_services_name_their_packages() {
    let (_rig, _p, _h, page) = setup(false, false, false).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["profiles"]["available"], false);
    assert_eq!(out.state["profiles"]["package"], "power-profiles-daemon");
    assert_eq!(out.state["actions"]["helper"], false);
    let out = run(
        &page,
        Op::SetProfile {
            name: "balanced".into(),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("Power modes isn't available. Install the power-profiles-daemon package.")
    );
    let out = run(&page, actions("lock", "lock", "lock")).await;
    assert_eq!(
        out.error.as_deref(),
        Some("uQ's system settings helper isn't available. Install the uq package.")
    );

    let empty = SysRig::new();
    let page = PowerState::with(empty.system(), PathBuf::from("/nonexistent"));
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["power"]["package"], "upower");
    assert!(out.state["actions"]["running"].is_null());
}

#[test]
fn words() {
    assert_eq!(duration_text(0), "");
    assert_eq!(duration_text(59), "1 min");
    assert_eq!(duration_text(3600), "1 h");
    assert_eq!(duration_text(3600 + 61), "1 h 2 min");
    assert_eq!(state_text(4), "Fully charged");
    assert_eq!(state_text(99), "Unknown");
}

#[tokio::test]
async fn charging_and_actions_already_running() {
    let (_rig, p, h, page) = setup(true, true, true).await;
    {
        let mut b = p.battery.lock().unwrap();
        b.state = 1;
        b.time_to_full = 45 * 60;
    }
    // logind already runs what the file says: nothing pending.
    p.state.lock().unwrap().logind = ["lock".into(), "ignore".into(), "suspend".into()];
    let out = run(&page, actions("lock", "ignore", "suspend")).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["power"]["battery"]["state"], "Charging");
    assert_eq!(out.state["power"]["battery"]["time"], "Full in 45 min");
    assert_eq!(out.state["actions"]["pending"], false);
    assert!(h.read("etc/systemd/logind.conf.d/50-uq.conf").is_some());
}

/// Follow the page's changes as the model does; the first notice comes once it is subscribed.
async fn watching(page: &Arc<PowerState>) -> (Arc<Notify>, tokio::task::JoinHandle<()>) {
    let changed = Arc::new(Notify::new());
    let watcher = {
        let page = page.clone();
        let changed = changed.clone();
        tokio::spawn(async move { page.watch(changed).await })
    };
    tokio::time::timeout(Duration::from_secs(10), changed.notified())
        .await
        .expect("the page subscribes");
    (changed, watcher)
}

#[tokio::test]
async fn battery_and_mode_changes_are_followed_without_polling() {
    let (_rig, p, _h, page) = setup(true, true, true).await;
    // A poll this quick would show up below if the page polled while it has signals.
    let page = Arc::new(page.with_poll(Duration::from_millis(50)));
    let (changed, watcher) = watching(&page).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(400), changed.notified())
            .await
            .is_err(),
        "nothing changed, so nothing is read again"
    );

    p.set_percentage(42.0).await;
    tokio::time::timeout(Duration::from_secs(10), changed.notified())
        .await
        .expect("UPower's change is noticed");
    let out = run(&*page, Op::Refresh).await;
    assert_eq!(out.state["power"]["battery"]["percentage"], 42.0);

    p.set_profile("power-saver").await;
    tokio::time::timeout(Duration::from_secs(10), changed.notified())
        .await
        .expect("power-profiles-daemon's change is noticed");
    let out = run(&*page, Op::Refresh).await;
    assert_eq!(out.state["profiles"]["active"], "power-saver");
    watcher.abort();
}

#[tokio::test]
async fn without_the_system_bus_the_page_polls() {
    let page = Arc::new(
        PowerState::with(
            SystemBus::at("unix:path=/nonexistent/uq-settings-test-bus"),
            PathBuf::from("/nonexistent"),
        )
        .with_poll(Duration::from_millis(50)),
    );
    let changed = Arc::new(Notify::new());
    let watcher = {
        let page = page.clone();
        let changed = changed.clone();
        tokio::spawn(async move { page.watch(changed).await })
    };
    for _ in 0..2 {
        tokio::time::timeout(Duration::from_secs(10), changed.notified())
            .await
            .expect("the fallback poll reads again");
    }
    watcher.abort();
}
