//! The Sound page against a recorded `pw-dump` of a laptop (built-in card, Bluetooth headphones,
//! two apps playing and one recording), with a fake set of tools that records each call.

use std::collections::HashSet;
use std::sync::Mutex as StdMutex;

use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;

use super::*;
use crate::pages::runner::run;

const LAPTOP: &str = include_str!("../../../tests/fixtures/pipewire/laptop.json");

#[derive(Default)]
struct FakeTools {
    dump: StdMutex<String>,
    calls: StdMutex<Vec<Vec<String>>>,
    started: StdMutex<Vec<Vec<String>>>,
    missing: StdMutex<HashSet<String>>,
    fail: StdMutex<Option<String>>,
    monitor: StdMutex<Option<tokio::sync::mpsc::UnboundedReceiver<()>>>,
}

impl Tools for Arc<FakeTools> {
    fn run(&self, argv: Vec<String>) -> BoxFuture<'static, Result<String, ToolError>> {
        let me = self.clone();
        async move {
            if me.missing.lock().unwrap().contains(&argv[0]) {
                return Err(ToolError::Missing(argv[0].clone()));
            }
            if argv[0] == "pw-dump" {
                return Ok(me.dump.lock().unwrap().clone());
            }
            me.calls.lock().unwrap().push(argv);
            match me.fail.lock().unwrap().clone() {
                Some(text) => Err(ToolError::Failed(text)),
                None => Ok(String::new()),
            }
        }
        .boxed()
    }

    fn start(&self, argv: Vec<String>) -> Result<(), ToolError> {
        self.started.lock().unwrap().push(argv);
        Ok(())
    }

    fn lines(&self, _argv: Vec<String>) -> BoxStream<'static, ()> {
        let rx = self.monitor.lock().unwrap().take().expect("one monitor");
        futures_util::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|v| (v, rx)) })
            .boxed()
    }
}

fn setup() -> (Arc<FakeTools>, SoundState, tempfile::TempDir) {
    let fake = Arc::new(FakeTools::default());
    *fake.dump.lock().unwrap() = LAPTOP.to_string();
    let root = tempfile::tempdir().unwrap();
    let page = SoundState::with(Arc::new(fake.clone()), root.path().to_path_buf());
    (fake, page, root)
}

fn by_id(list: &Value, id: u64) -> Value {
    list.as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == id)
        .cloned()
        .unwrap_or(Value::Null)
}

fn calls(fake: &FakeTools) -> Vec<String> {
    fake.calls
        .lock()
        .unwrap()
        .iter()
        .map(|c| c.join(" "))
        .collect()
}

#[tokio::test]
async fn reads_outputs_inputs_apps_and_cards() {
    let (_fake, page, _root) = setup();
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let s = out.state;
    assert_eq!(s["available"], true);
    assert_eq!(s["outputs"].as_array().unwrap().len(), 2);
    let speakers = by_id(&s["outputs"], 50);
    assert_eq!(speakers["name"], "Built-in Audio Analog Stereo");
    assert_eq!(speakers["volume"], 50, "the cube root, as wpctl shows it");
    assert_eq!(speakers["default"], true);
    assert_eq!(speakers["serial"], 1050);
    let phones = by_id(&s["outputs"], 60);
    assert_eq!(phones["volume"], 75);
    assert_eq!(phones["default"], false);
    let mic = by_id(&s["inputs"], 51);
    assert_eq!(mic["volume"], 90);
    assert_eq!(mic["mute"], true);
    assert_eq!(mic["default"], true);

    let firefox = by_id(&s["playback"], 70);
    assert_eq!(firefox["app"], "Firefox");
    assert_eq!(
        firefox["title"], "<i>Video</i> — YouTube",
        "passed on as it is"
    );
    assert_eq!(firefox["through"], 50);
    assert_eq!(firefox["pinned"], false);
    let music = by_id(&s["playback"], 71);
    assert_eq!(music["through"], 60);
    assert_eq!(music["through_serial"], 1060);
    assert_eq!(music["pinned"], true);
    assert_eq!(music["mute"], true);
    assert_eq!(music["volume"], 60);
    let rec = by_id(&s["recording"], 72);
    assert_eq!(rec["through"], 51);

    let cards = s["cards"].as_array().unwrap();
    assert_eq!(cards.len(), 2, "the camera is no sound card");
    let builtin = by_id(&s["cards"], 45);
    assert_eq!(builtin["profile"], 1);
    let names: Vec<_> = builtin["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["Off", "Analog Stereo Duplex", "Analog Stereo Output"],
        "a profile that can't work now (HDMI unplugged) is left out"
    );
    assert_eq!(s["test"], false);
}

#[tokio::test]
async fn changes_go_through_wpctl_and_pw_metadata() {
    let (fake, page, root) = setup();
    for op in [
        Op::SetDefault(60),
        Op::SetVolume(50, 42),
        Op::SetVolume(70, 250),
        Op::SetMute(51, false),
        Op::SetProfile(45, 3),
        Op::Route(70, 1060),
        Op::Route(71, 0),
    ] {
        assert_eq!(run(&page, op.clone()).await.error, None, "{op:?}");
    }
    assert_eq!(
        calls(&fake),
        [
            "wpctl set-default 60",
            "wpctl set-volume 50 0.42",
            "wpctl set-volume 70 1.00",
            "wpctl set-mute 51 0",
            "wpctl set-profile 45 3",
            "pw-metadata -n default 70 target.object 1060 Spa:Id",
            "pw-metadata -n default -d 71 target.object",
        ]
    );

    let sounds = root.path().join("usr/share/sounds/freedesktop/stereo");
    std::fs::create_dir_all(&sounds).unwrap();
    std::fs::write(sounds.join("audio-test-signal.oga"), b"x").unwrap();
    let out = run(&page, Op::Test(60)).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["test"], true);
    assert_eq!(out.state["notice"]["text"], "Playing a test sound…");
    assert_eq!(
        fake.started.lock().unwrap()[0],
        [
            "pw-play",
            "--target",
            "bluez_output.AA_BB_CC_DD_EE_01.1",
            &sounds.join("audio-test-signal.oga").display().to_string(),
        ]
    );
}

#[tokio::test]
async fn stale_or_wrong_ids_are_refused() {
    let (fake, page, _root) = setup();
    let gone = |op| {
        let page = &page;
        async move { run(page, op).await.error.unwrap() }
    };
    assert_eq!(
        gone(Op::SetDefault(70)).await,
        "That device or app is gone."
    );
    assert_eq!(
        gone(Op::SetVolume(999, 10)).await,
        "That device or app is gone."
    );
    assert_eq!(
        gone(Op::Route(50, 1060)).await,
        "That device or app is gone."
    );
    assert_eq!(
        gone(Op::Route(70, 1051)).await,
        "That device is gone.",
        "an input"
    );
    assert_eq!(
        gone(Op::SetProfile(50, 1)).await,
        "That sound card is gone."
    );
    assert!(gone(Op::SetProfile(45, 9)).await.contains("doesn't offer"));
    assert!(gone(Op::Test(50)).await.contains("sound-theme-freedesktop"));
    assert!(calls(&fake).is_empty(), "nothing was changed");
}

#[tokio::test]
async fn missing_tools_name_their_packages_and_failures_show() {
    let (fake, page, _root) = setup();
    fake.missing.lock().unwrap().insert("wpctl".into());
    let out = run(&page, Op::SetMute(50, true)).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Sound settings isn't available. Install the wireplumber package.")
    );
    assert_eq!(
        by_id(&out.state["outputs"], 50)["mute"],
        false,
        "the last good value"
    );

    fake.missing.lock().unwrap().clear();
    *fake.fail.lock().unwrap() = Some("Object '50' not found".into());
    let out = run(&page, Op::SetMute(50, true)).await;
    assert_eq!(out.error.as_deref(), Some("Object '50' not found"));

    fake.missing.lock().unwrap().insert("pw-dump".into());
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["available"], false);
    assert_eq!(out.state["package"], "pipewire");

    fake.missing.lock().unwrap().clear();
    *fake.dump.lock().unwrap() = "not json".into();
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["available"], false);
    assert!(
        out.state["text"]
            .as_str()
            .unwrap()
            .starts_with("Sound settings got unreadable data")
    );
}

#[tokio::test]
async fn the_page_reads_again_when_the_graph_changes() {
    let (fake, page, _root) = setup();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    *fake.monitor.lock().unwrap() = Some(rx);
    let page = Arc::new(page);
    let changed = Arc::new(Notify::new());
    let watcher = {
        let page = page.clone();
        let changed = changed.clone();
        tokio::spawn(async move { page.watch(changed).await })
    };
    tx.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), changed.notified())
        .await
        .expect("a change wakes the page");
    watcher.abort();
}

#[test]
fn defaults_named_either_way() {
    assert_eq!(named(&json!({"name": "a"})).as_deref(), Some("a"));
    assert_eq!(named(&json!("{\"name\": \"b\"}")).as_deref(), Some("b"));
    assert_eq!(named(&json!("c")).as_deref(), Some("c"));
    assert_eq!(named(&json!(1)), None);
    let g = parse("[]").unwrap();
    assert_eq!(g.state()["outputs"], json!([]));
}
