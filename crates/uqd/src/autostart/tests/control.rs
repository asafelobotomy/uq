//! Minimised apps, retry, stop and run now.

use super::*;

#[tokio::test]
async fn only_apps_set_to_start_minimised_are_hidden() {
    let plan = vec![minimised("a.desktop", true), minimised("b.desktop", false)];
    let (_rig, hider, out) = with_hider(plan, Some(Hidden::Tray)).await;
    let asked = hider.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(asked[0].starts_with("app-uq-autostart-a-"), "{asked:?}");
    assert_eq!(out[0]["minimize"], true);
    assert_eq!(out[0]["hidden"], "tray");
    assert_eq!(out[1]["minimize"], false);
    assert!(out[1].get("hidden").is_none());
}

#[tokio::test]
async fn minimised_without_a_tray_icon_is_reported() {
    let (_rig, _h, out) =
        with_hider(vec![minimised("a.desktop", true)], Some(Hidden::Minimised)).await;
    assert_eq!(out[0]["hidden"], "minimised");
}

#[tokio::test]
async fn an_unreachable_compositor_reports_nothing() {
    let (_rig, _h, out) = with_hider(vec![minimised("a.desktop", true)], None).await;
    assert!(out[0].get("hidden").is_none());
    assert_eq!(out[0]["status"], "running");
}

#[tokio::test(start_paused = true)]
async fn a_failed_app_is_started_once_more_when_retry_is_on() {
    let r = rig();
    r.fake.exit_once("uq-autostart-a", ExitInfo::Code(1));
    start(&r.runner, vec![with_retry("a.desktop")]).await;
    settle().await;
    assert_eq!(r.fake.calls().len(), 1);
    tokio::time::sleep(RETRY_AFTER + Duration::from_secs(1)).await;
    settle().await;
    assert_eq!(r.fake.calls().len(), 2, "started again after it failed");
    let out = rows(&r.runner, &[with_retry("a.desktop")]).await;
    assert_eq!(out[0]["status"], "running");
    assert_eq!(out[0]["retried"], true);
}

#[tokio::test(start_paused = true)]
async fn retry_is_once_and_only_when_asked() {
    let r = rig();
    r.fake.exit("uq-autostart-a", ExitInfo::Code(1));
    r.fake.exit("uq-autostart-b", ExitInfo::Code(1));
    start(
        &r.runner,
        vec![with_retry("a.desktop"), entry("b.desktop", &["app"], 0)],
    )
    .await;
    tokio::time::sleep(RETRY_AFTER * 3).await;
    settle().await;
    let units: Vec<String> = r.fake.calls().into_iter().map(|c| c.tag).collect();
    assert_eq!(
        units.iter().filter(|t| *t == "uq-autostart-a").count(),
        2,
        "{units:?}"
    );
    assert_eq!(units.iter().filter(|t| *t == "uq-autostart-b").count(), 1);
    let out = rows(&r.runner, &[with_retry("a.desktop")]).await;
    assert_eq!(out[0]["status"], "failed");
}

#[tokio::test]
async fn stop_now_ends_a_running_app_without_calling_it_a_failure() {
    let r = rig();
    let plan = vec![entry("a.desktop", &["app"], 0)];
    start(&r.runner, plan.clone()).await;
    settle().await;
    assert!(!stop_now(&r.runner, "nope.desktop", || {}).await);
    assert!(stop_now(&r.runner, "a.desktop", || {}).await);
    settle().await;
    assert_eq!(r.fake.stops.lock().unwrap().len(), 1);
    let out = rows(&r.runner, &plan).await;
    assert_eq!(out[0]["status"], "exited");
    // Nothing runs now, so there is nothing to stop.
    assert!(!stop_now(&r.runner, "a.desktop", || {}).await);
}

#[tokio::test]
async fn run_now_starts_an_app_that_is_not_running_once() {
    let r = rig();
    let e = entry("a.desktop", &["app"], 600);
    // Not started by the login plan (it is still waiting for its delay) counts as running.
    assert!(run_now(&r.runner, &e, Vec::new(), || {}).await);
    settle().await;
    assert_eq!(r.fake.calls().len(), 1);
    assert!(
        !run_now(&r.runner, &e, Vec::new(), || {}).await,
        "already running"
    );
    assert!(stop_now(&r.runner, "a.desktop", || {}).await);
    settle().await;
    assert!(run_now(&r.runner, &e, Vec::new(), || {}).await);
    assert_eq!(r.fake.calls().len(), 2);
    let off = Resolved {
        plan: Plan::Off,
        ..e
    };
    assert!(!run_now(&r.runner, &off, Vec::new(), || {}).await);
}

#[tokio::test(start_paused = true)]
async fn rows_say_how_long_after_login_an_app_started() {
    let r = rig();
    let plan = vec![entry("a.desktop", &["app"], 7)];
    start(&r.runner, plan.clone()).await;
    settle().await;
    let out = rows(&r.runner, &plan).await;
    assert_eq!(out[0]["start_in"], 7);
    assert_eq!(out[0]["started_after"], 7);
}

#[tokio::test(start_paused = true)]
async fn run_now_after_login_has_no_after_login_time() {
    let r = rig();
    let plan = vec![entry("a.desktop", &["app"], 0)];
    start(&r.runner, plan.clone()).await;
    settle().await;
    assert!(stop_now(&r.runner, "a.desktop", || {}).await);
    settle().await;
    tokio::time::sleep(std::time::Duration::from_secs(90)).await;
    assert!(run_now(&r.runner, &plan[0], Vec::new(), || {}).await);
    settle().await;
    let out = rows(&r.runner, &plan).await;
    assert_eq!(out[0]["status"], "running");
    assert!(
        out[0].get("started_after").is_none_or(Value::is_null),
        "Run now isn't a start after login: {}",
        out[0]
    );
}
