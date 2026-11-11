use super::*;
use crate::compositor::fake::FakeCompositor;
use serde_json::{Value, json};

fn client(class: &str, floating: bool, at: [i64; 2], focus: i64) -> Value {
    json!({
        "address": "0xdead", "pid": 4242, "mapped": true, "hidden": false,
        "at": at, "size": [100, 100], "workspace": {"id": 1, "name": "1"},
        "floating": floating, "class": class, "title": format!("{class} title"),
        "initialClass": class, "initialTitle": "init", "xwayland": false, "pinned": false,
        "fullscreen": 0, "tags": [], "focusHistoryID": focus
    })
}

fn comp() -> Arc<FakeCompositor> {
    let c = FakeCompositor::running();
    *c.clients_json.lock().unwrap() = json!([
        client("tiled", false, [0, 0], 0),
        client("float", true, [40, 40], 1),
        client("far", false, [500, 500], 2),
    ])
    .to_string();
    *c.monitors_json.lock().unwrap() = json!([{"id": 0, "disabled": false,
        "activeWorkspace": {"id": 1, "name": "1"}, "specialWorkspace": {"id": 0, "name": ""}}])
    .to_string();
    *c.cursor_json.lock().unwrap() = r#"{"x":60,"y":60}"#.into();
    Arc::new(c)
}

type Tokens = Arc<Mutex<Vec<String>>>;

/// Wait for the emitted token, then send each outcome in turn.
async fn respond(broker: &PickBroker, tokens: &Tokens, outcomes: &[&str]) {
    let token = loop {
        if let Some(t) = tokens.lock().unwrap().first().cloned() {
            break t;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    };
    for o in outcomes {
        broker.done(&token, o.parse().unwrap());
    }
}

async fn run(
    broker: &PickBroker,
    comp: Arc<FakeCompositor>,
    outcomes: Option<&[&str]>,
) -> (Value, Vec<String>) {
    let tokens: Tokens = Arc::default();
    let t2 = tokens.clone();
    let emit = move |t: String| t2.lock().unwrap().push(t);
    let reply = async {
        let s = pick(broker, comp, emit).await.unwrap();
        serde_json::from_str::<Value>(&s).unwrap()
    };
    let v = match outcomes {
        Some(o) => tokio::join!(reply, respond(broker, &tokens, o)).0,
        None => reply.await,
    };
    let t = tokens.lock().unwrap().clone();
    (v, t)
}

#[tokio::test]
async fn pick_click_returns_only_the_clicked_window() {
    let b = PickBroker::default();
    let (v, _) = run(&b, comp(), Some(&["shown", "click"])).await;
    assert_eq!(v["status"], "picked");
    let w = v["window"].as_object().unwrap();
    assert_eq!(w["class"], "float");
    assert_eq!(w["title"], "float title");
    assert_eq!(w["initial_class"], "float");
    assert_eq!(w["initial_title"], "init");
    assert_eq!(w["xwayland"], false);
    assert_eq!(w["floating"], true);
    assert!(!w.contains_key("address") && !w.contains_key("pid"));
    let text = v.to_string();
    assert!(!text.contains("tiled") && !text.contains("far") && !text.contains("0xdead"));
}

#[tokio::test]
async fn pick_click_on_empty_desktop_is_none() {
    let c = comp();
    *c.cursor_json.lock().unwrap() = r#"{"x":900,"y":900}"#.into();
    let (v, _) = run(&PickBroker::default(), c, Some(&["shown", "click"])).await;
    assert_eq!(v, json!({"status": "none"}));
}

#[tokio::test]
async fn pick_escape_is_cancelled() {
    let (v, _) = run(&PickBroker::default(), comp(), Some(&["shown", "cancel"])).await;
    assert_eq!(v, json!({"status": "cancelled"}));
}

#[tokio::test]
async fn pick_without_shell_is_no_shell_within_ack_timeout() {
    let b = PickBroker::with_timeouts(Duration::from_millis(50), Duration::from_secs(20));
    let t = std::time::Instant::now();
    let (v, tokens) = run(&b, comp(), None).await;
    assert_eq!(v, json!({"status": "no_shell"}));
    // The 50 ms ack timeout, not the 20 s pick timeout.
    assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
    assert_eq!(tokens.len(), 1);
}

#[tokio::test]
async fn pick_times_out() {
    let b = PickBroker::with_timeouts(Duration::from_millis(50), Duration::from_millis(100));
    let (v, _) = run(&b, comp(), Some(&["shown"])).await;
    assert_eq!(v, json!({"status": "timeout"}));
}

#[tokio::test]
async fn pick_second_request_conflicts() {
    let b = PickBroker::default();
    let tokens: Tokens = Arc::default();
    let t2 = tokens.clone();
    let first = pick(&b, comp(), move |t| t2.lock().unwrap().push(t));
    let second = async {
        // Let the first pick begin.
        while tokens.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        let err = pick(&b, comp(), |_| panic!("must not emit"))
            .await
            .unwrap_err();
        assert_eq!(
            err,
            DesktopError::Conflict("Already choosing a window.".into())
        );
        respond(&b, &tokens, &["shown", "cancel"]).await;
    };
    let (r, ()) = tokio::join!(first, second);
    assert!(r.unwrap().contains("cancelled"));
    // The slot is free again.
    assert!(b.begin().is_ok());
}

#[tokio::test]
async fn pick_forged_token_is_ignored() {
    let b = PickBroker::with_timeouts(Duration::from_millis(50), Duration::from_secs(20));
    let tokens: Tokens = Arc::default();
    let t2 = tokens.clone();
    let reply = pick(&b, comp(), move |t| t2.lock().unwrap().push(t));
    let forger = async {
        while tokens.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        for o in ["shown", "click", "cancel"] {
            b.done(&"0".repeat(32), o.parse().unwrap());
        }
    };
    let (r, ()) = tokio::join!(reply, forger);
    assert_eq!(r.unwrap(), json!({"status": "no_shell"}).to_string());
}

#[tokio::test]
async fn pick_hyprland_down_is_unavailable() {
    let c = comp();
    *c.running.lock().unwrap() = false;
    let (v, tokens) = run(&PickBroker::default(), c, None).await;
    assert_eq!(v, json!({"status": "unavailable"}));
    assert!(tokens.is_empty());
}

#[tokio::test]
async fn pick_token_is_32_hex() {
    let (_, tokens) = run(&PickBroker::default(), comp(), Some(&["shown", "cancel"])).await;
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].len(), 32);
    assert!(
        tokens[0]
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f'))
    );
}

#[test]
fn outcome_parses_only_known_words() {
    assert_eq!("shown".parse::<Outcome>().unwrap(), Outcome::Shown);
    assert!("Click".parse::<Outcome>().is_err());
}
