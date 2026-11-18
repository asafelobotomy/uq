//! `uqlip`: follows the Wayland clipboard, keeps the history, answers `uqctl clip` and the shell.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Command, Stdio};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uqlip::config::{Config, config_path, data_dir, socket_path};
use uqlip::daemon::Daemon;
use uqlip::ipc::Request;
use uqlip::store::Store;
use uqlip::watch::{self, Event, Policy};

/// The focused window's class, for the ignore list and the "from" label. Best effort: Hyprland
/// only, and `None` anywhere else. `UQ_CLIP_TEST_APP` stands in for it in tests.
fn active_app() -> Option<String> {
    if let Ok(a) = std::env::var("UQ_CLIP_TEST_APP") {
        return Some(a).filter(|a| !a.is_empty());
    }
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    let out = Command::new("hyprctl")
        .args(["-j", "activewindow"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    v["class"]
        .as_str()
        .filter(|c| !c.is_empty())
        .map(str::to_owned)
}

fn serve(stream: UnixStream, daemon: Arc<Mutex<Daemon>>) {
    let mut out = match stream.try_clone() {
        Ok(o) => o,
        Err(_) => return,
    };
    let mut lines = BufReader::new(stream).lines();
    while let Some(Ok(line)) = lines.next() {
        let reply = match serde_json::from_str::<Request>(&line) {
            Ok(Request::Watch) => {
                let (tx, rx) = channel::<String>();
                {
                    let mut d = daemon.lock().unwrap();
                    let _ = out.write_all(b"{\"ok\":true}\n");
                    d.add_watcher(tx);
                }
                while let Ok(l) = rx.recv() {
                    if out
                        .write_all(l.as_bytes())
                        .and_then(|_| out.write_all(b"\n"))
                        .is_err()
                    {
                        break;
                    }
                }
                return;
            }
            Ok(req) => daemon.lock().unwrap().handle(req),
            Err(e) => serde_json::json!({"ok": false, "error": format!("not a request: {e}")}),
        };
        if out
            .write_all(reply.to_string().as_bytes())
            .and_then(|_| out.write_all(b"\n"))
            .is_err()
        {
            return;
        }
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let config = Config::load(&config_path());
    let dir = data_dir();
    let store = match Store::open(&dir, config.limits()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("uqlip: {e}");
            std::process::exit(1);
        }
    };
    let daemon = Arc::new(Mutex::new(Daemon::new(store, config.clone())));
    {
        let mut d = daemon.lock().unwrap();
        if config.clear_on_login
            && let Err(e) = d.store.clear(true)
        {
            tracing::warn!("couldn't clear the history at start: {e}");
        }
        d.expire_due();
    }

    let sock = socket_path();
    if let Some(parent) = sock.parent() {
        let _ = std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent);
    }
    // A leftover socket from a crash would refuse to bind; a live daemon would answer.
    if UnixStream::connect(&sock).is_ok() {
        eprintln!("uqlip: already running");
        std::process::exit(1);
    }
    let _ = std::fs::remove_file(&sock);
    let listener = match UnixListener::bind(&sock) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("uqlip: can't listen on {}: {e}", sock.display());
            std::process::exit(1);
        }
    };
    let _ = std::fs::set_permissions(&sock, std::fs::Permissions::from_mode(0o600));

    let (tx, rx) = channel::<Event>();
    let policy: Policy = {
        let d = daemon.clone();
        Arc::new(move |offered, hint| {
            let (paused, privacy, enabled) = {
                let d = d.lock().unwrap();
                (d.paused, d.config.privacy.clone(), d.config.enabled)
            };
            enabled
                && privacy
                    .check(paused, offered, hint, active_app().as_deref())
                    .is_ok()
        })
    };
    let max = config.limits().max_entry_bytes;
    match watch::spawn(tx, policy, max) {
        Ok(h) => {
            let mut d = daemon.lock().unwrap();
            d.protocol = h.protocol();
            let h = Arc::new(Mutex::new(h));
            d.set_setter(move |m, data| h.lock().unwrap().set_selection(m, data));
            tracing::info!("watching the clipboard through {}", d.protocol);
        }
        Err(e) => tracing::error!("can't watch the clipboard: {e}"),
    }

    let d2 = daemon.clone();
    std::thread::spawn(move || {
        for s in listener.incoming().flatten() {
            let d = d2.clone();
            std::thread::spawn(move || serve(s, d));
        }
    });

    loop {
        match rx.recv_timeout(Duration::from_secs(600)) {
            Ok(Event::Copied(c)) => {
                let app = active_app();
                let mut d = daemon.lock().unwrap();
                d.captured(c, app.as_deref());
                d.expire_due();
            }
            Ok(Event::Ignored) => {}
            Ok(Event::Unsupported) => {
                tracing::error!("this compositor can't be watched: no data-control protocol");
                daemon.lock().unwrap().protocol = "none";
            }
            Err(RecvTimeoutError::Timeout) => daemon.lock().unwrap().expire_due(),
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}
