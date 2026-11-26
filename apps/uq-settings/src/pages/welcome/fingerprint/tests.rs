use super::fake::{Calls, Device, serve};
use super::*;
use crate::pages::fake_system::SysRig;

async fn rig(enrolled: &[&str], script: Vec<(&'static str, bool)>) -> (SysRig, Calls) {
    let mut sys = SysRig::new();
    let calls = Calls::default();
    let conn = sys.own("org.example.Unused").await;
    serve(
        &conn,
        Device {
            enrolled: Arc::new(Mutex::new(enrolled.iter().map(|s| s.to_string()).collect())),
            script,
            calls: calls.clone(),
        },
    )
    .await;
    (sys, calls)
}

#[tokio::test]
async fn no_fprintd_is_no_reader() {
    let sys = SysRig::new();
    let r = reader(&sys.system()).await;
    assert_eq!(r, Reader::default());
    assert!(!r.offer());
}

#[tokio::test]
async fn a_reader_without_fingers_is_offered() {
    let (sys, _) = rig(&[], vec![]).await;
    let r = reader(&sys.system()).await;
    assert!(r.present && !r.enrolled && r.offer());
    let (sys, _) = rig(&["left-thumb"], vec![]).await;
    let r = reader(&sys.system()).await;
    assert!(r.present && r.enrolled && !r.offer());
}

#[tokio::test]
async fn enrol_reports_each_stage_then_releases() {
    let (sys, calls) = rig(
        &[],
        vec![
            ("enroll-stage-passed", false),
            ("enroll-retry-scan", false),
            ("enroll-stage-passed", false),
            ("enroll-completed", true),
        ],
    )
    .await;
    let e = Enrolment::default();
    e.run(&sys.system()).await;
    let got = e.get();
    assert_eq!(got.phase, Phase::Done);
    assert_eq!((got.stage, got.stages), (3, 3));
    assert_eq!(got.message, "Your fingerprint is saved.");
    assert_eq!(
        calls.all(),
        ["claim ", "enroll right-index-finger", "stop", "release"]
    );
    assert!(reader(&sys.system()).await.enrolled);
}

#[tokio::test]
async fn a_failed_enrol_says_why() {
    let (sys, calls) = rig(&[], vec![("enroll-duplicate", true)]).await;
    let e = Enrolment::default();
    e.run(&sys.system()).await;
    assert_eq!(e.get().phase, Phase::Failed);
    assert_eq!(e.get().message, "That finger is already saved.");
    assert_eq!(calls.all().last().map(String::as_str), Some("release"));
}

#[tokio::test]
async fn cancel_stops_and_releases() {
    let (sys, calls) = rig(&[], vec![]).await;
    let e = Enrolment::default();
    let run = {
        let e = e.clone();
        let sys = sys.system();
        tokio::spawn(async move { e.run(&sys).await })
    };
    for _ in 0..100 {
        if calls.all().iter().any(|c| c.starts_with("enroll")) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(e.busy());
    e.cancel();
    tokio::time::timeout(std::time::Duration::from_secs(5), run)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(e.get(), Enroll::default());
    assert_eq!(calls.all()[2..], ["stop", "release"]);
}

#[test]
fn every_result_has_a_plain_message() {
    for r in [
        "enroll-completed",
        "enroll-stage-passed",
        "enroll-retry-scan",
        "enroll-swipe-too-short",
        "enroll-finger-not-centered",
        "enroll-remove-and-retry",
        "enroll-data-full",
        "enroll-duplicate",
        "enroll-disconnected",
        "enroll-failed",
        "something-new",
    ] {
        let (_, m, _) = status(r);
        assert!(m.ends_with('.') && !m.contains("enroll"), "{r}: {m}");
    }
}
