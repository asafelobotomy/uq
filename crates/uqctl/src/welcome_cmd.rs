//! `uqctl welcome [--reset]`: open Welcome in Settings when it is due (Phase 5 spec §5.2).
//!
//! bindle runs `uqctl welcome` after its first-login wizard, having removed the defer file
//! (docs/welcome.md). A user who already finished Welcome is told how to see it again, and
//! nothing opens: `uqctl welcome` never shows Welcome twice unless asked with `--reset`.

use crate::settings_app;
use std::process::ExitCode;
use uq_core::client::DesktopProxy;

/// The Settings page id that opens the Welcome flow (`uq-settings --welcome`).
pub const PAGE: &str = "welcome";

/// What `uqctl welcome` did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Opened,
    AlreadyDone,
}

/// Reset first when asked, then open Welcome on the Settings app if it is due.
pub async fn run(
    conn: &zbus::Connection,
    reset: bool,
    spawn: impl FnOnce(&[String]) -> std::io::Result<()>,
) -> Result<Outcome, String> {
    let uqd = DesktopProxy::new(conn).await.map_err(|e| e.to_string())?;
    let down = |e: zbus::Error| {
        if uq_core::client::no_owner(&e) {
            "uqd is not running".to_string()
        } else {
            e.to_string()
        }
    };
    if reset {
        uqd.reset_welcome().await.map_err(down)?;
    }
    let status: serde_json::Value = serde_json::from_str(&uqd.get_welcome().await.map_err(down)?)
        .map_err(|e| format!("bad welcome status from uqd: {e}"))?;
    if status["pending"] != true {
        return Ok(Outcome::AlreadyDone);
    }
    settings_app::open(conn, PAGE, "", spawn).await?;
    Ok(Outcome::Opened)
}

pub async fn main(reset: bool) -> ExitCode {
    let res = settings_app::within(std::time::Duration::from_secs(15), async {
        match zbus::Connection::session().await {
            Ok(conn) => run(&conn, reset, settings_app::spawn_detached).await,
            Err(e) => Err(format!("{e} (is a session bus running?)")),
        }
    })
    .await;
    match res {
        Ok(Outcome::Opened) => ExitCode::SUCCESS,
        Ok(Outcome::AlreadyDone) => {
            println!("Welcome was already shown. To see it again: uqctl welcome --reset");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("uqctl: {msg}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;
    use uq_core::test_bus::{connect, private_bus};

    /// A real uqd on a private bus.
    async fn uqd(
        bus: &uq_core::test_bus::Bus,
        dir: &std::path::Path,
    ) -> (zbus::Connection, uqd::dbus::SharedDesktop) {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut paths = uq_core::paths::Paths::under(dir);
        paths.themes_dir = repo.join("themes");
        paths.templates_dir = repo.join("templates");
        let desktop = uqd::service::Desktop::new(
            paths,
            Arc::new(uqd::compositor::fake::FakeCompositor::running()),
            Arc::new(uqd::system::FakeSystem::default()),
        )
        .unwrap();
        uqd::dbus::serve(
            zbus::connection::Builder::address(bus.address.as_str()).unwrap(),
            desktop,
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn opens_when_due_then_not_again_until_reset() {
        let bus = private_bus();
        let dir = tempfile::tempdir().unwrap();
        let _uqd = uqd(&bus, dir.path()).await;
        let conn = connect(&bus).await;

        let mut argv = Vec::new();
        let out = run(&conn, false, |a| {
            argv = a.to_vec();
            Ok(())
        })
        .await
        .unwrap();
        assert_eq!(out, Outcome::Opened);
        assert_eq!(argv, ["uq-settings", "--page", "welcome", "--setting", ""]);

        // Settings marks Welcome done when it shows it.
        let proxy = DesktopProxy::new(&conn).await.unwrap();
        proxy
            .set_welcome_done(uq_core::welcome::CURRENT_WELCOME_VERSION)
            .await
            .unwrap();
        let out = run(&conn, false, |_| panic!("already done: nothing opens"))
            .await
            .unwrap();
        assert_eq!(out, Outcome::AlreadyDone);

        let mut opened = false;
        let out = run(&conn, true, |_| {
            opened = true;
            Ok(())
        })
        .await
        .unwrap();
        assert_eq!(out, Outcome::Opened);
        assert!(opened, "--reset opens Welcome again");
        let st: serde_json::Value =
            serde_json::from_str(&proxy.get_welcome().await.unwrap()).unwrap();
        assert_eq!(st["pending"], true);
    }

    #[tokio::test]
    async fn uqd_down_is_an_error() {
        let bus = private_bus();
        let conn = connect(&bus).await;
        let err = run(&conn, false, |_| panic!("must not open"))
            .await
            .unwrap_err();
        assert_eq!(err, "uqd is not running");
    }
}
