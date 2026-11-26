use super::*;

#[test]
fn clients_parse_and_skip_odd_entries() {
    let reply = r#"[
      {"address":"0x55d0c0ffee","mapped":true,"pid":4242,"workspace":{"id":3,"name":"3"}},
      {"address":"0x1","mapped":false,"pid":7,"workspace":{"id":-98,"name":"special:minimized"}},
      {"address":"nope","pid":1,"workspace":{"id":1}},
      {"address":"0x2","pid":-1,"workspace":{"id":1}},
      {"address":"0x3","pid":5}
    ]"#;
    assert_eq!(
        parse_clients(reply),
        vec![
            Client {
                address: "0x55d0c0ffee".into(),
                pid: 4242,
                workspace: 3,
                mapped: true
            },
            Client {
                address: "0x1".into(),
                pid: 7,
                workspace: -98,
                mapped: false
            },
        ]
    );
    assert!(parse_clients("not json").is_empty());
    assert!(parse_clients("{}").is_empty());
}

#[test]
fn lua_matches_the_shells_own_minimise_and_close() {
    let w = r#""address:0x55d0c0ffee""#;
    assert_eq!(
        minimise_lua("0x55d0c0ffee", 4),
        [
            format!(
                "return hl.dispatch(hl.dsp.window.tag({{ tag = \"+uq_ws_4\", window = {w} }}))"
            ),
            format!(
                "return hl.dispatch(hl.dsp.window.move({{ workspace = \"special:minimized\", follow = false, window = {w} }}))"
            ),
        ]
    );
    assert_eq!(
        close_lua("0x55d0c0ffee"),
        format!("return hl.dispatch(hl.dsp.window.close({{ window = {w} }}))")
    );
}

#[test]
fn addresses_and_bus_names_are_checked() {
    assert!(valid_address("0xdeadbeef"));
    for bad in [
        "",
        "0x",
        "0xzz",
        "x1",
        "0x1\") os.exit()--",
        "0x11111111111111111",
    ] {
        assert!(!valid_address(bad), "{bad}");
    }
    assert_eq!(
        item_bus_name("org.kde.StatusNotifierItem-1234-1/StatusNotifierItem"),
        Some("org.kde.StatusNotifierItem-1234-1")
    );
    assert_eq!(item_bus_name(":1.45/org/ayatana"), Some(":1.45"));
    assert_eq!(item_bus_name("/only/path"), None);
    assert_eq!(item_bus_name(""), None);
}

#[test]
fn hidden_keys() {
    assert_eq!(Hidden::Tray.key(), "tray");
    assert_eq!(Hidden::Minimised.key(), "minimised");
    assert_eq!(Hidden::NoWindow.key(), "no_window");
}

use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

const UNIT: &str = "app-uq-autostart-nm-applet-0000abcd";

/// A Hyprland that lists `clients` and records every `eval`.
fn fake_hyprland(dir: &std::path::Path, clients: String) -> (HyprSocket, Arc<Mutex<Vec<String>>>) {
    let sock = dir.join("hypr/sig");
    std::fs::create_dir_all(&sock).unwrap();
    let listener = UnixListener::bind(sock.join(".socket.sock")).unwrap();
    let evals = Arc::new(Mutex::new(Vec::new()));
    let seen = evals.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = listener.accept().await else {
                return;
            };
            let mut buf = vec![0u8; 4096];
            let n = s.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).into_owned();
            let reply = if req == "j/clients" {
                clients.clone()
            } else {
                seen.lock().unwrap().push(req);
                "ok".to_string()
            };
            let _ = s.write_all(reply.as_bytes()).await;
        }
    });
    (
        HyprSocket::new(Some(dir.to_path_buf()), Some("sig".into())),
        evals,
    )
}

struct FakeTray(Vec<u32>);

#[async_trait]
impl Tray for FakeTray {
    async fn pids(&self) -> Vec<u32> {
        self.0.clone()
    }
}

/// `/proc` with pid 100 in the app's scope and pid 200 in someone else's.
fn fake_proc(dir: &std::path::Path) -> PathBuf {
    for (pid, cg) in [
        (
            100,
            format!("0::/user.slice/user-1000.slice/user@1000.service/app.slice/{UNIT}.scope\n"),
        ),
        (
            200,
            "0::/user.slice/user-1000.slice/session-2.scope\n".to_string(),
        ),
    ] {
        let d = dir.join("proc").join(pid.to_string());
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("cgroup"), cg).unwrap();
    }
    dir.join("proc")
}

const CLIENTS: &str = r#"[
  {"address":"0xaaa1","mapped":true,"pid":100,"workspace":{"id":2}},
  {"address":"0xbbb2","mapped":true,"pid":200,"workspace":{"id":2}}
]"#;

#[tokio::test]
async fn an_app_with_a_tray_icon_is_closed_to_the_tray() {
    let dir = tempfile::tempdir().unwrap();
    let (hypr, evals) = fake_hyprland(dir.path(), CLIENTS.into());
    let hider = SystemHider::with(hypr, Box::new(FakeTray(vec![100])), fake_proc(dir.path()))
        .with_waits(Duration::from_millis(50), Duration::from_millis(100));
    assert_eq!(hider.hide(UNIT).await, Some(Hidden::Tray));
    // Only the app's own window, and only a close.
    assert_eq!(
        evals.lock().unwrap().clone(),
        vec![format!("eval {}", close_lua("0xaaa1"))]
    );
}

#[tokio::test]
async fn an_app_without_a_tray_icon_is_minimised_where_it_opened() {
    let dir = tempfile::tempdir().unwrap();
    let (hypr, evals) = fake_hyprland(dir.path(), CLIENTS.into());
    // Another app's tray icon doesn't count.
    let hider = SystemHider::with(hypr, Box::new(FakeTray(vec![200])), fake_proc(dir.path()))
        .with_waits(Duration::from_millis(50), Duration::from_millis(100));
    assert_eq!(hider.hide(UNIT).await, Some(Hidden::Minimised));
    let want: Vec<String> = minimise_lua("0xaaa1", 2)
        .into_iter()
        .map(|l| format!("eval {l}"))
        .collect();
    assert_eq!(evals.lock().unwrap().clone(), want);
}

#[tokio::test]
async fn an_unreachable_compositor_gives_no_answer() {
    let dir = tempfile::tempdir().unwrap();
    let hypr = HyprSocket::new(Some(dir.path().to_path_buf()), Some("none".into()));
    let hider = SystemHider::with(hypr, Box::new(FakeTray(vec![])), fake_proc(dir.path()));
    assert_eq!(hider.hide(UNIT).await, None);
}
