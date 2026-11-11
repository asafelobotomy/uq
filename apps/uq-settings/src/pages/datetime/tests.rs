use super::*;
use crate::pages::fake_system::{FakeTimedate, SysRig};
use crate::pages::fake_uqd;
use crate::pages::runner::run;
use uq_core::state::State;

async fn setup(uqd: bool) -> (SysRig, FakeTimedate, Option<fake_uqd::Rig>, DateTimeState) {
    let mut rig = SysRig::new();
    let td = FakeTimedate::start(&mut rig).await;
    let uqd = if uqd {
        Some(fake_uqd::rig(State::default()).await)
    } else {
        None
    };
    let page = DateTimeState::with(uqd.as_ref().map(|r| r.client.clone()), rig.system());
    (rig, td, uqd, page)
}

#[tokio::test]
async fn reads_zone_ntp_and_clock() {
    let (_rig, _td, _uqd, page) = setup(true).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let t = &out.state["timedate"];
    assert_eq!(t["timezone"], "Europe/London");
    assert_eq!(t["ntp"], true);
    assert_eq!(t["zones"].as_array().unwrap().len(), 4);
    assert_eq!(out.state["clock_24h"], true);
}

#[tokio::test]
async fn every_setting_round_trips() {
    let (rig, td, uqd, page) = setup(true).await;
    let out = run(
        &page,
        Op::SetTimezone {
            zone: "Europe/Berlin".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["timedate"]["timezone"], "Europe/Berlin");
    let out = run(&page, Op::SetNtp { on: false }).await;
    assert_eq!(out.state["timedate"]["ntp"], false);
    let usec = 1_800_000_000_000_000;
    let out = run(&page, Op::SetTime { usec }).await;
    assert_eq!(out.error, None);
    assert_eq!(td.state.lock().unwrap().time_usec, usec as u64);
    assert_eq!(out.state["notice"]["text"], "The date and time were set.");
    let out = run(&page, Op::SetClock24h { on: false }).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["clock_24h"], false);
    assert!(
        !uqd.as_ref()
            .unwrap()
            .state
            .lock()
            .unwrap()
            .settings
            .shell
            .clock_24h
    );

    let again = DateTimeState::with(uqd.as_ref().map(|r| r.client.clone()), rig.system());
    let out = run(&again, Op::Refresh).await;
    assert_eq!(out.state["timedate"]["timezone"], "Europe/Berlin");
    assert_eq!(out.state["timedate"]["ntp"], false);
    assert_eq!(out.state["clock_24h"], false);
}

#[tokio::test]
async fn refusals_and_checks() {
    let (_rig, td, _uqd, page) = setup(true).await;
    let out = run(
        &page,
        Op::SetTime {
            usec: 1_800_000_000_000_000,
        },
    )
    .await;
    assert!(out.error.unwrap().contains("Turn that off"));
    let out = run(
        &page,
        Op::SetTimezone {
            zone: "Mars/Olympus".into(),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("That time zone isn't known on this computer.")
    );
    let out = run(&page, Op::SetTime { usec: 5 }).await;
    assert!(out.error.unwrap().contains("between 2000 and 2200"));
    td.deny.set(true);
    let out = run(&page, Op::SetNtp { on: false }).await;
    assert_eq!(out.error, None, "quiet");
    assert_eq!(out.state["timedate"]["ntp"], true);
}

#[tokio::test]
async fn works_without_uqd_and_names_a_missing_service() {
    let (_rig, _td, _uqd, page) = setup(false).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    assert!(out.state["clock_24h"].is_null());
    assert_eq!(out.state["timedate"]["timezone"], "Europe/London");
    let out = run(&page, Op::SetClock24h { on: false }).await;
    assert_eq!(
        out.error.as_deref(),
        Some("uQ's settings service isn't running.")
    );

    let empty = SysRig::new();
    let page = DateTimeState::with(None, empty.system());
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["timedate"]["available"], false);
    assert_eq!(out.state["timedate"]["package"], "systemd");
    let out = run(&page, Op::SetNtp { on: true }).await;
    assert_eq!(
        out.error.as_deref(),
        Some("The date and time service isn't available. Install the systemd package.")
    );
}
