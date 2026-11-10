//! `uqctl settings [PAGE] [SETTING]`: ask the running Settings app to show a page, or start it.

use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode, Stdio};
use uq_core::client::SettingsAppProxy;

const APP: &str = "uq-settings";

/// Whether the error means nobody owns `org.uq.Settings1`.
fn not_running(err: &zbus::Error) -> bool {
    const NAMES: [&str; 2] = [
        "org.freedesktop.DBus.Error.ServiceUnknown",
        "org.freedesktop.DBus.Error.NameHasNoOwner",
    ];
    match err {
        zbus::Error::MethodError(name, _, _) => NAMES.contains(&name.as_str()),
        zbus::Error::FDO(e) => matches!(
            **e,
            zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_)
        ),
        _ => false,
    }
}

/// Show the page on the running instance; when there is none, call `spawn` with the argv of a
/// fresh one.
async fn open(
    conn: &zbus::Connection,
    page: &str,
    setting: &str,
    spawn: impl FnOnce(&[String]) -> std::io::Result<()>,
) -> Result<(), String> {
    let proxy = SettingsAppProxy::new(conn)
        .await
        .map_err(|e| e.to_string())?;
    match proxy.show_page(page, setting).await {
        Ok(()) => Ok(()),
        Err(e) if not_running(&e) => {
            let argv: Vec<String> = [APP, "--page", page, "--setting", setting]
                .map(String::from)
                .into();
            spawn(&argv).map_err(|e| format!("could not start {APP}: {e}"))
        }
        Err(zbus::Error::MethodError(name, msg, _)) => Err(msg.unwrap_or_else(|| name.to_string())),
        Err(e) => Err(e.to_string()),
    }
}

/// Start the app in its own process group, so it outlives uqctl, with no stdio. Never waited on.
fn spawn_detached(argv: &[String]) -> std::io::Result<()> {
    Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(drop)
}

pub async fn main(page: &str, setting: &str) -> ExitCode {
    let res = match zbus::Connection::session().await {
        Ok(conn) => open(&conn, page, setting, spawn_detached).await,
        Err(e) => Err(format!("{e} (is a session bus running?)")),
    };
    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("uqctl: {msg}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::process::Child;
    use std::sync::{Arc, Mutex};

    struct Bus {
        child: Child,
        address: String,
        _dir: tempfile::TempDir,
    }

    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    /// A private dbus-daemon; the socket path stays under 108 bytes.
    fn private_bus() -> Bus {
        let dir = tempfile::Builder::new()
            .prefix("uqs")
            .tempdir_in("/tmp")
            .unwrap();
        let conf = dir.path().join("c.conf");
        std::fs::write(
            &conf,
            format!(
                "<busconfig><type>session</type><listen>unix:path={}</listen>\
                 <policy context=\"default\"><allow send_destination=\"*\" eavesdrop=\"true\"/>\
                 <allow eavesdrop=\"true\"/><allow own=\"*\"/></policy></busconfig>",
                dir.path().join("b").display()
            ),
        )
        .unwrap();
        let mut child = Command::new("dbus-daemon")
            .args(["--nofork", "--print-address"])
            .arg(format!("--config-file={}", conf.display()))
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon");
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let address = line.trim().to_string();
        assert!(address.len() < 108, "{address}");
        Bus {
            child,
            address,
            _dir: dir,
        }
    }

    async fn connect(bus: &Bus) -> zbus::Connection {
        zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }

    struct Fake(Arc<Mutex<Vec<(String, String)>>>);

    #[zbus::interface(name = "org.uq.Settings1")]
    impl Fake {
        fn show_page(&self, page_id: &str, setting_id: &str) {
            self.0
                .lock()
                .unwrap()
                .push((page_id.into(), setting_id.into()));
        }
    }

    #[tokio::test]
    async fn show_page_on_running_instance() {
        let bus = private_bus();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let _app = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name("org.uq.Settings1")
            .unwrap()
            .serve_at("/org/uq/Settings1", Fake(calls.clone()))
            .unwrap()
            .build()
            .await
            .unwrap();
        let conn = connect(&bus).await;
        open(&conn, "wallpaper", "fit", |_| panic!("must not spawn"))
            .await
            .unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            vec![("wallpaper".to_string(), "fit".to_string())]
        );
    }

    #[tokio::test]
    async fn spawns_when_absent() {
        let bus = private_bus();
        let conn = connect(&bus).await;
        let mut got: Vec<String> = Vec::new();
        open(&conn, "wallpaper", "", |argv| {
            got = argv.to_vec();
            Ok(())
        })
        .await
        .unwrap();
        assert_eq!(got, ["uq-settings", "--page", "wallpaper", "--setting", ""]);
    }

    #[tokio::test]
    async fn spawn_failure_is_an_error() {
        let bus = private_bus();
        let conn = connect(&bus).await;
        let err = open(&conn, "", "", |_| Err(std::io::ErrorKind::NotFound.into()))
            .await
            .unwrap_err();
        assert!(err.contains("could not start uq-settings"), "{err}");
    }
}
