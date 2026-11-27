//! Power modes a profile sets and puts back.

use super::*;

#[tokio::test]
async fn power_missing_is_a_notice_not_a_failure() {
    let mut r = rig_with(FakePower::missing()).await;
    add(&mut r, TURBO).await;
    let report = r.desktop.apply_profile("turbo").await.unwrap();
    assert_eq!(
        report.notices,
        [power_notice(&PowerError::Missing.to_string())]
    );
    assert_eq!(look(&r, "gaps_in"), Some(json!(0)));
    assert_eq!(r.desktop.active_profile().unwrap().id, "turbo");
}

#[tokio::test]
async fn power_refused_is_a_notice() {
    let power = FakePower::new("balanced");
    power.refuse.store(true, Ordering::SeqCst);
    let mut r = rig_with(power).await;
    add(&mut r, TURBO).await;
    let report = r.desktop.apply_profile("turbo").await.unwrap();
    assert_eq!(
        report.notices,
        [power_notice(
            &PowerError::Refused(String::new()).to_string()
        )]
    );
    assert_eq!(look(&r, "gaps_in"), Some(json!(0)));
    assert_eq!(*r.power.mode.lock().unwrap(), Some("balanced".into()));
}

#[tokio::test]
async fn power_timeout_is_a_notice() {
    let power = FakePower::new("balanced");
    // Far past the 50 ms timeout, so a loaded machine can't blur the two.
    *power.delay.lock().unwrap() = Duration::from_secs(30);
    let mut r = rig_with(power).await;
    r.desktop.power_timeout = Duration::from_millis(50);
    add(&mut r, TURBO).await;
    let t = Instant::now();
    let report = r.desktop.apply_profile("turbo").await.unwrap();
    assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
    assert_eq!(
        report.notices,
        [power_notice(&PowerError::Timeout.to_string())]
    );
    assert_eq!(look(&r, "gaps_in"), Some(json!(0)));
}

#[tokio::test]
async fn power_set_that_times_out_is_still_put_back_if_it_lands() {
    let mut r = rig().await;
    add(&mut r, TURBO).await;
    // The read answers in time; the set doesn't, but lands later.
    r.desktop.power_timeout = Duration::from_millis(50);
    let slow = Arc::new(SlowSet(r.power.clone()));
    r.desktop.power = slow;
    let report = r.desktop.apply_profile("turbo").await.unwrap();
    assert_eq!(
        report.notices,
        [power_notice(&PowerError::Timeout.to_string())]
    );
    *r.power.mode.lock().unwrap() = Some("performance".into());
    r.desktop.power = r.power.clone();
    let report = r.desktop.profile_off().await.unwrap();
    assert!(report.kept.is_empty(), "{report:?}");
    assert_eq!(*r.power.mode.lock().unwrap(), Some("balanced".into()));
}

#[tokio::test]
async fn off_puts_power_back_only_if_unchanged() {
    let mut r = rig().await;
    add(&mut r, TURBO).await;
    let mode = |r: &Rig| r.power.mode.lock().unwrap().clone().unwrap();

    r.desktop.apply_profile("turbo").await.unwrap();
    assert_eq!(mode(&r), "performance");
    let report = r.desktop.profile_off().await.unwrap();
    assert_eq!(mode(&r), "balanced");
    assert!(
        report.kept.is_empty() && report.notices.is_empty(),
        "{report:?}"
    );

    // Changed by hand while the profile was on: kept and named.
    r.desktop.apply_profile("turbo").await.unwrap();
    *r.power.mode.lock().unwrap() = Some("power-saver".into());
    let report = r.desktop.profile_off().await.unwrap();
    assert_eq!(mode(&r), "power-saver");
    let labels: Vec<_> = report.kept.iter().map(|k| k.label.as_str()).collect();
    assert_eq!(labels, ["Power mode"]);

    // Already back where it was: nothing set, nothing reported.
    *r.power.mode.lock().unwrap() = Some("balanced".into());
    r.desktop.apply_profile("turbo").await.unwrap();
    *r.power.mode.lock().unwrap() = Some("balanced".into());
    let sets = r.power.sets.lock().unwrap().len();
    let report = r.desktop.profile_off().await.unwrap();
    assert!(
        report.kept.is_empty() && report.notices.is_empty(),
        "{report:?}"
    );
    assert_eq!(r.power.sets.lock().unwrap().len(), sets);
}

#[tokio::test]
async fn switching_keeps_the_first_power_before() {
    let mut r = rig().await;
    add(&mut r, TURBO).await;
    add(
        &mut r,
        "format = 1\nid = \"quiet\"\nname = \"Quiet\"\n[look]\ngaps_in = 2\n[power]\nprofile = \"power-saver\"\n",
    )
    .await;
    r.desktop.apply_profile("turbo").await.unwrap();
    r.desktop.apply_profile("quiet").await.unwrap();
    assert_eq!(*r.power.mode.lock().unwrap(), Some("power-saver".into()));
    r.desktop.profile_off().await.unwrap();
    assert_eq!(*r.power.mode.lock().unwrap(), Some("balanced".into()));
    // A profile without a power mode puts the first one's back.
    r.desktop.apply_profile("turbo").await.unwrap();
    add(&mut r, GAMING).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    assert_eq!(*r.power.mode.lock().unwrap(), Some("balanced".into()));
}

#[tokio::test]
async fn restore_puts_the_profiles_power_mode_back() {
    let mut r = rig().await;
    add(&mut r, TURBO).await;
    let id = r.desktop.save_restore_point("mine").unwrap();
    r.desktop.apply_profile("turbo").await.unwrap();
    assert_eq!(*r.power.mode.lock().unwrap(), Some("performance".into()));
    let notices = r.desktop.restore(&id).await.unwrap();
    assert!(notices.is_empty(), "{notices:?}");
    assert_eq!(*r.power.mode.lock().unwrap(), Some("balanced".into()));
    assert!(r.desktop.active_profile().is_none());

    // Changed by hand while the profile was on: left alone.
    r.desktop.apply_profile("turbo").await.unwrap();
    *r.power.mode.lock().unwrap() = Some("power-saver".into());
    r.desktop.restore(&id).await.unwrap();
    assert_eq!(*r.power.mode.lock().unwrap(), Some("power-saver".into()));
}

#[tokio::test]
async fn restore_with_power_refused_still_restores_with_a_notice() {
    let mut r = rig().await;
    add(&mut r, TURBO).await;
    let id = r.desktop.save_restore_point("mine").unwrap();
    r.desktop.apply_profile("turbo").await.unwrap();
    r.power.refuse.store(true, Ordering::SeqCst);
    let notices = r.desktop.restore(&id).await.unwrap();
    assert_eq!(
        notices,
        [power_notice(
            &PowerError::Refused(String::new()).to_string()
        )]
    );
    assert_eq!(look(&r, "gaps_in"), Some(json!(5)));
    assert!(r.desktop.active_profile().is_none());
}
