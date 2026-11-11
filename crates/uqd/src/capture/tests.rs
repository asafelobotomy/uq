use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::service::Desktop;
use crate::system::FakeSystem;
use std::path::PathBuf;
use uq_core::history::Origin;
use uq_core::paths::Paths;

const PAUSE: &str = "eval hl.dispatch(hl.dsp.submap(\"uq_capture\"))";
const RESUME: &str = "eval hl.dispatch(hl.dsp.submap(\"reset\"))";

fn comp() -> Arc<FakeCompositor> {
    Arc::new(FakeCompositor::running())
}

fn evals(c: &FakeCompositor) -> Vec<String> {
    c.calls()
        .into_iter()
        .filter(|x| x.starts_with("eval"))
        .collect()
}

/// Lets the timer task run after the clock moved.
async fn settle() {
    for _ in 0..5 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(start_paused = true)]
async fn capture_pause_then_resume() {
    let (c, b) = (comp(), CaptureBroker::default());
    let t = b.pause(c.clone()).await.unwrap();
    assert_eq!(t.len(), 32);
    b.resume(c.as_ref(), &t).await.unwrap();
    assert_eq!(evals(&c), [PAUSE, RESUME]);
    // Resumed means resumed: the old token is spent and the timer is gone.
    tokio::time::advance(Duration::from_secs(60)).await;
    settle().await;
    assert_eq!(evals(&c).len(), 2);
    assert!(b.resume(c.as_ref(), &t).await.is_err());
}

#[tokio::test(start_paused = true)]
async fn capture_times_out_after_15s() {
    let (c, b) = (comp(), CaptureBroker::default());
    let t = b.pause(c.clone()).await.unwrap();
    tokio::time::advance(Duration::from_secs(14)).await;
    settle().await;
    assert_eq!(evals(&c), [PAUSE], "still paused at 14 s");
    tokio::time::advance(Duration::from_secs(2)).await;
    settle().await;
    assert_eq!(evals(&c), [PAUSE, RESUME]);
    // The page's late Resume is refused, and dispatches nothing more.
    assert!(matches!(
        b.resume(c.as_ref(), &t).await,
        Err(DesktopError::InvalidArgument(_))
    ));
    assert_eq!(evals(&c).len(), 2);
}

#[tokio::test(start_paused = true)]
async fn capture_second_pause_replaces_first() {
    let (c, b) = (comp(), CaptureBroker::default());
    let first = b.pause(c.clone()).await.unwrap();
    tokio::time::advance(Duration::from_secs(10)).await;
    let second = b.pause(c.clone()).await.unwrap();
    assert_ne!(first, second);
    assert!(b.resume(c.as_ref(), &first).await.is_err());
    // The first timer would have fired at 15 s; the second pause re-armed it for 25 s.
    tokio::time::advance(Duration::from_secs(10)).await;
    settle().await;
    assert_eq!(evals(&c), [PAUSE, PAUSE], "still paused at 20 s");
    tokio::time::advance(Duration::from_secs(6)).await;
    settle().await;
    assert_eq!(evals(&c), [PAUSE, PAUSE, RESUME]);
    // Exactly one reset, not one per pause.
    tokio::time::advance(Duration::from_secs(60)).await;
    settle().await;
    assert_eq!(evals(&c).len(), 3);
}

#[tokio::test(start_paused = true)]
async fn capture_forged_token_refused() {
    let (c, b) = (comp(), CaptureBroker::default());
    // Nothing paused.
    assert!(b.resume(c.as_ref(), "").await.is_err());
    let t = b.pause(c.clone()).await.unwrap();
    for bad in ["", "0".repeat(32).as_str(), &format!("{t}0"), &t[1..]] {
        assert!(
            matches!(
                b.resume(c.as_ref(), bad).await,
                Err(DesktopError::InvalidArgument(_))
            ),
            "{bad:?}"
        );
    }
    assert_eq!(evals(&c), [PAUSE], "a refused token dispatches nothing");
    b.resume(c.as_ref(), &t).await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn capture_never_holds_the_desktop_lock() {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let c = comp();
    let desktop = Desktop::new(paths, c.clone(), Arc::new(FakeSystem::default())).unwrap();
    let desktop = Arc::new(tokio::sync::Mutex::new(desktop));
    let b = CaptureBroker::default();
    let comp_of = desktop.lock().await.comp.clone();
    let t = b.pause(comp_of).await.unwrap();
    // A write goes through while the page is still listening.
    desktop
        .lock()
        .await
        .set_section("look", r#"{"gaps_in":7}"#, Origin::Settings)
        .await
        .unwrap();
    b.resume(c.as_ref(), &t).await.unwrap();
    assert!(evals(&c).iter().any(|e| e == RESUME));
}

#[tokio::test(start_paused = true)]
async fn capture_hyprland_down_is_plain_error() {
    let c = comp();
    *c.running.lock().unwrap() = false;
    let b = CaptureBroker::default();
    let e = b.pause(c.clone()).await.unwrap_err();
    assert!(
        matches!(&e, DesktopError::Io(m) if m == "Hyprland isn't answering"),
        "{e:?}"
    );
    // Nothing was armed: no timer fires, and no token exists.
    tokio::time::advance(Duration::from_secs(60)).await;
    settle().await;
    assert_eq!(evals(&c), [PAUSE]);
    assert!(b.resume(c.as_ref(), "x").await.is_err());
}

#[tokio::test(start_paused = true)]
async fn capture_timeout_is_configurable() {
    let (c, b) = (comp(), CaptureBroker::with_timeout(Duration::from_secs(1)));
    b.pause(c.clone()).await.unwrap();
    tokio::time::advance(Duration::from_secs(2)).await;
    settle().await;
    assert_eq!(evals(&c), [PAUSE, RESUME]);
}

#[tokio::test(start_paused = true)]
async fn capture_late_pause_answer_still_resumes() {
    // Hyprland applied the switch but the reply never came: shortcuts must not stay off.
    let (c, b) = (comp(), CaptureBroker::default());
    *c.eval_timeouts.lock().unwrap() = 1;
    let e = b.pause(c.clone()).await.unwrap_err();
    assert!(matches!(&e, DesktopError::Io(_)), "{e:?}");
    assert_eq!(evals(&c), [PAUSE, RESUME], "put back at once");
    tokio::time::advance(Duration::from_secs(60)).await;
    settle().await;
    assert_eq!(evals(&c).len(), 2, "and the timer is gone");
}

#[tokio::test(start_paused = true)]
async fn capture_failed_pause_and_failed_reset_still_resumes_by_timer() {
    let (c, b) = (comp(), CaptureBroker::default());
    *c.eval_timeouts.lock().unwrap() = 2;
    assert!(b.pause(c.clone()).await.is_err());
    assert_eq!(evals(&c), [PAUSE, RESUME]);
    tokio::time::advance(Duration::from_secs(16)).await;
    settle().await;
    assert_eq!(evals(&c), [PAUSE, RESUME, RESUME], "the timer tried again");
}

#[tokio::test(start_paused = true)]
async fn capture_rejected_pause_reply_resumes() {
    let (c, b) = (comp(), CaptureBroker::default());
    *c.eval_error.lock().unwrap() = Some("weird".into());
    assert!(b.pause(c.clone()).await.is_err());
    assert_eq!(evals(&c), [PAUSE, RESUME]);
}
