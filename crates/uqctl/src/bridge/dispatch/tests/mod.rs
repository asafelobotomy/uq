use super::*;
use serde_json::json;

const A: &str = "0x55d0c0ffee";

fn lua(name: &str, args: Value) -> Vec<String> {
    Action::from_call(name, &args).unwrap().to_lua()
}

/// A fake request socket: answers each connection with `reply(request)` and records the
/// requests.
fn fake_hypr(
    reply: fn(&str) -> String,
) -> (
    tempfile::TempDir,
    HyprSocket,
    std::sync::Arc<std::sync::Mutex<Vec<String>>>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("hypr/sig");
    std::fs::create_dir_all(&sock).unwrap();
    let l = tokio::net::UnixListener::bind(sock.join(".socket.sock")).unwrap();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = l.accept().await {
            let mut buf = vec![0u8; 8192];
            let n = s.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).into_owned();
            let _ = s.write_all(reply(&req).as_bytes()).await;
            log.lock().unwrap().push(req);
        }
    });
    let h = HyprSocket::new(Some(dir.path().to_path_buf()), Some("sig".into()));
    (dir, h, seen)
}

fn look_reply(opt: &str) -> String {
    match opt {
        "general:border_size" => r#"{"option": "general:border_size", "int": 2, "set": true }"#,
        "general:gaps_out" => r#"{"option": "general:gaps_out", "css": "4 8", "set": true }"#,
        "general:layout" => r#"{"option": "general:layout", "str": "scrolling", "set": true }"#,
        _ => "no such option",
    }
    .to_string()
}

fn batch_reply(req: &str) -> String {
    let Some(list) = req.strip_prefix("[[BATCH]]") else {
        return "unknown request".into();
    };
    list.split(';')
        .map(|r| look_reply(r.trim_start_matches("j/getoption ")))
        .collect::<Vec<_>>()
        .join(BATCH_SEPARATOR)
}

/// No batch support: one option per request.
fn single_reply(req: &str) -> String {
    match req.strip_prefix("j/getoption ") {
        Some(opt) => look_reply(opt),
        None => "unknown request".into(),
    }
}

mod actions;
mod columns;
mod script;
