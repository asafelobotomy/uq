//! `uqctl settings [PAGE] [SETTING]`: ask the running Settings app to show a page, or start it.

use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode, Stdio};
use uq_core::client::{SettingsAppProxy, no_owner};

const APP: &str = "uq-settings";

/// Show the page on the running instance; when there is none, call `spawn` with the argv of a
/// fresh one.
pub(crate) async fn open(
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
        Err(e) if no_owner(&e) => {
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
pub(crate) fn spawn_detached(argv: &[String]) -> std::io::Result<()> {
    Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(drop)
}

/// Like every other uqctl command, never hang a keybind: give up after `limit`.
pub(crate) async fn within<T>(
    limit: std::time::Duration,
    fut: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    tokio::time::timeout(limit, fut)
        .await
        .unwrap_or_else(|_| Err(format!("{APP} did not answer within {} s", limit.as_secs())))
}

pub async fn main(page: &str, setting: &str) -> ExitCode {
    let res = within(std::time::Duration::from_secs(15), async {
        match zbus::Connection::session().await {
            Ok(conn) => open(&conn, page, setting, spawn_detached).await,
            Err(e) => Err(format!("{e} (is a session bus running?)")),
        }
    })
    .await;
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
    use std::sync::{Arc, Mutex};
    use uq_core::test_bus::{connect, private_bus};

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
            .name(uq_core::client::SETTINGS_NAME)
            .unwrap()
            .serve_at(uq_core::client::SETTINGS_PATH, Fake(calls.clone()))
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

    #[tokio::test]
    async fn a_hung_call_times_out() {
        let err = within(
            std::time::Duration::from_millis(50),
            std::future::pending::<Result<(), String>>(),
        )
        .await
        .unwrap_err();
        assert!(err.contains("did not answer within"), "{err}");
        assert_eq!(
            within(std::time::Duration::from_secs(1), async { Ok(7) }).await,
            Ok(7)
        );
    }
}
