//! The only code that knows how to talk to Hyprland.

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum CompositorError {
    #[error("Hyprland is not running")]
    NotRunning,
    #[error("Hyprland rejected the request: {0}")]
    Rejected(String),
    #[error("Hyprland did not answer in time ({0})")]
    Timeout(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[async_trait]
pub trait Compositor: Send + Sync {
    /// Check a main config file without applying it.
    async fn verify(&self, main_config: &Path) -> Result<(), CompositorError>;
    /// Run Lua in the live compositor (`hyprctl eval`).
    async fn eval(&self, lua: &str) -> Result<(), CompositorError>;
    /// Re-read the config (`hyprctl reload`).
    async fn reload(&self) -> Result<(), CompositorError>;
    /// Switch the live cursor (`hyprctl setcursor <theme> <size>`).
    async fn set_cursor(&self, theme: &str, size: u8) -> Result<(), CompositorError>;
    /// Current config errors (`hyprctl configerrors`).
    async fn config_errors(&self) -> Result<Vec<String>, CompositorError>;
    /// Live outputs as JSON (`hyprctl monitors all -j`).
    async fn monitors(&self) -> Result<String, CompositorError>;
    /// Live input devices as JSON (`hyprctl devices -j`).
    async fn devices(&self) -> Result<String, CompositorError>;
}

/// Hyprland over its request socket, plus `Hyprland --verify-config` for validation.
#[derive(Debug, Clone)]
pub struct Hyprland {
    socket: HyprSocket,
    binary: PathBuf,
    verify_timeout: Duration,
}

pub use crate::hypr::{HyprSocket, REQUEST_TIMEOUT};
pub const VERIFY_TIMEOUT: Duration = Duration::from_secs(10);

impl Hyprland {
    pub fn new(runtime_dir: Option<PathBuf>, signature: Option<String>) -> Self {
        Hyprland {
            socket: HyprSocket::new(runtime_dir, signature),
            binary: PathBuf::from("Hyprland"),
            verify_timeout: VERIFY_TIMEOUT,
        }
    }

    pub fn from_env() -> Self {
        Hyprland {
            socket: HyprSocket::from_env(),
            binary: PathBuf::from("Hyprland"),
            verify_timeout: VERIFY_TIMEOUT,
        }
    }

    pub fn with_timeouts(mut self, request: Duration, verify: Duration) -> Self {
        self.socket = self.socket.with_timeout(request);
        self.verify_timeout = verify;
        self
    }

    pub fn with_binary(mut self, binary: impl Into<PathBuf>) -> Self {
        self.binary = binary.into();
        self
    }
}

#[async_trait]
impl Compositor for Hyprland {
    async fn verify(&self, main_config: &Path) -> Result<(), CompositorError> {
        let mut cmd = tokio::process::Command::new(&self.binary);
        cmd.arg("--verify-config")
            .arg("-c")
            .arg(main_config)
            .kill_on_drop(true);
        let out = match tokio::time::timeout(self.verify_timeout, cmd.output())
            .await
            .map_err(|_| CompositorError::Timeout("--verify-config".into()))?
        {
            Ok(o) => o,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(CompositorError::NotRunning);
            }
            Err(e) => return Err(e.into()),
        };
        if out.status.success() {
            return Ok(());
        }
        let mut msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let err = String::from_utf8_lossy(&out.stderr);
        if !err.trim().is_empty() {
            msg.push('\n');
            msg.push_str(err.trim());
        }
        Err(CompositorError::Rejected(msg))
    }

    async fn eval(&self, lua: &str) -> Result<(), CompositorError> {
        self.socket.eval(lua).await
    }

    async fn reload(&self) -> Result<(), CompositorError> {
        self.socket.expect_ok("reload").await
    }

    async fn set_cursor(&self, theme: &str, size: u8) -> Result<(), CompositorError> {
        // Hyprland splits the argument at the last space: theme first, size last.
        self.socket
            .expect_ok(&format!("setcursor {theme} {size}"))
            .await
    }

    async fn config_errors(&self) -> Result<Vec<String>, CompositorError> {
        let reply = self.socket.request("j/configerrors").await?;
        let lines: Vec<String> = serde_json::from_str(&reply)
            .unwrap_or_else(|_| reply.lines().map(str::to_string).collect());
        Ok(lines.into_iter().filter(|l| !l.trim().is_empty()).collect())
    }

    async fn monitors(&self) -> Result<String, CompositorError> {
        self.socket.request("j/monitors all").await
    }

    async fn devices(&self) -> Result<String, CompositorError> {
        self.socket.request("j/devices").await
    }
}

/// In-memory compositor for tests: records calls, returns configurable results.
#[cfg(any(test, feature = "fake"))]
pub mod fake {
    use super::*;
    use std::sync::Mutex;

    #[derive(Debug, Default)]
    pub struct FakeCompositor {
        pub running: Mutex<bool>,
        /// `Some(msg)` makes `verify` return `Rejected(msg)`.
        pub verify_error: Mutex<Option<String>>,
        pub calls: Mutex<Vec<String>>,
        /// (main config path, its contents) for every `verify` call.
        pub verified: Mutex<Vec<(std::path::PathBuf, String)>>,
        /// What `monitors` returns (`monitors all -j`); empty means `[]`.
        pub monitors_json: Mutex<String>,
        /// What `devices` returns (`devices -j`); empty means `{}`.
        pub devices_json: Mutex<String>,
        /// `Some(msg)` makes the next `eval` return `Rejected(msg)`, once.
        pub eval_error: Mutex<Option<String>>,
        /// `Some(msg)` makes the next `reload` return `Rejected(msg)`, once.
        pub reload_error: Mutex<Option<String>>,
    }

    impl FakeCompositor {
        pub fn running() -> Self {
            FakeCompositor {
                running: Mutex::new(true),
                ..Default::default()
            }
        }
        pub fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
        fn record(&self, call: String) -> Result<(), CompositorError> {
            self.calls.lock().unwrap().push(call);
            if *self.running.lock().unwrap() {
                Ok(())
            } else {
                Err(CompositorError::NotRunning)
            }
        }
    }

    #[async_trait]
    impl Compositor for FakeCompositor {
        async fn verify(&self, main_config: &Path) -> Result<(), CompositorError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("verify {}", main_config.display()));
            let src = std::fs::read_to_string(main_config).unwrap_or_default();
            self.verified
                .lock()
                .unwrap()
                .push((main_config.to_path_buf(), src));
            match self.verify_error.lock().unwrap().clone() {
                Some(msg) => Err(CompositorError::Rejected(msg)),
                None => Ok(()),
            }
        }
        async fn eval(&self, lua: &str) -> Result<(), CompositorError> {
            self.record(format!("eval {lua}"))?;
            match self.eval_error.lock().unwrap().take() {
                Some(msg) => Err(CompositorError::Rejected(msg)),
                None => Ok(()),
            }
        }
        async fn reload(&self) -> Result<(), CompositorError> {
            self.record("reload".into())?;
            match self.reload_error.lock().unwrap().take() {
                Some(msg) => Err(CompositorError::Rejected(msg)),
                None => Ok(()),
            }
        }
        async fn set_cursor(&self, theme: &str, size: u8) -> Result<(), CompositorError> {
            self.record(format!("setcursor {theme} {size}"))
        }
        async fn config_errors(&self) -> Result<Vec<String>, CompositorError> {
            self.record("configerrors".into()).map(|()| Vec::new())
        }
        async fn monitors(&self) -> Result<String, CompositorError> {
            self.record("monitors".into())?;
            let j = self.monitors_json.lock().unwrap().clone();
            Ok(if j.is_empty() { "[]".into() } else { j })
        }
        async fn devices(&self) -> Result<String, CompositorError> {
            self.record("devices".into())?;
            let j = self.devices_json.lock().unwrap().clone();
            Ok(if j.is_empty() { "{}".into() } else { j })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    /// `verify`, retrying while exec fails with ETXTBSY (see `test_util`).
    async fn verify_retrying_busy(h: &Hyprland, cfg: &str) -> Result<(), CompositorError> {
        for _ in 0..250 {
            match h.verify(std::path::Path::new(cfg)).await {
                Err(CompositorError::Io(e)) if crate::test_util::is_busy(&e) => {
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                other => return other,
            }
        }
        h.verify(std::path::Path::new(cfg)).await
    }

    #[tokio::test]
    async fn hyprland_without_signature_is_not_running() {
        let h = Hyprland::new(Some("/run/user/1000".into()), None);
        assert!(matches!(
            h.eval("return 1").await,
            Err(CompositorError::NotRunning)
        ));
        assert!(matches!(h.reload().await, Err(CompositorError::NotRunning)));
    }

    #[tokio::test]
    async fn hyprland_missing_socket_is_not_running() {
        let dir = tempfile::tempdir().unwrap();
        let h = Hyprland::new(Some(dir.path().into()), Some("gone".into()));
        assert!(matches!(
            h.eval("return 1").await,
            Err(CompositorError::NotRunning)
        ));
    }

    #[tokio::test]
    async fn hyprland_socket_reply_parsing() {
        let dir = tempfile::tempdir().unwrap();
        let sock_dir = dir.path().join("hypr/SIG");
        std::fs::create_dir_all(&sock_dir).unwrap();
        let listener = UnixListener::bind(sock_dir.join(".socket.sock")).unwrap();
        let server = tokio::spawn(async move {
            let mut seen = Vec::new();
            for reply in ["ok", "bad thing"] {
                let (mut s, _) = listener.accept().await.unwrap();
                let mut buf = vec![0u8; 65536];
                let n = s.read(&mut buf).await.unwrap();
                seen.push(String::from_utf8_lossy(&buf[..n]).into_owned());
                s.write_all(reply.as_bytes()).await.unwrap();
            }
            seen
        });
        let h = Hyprland::new(Some(dir.path().into()), Some("SIG".into()));
        let rule = r#"hl.workspace_rule({ workspace = "2", layout = "scrolling" })"#;
        assert!(h.eval(rule).await.is_ok());
        match h.reload().await {
            Err(CompositorError::Rejected(msg)) => assert_eq!(msg, "bad thing"),
            other => panic!("expected Rejected, got {other:?}"),
        }
        assert_eq!(
            server.await.unwrap(),
            vec![format!("eval {rule}"), "reload".to_string()]
        );
    }

    #[tokio::test]
    async fn oversized_reply_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let sock_dir = dir.path().join("hypr/SIG");
        std::fs::create_dir_all(&sock_dir).unwrap();
        let listener = UnixListener::bind(sock_dir.join(".socket.sock")).unwrap();
        tokio::spawn(async move {
            for len in [
                crate::hypr::MAX_REPLY_BYTES,
                crate::hypr::MAX_REPLY_BYTES + 1,
            ] {
                let (mut s, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 64];
                let _ = s.read(&mut buf).await.unwrap();
                let _ = s.write_all(&vec![b'x'; len]).await;
            }
        });
        let h = Hyprland::new(Some(dir.path().into()), Some("SIG".into()));
        assert_eq!(
            h.devices().await.unwrap().len(),
            crate::hypr::MAX_REPLY_BYTES
        );
        assert!(matches!(
            h.devices().await,
            Err(CompositorError::Rejected(_))
        ));
    }

    #[tokio::test]
    async fn request_times_out_when_hyprland_hangs() {
        let dir = tempfile::tempdir().unwrap();
        let sock_dir = dir.path().join("hypr/SIG");
        std::fs::create_dir_all(&sock_dir).unwrap();
        let listener = UnixListener::bind(sock_dir.join(".socket.sock")).unwrap();
        tokio::spawn(async move {
            let (_held, _) = listener.accept().await.unwrap();
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        });
        let h = Hyprland::new(Some(dir.path().into()), Some("SIG".into())).with_timeouts(
            std::time::Duration::from_millis(100),
            std::time::Duration::from_secs(1),
        );
        let started = std::time::Instant::now();
        assert!(matches!(
            h.eval("return 1").await,
            Err(CompositorError::Timeout(_))
        ));
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }

    #[tokio::test]
    async fn verify_times_out_and_kills_hung_binary() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("Hyprland");
        let marker = dir.path().join("finished");
        crate::test_util::write_script(
            &fake,
            &format!("#!/bin/sh\nsleep 3\ntouch {}\n", marker.display()),
        );
        let h = Hyprland::new(None, None).with_binary(&fake).with_timeouts(
            std::time::Duration::from_secs(1),
            std::time::Duration::from_millis(200),
        );
        // Retry ETXTBSY (script just written, other threads forking) before timing the real run.
        let mut tries = 0;
        let started = loop {
            let started = std::time::Instant::now();
            match h.verify(std::path::Path::new("/x.lua")).await {
                Err(CompositorError::Timeout(_)) => break started,
                Err(CompositorError::Io(e)) if crate::test_util::is_busy(&e) && tries < 250 => {
                    tries += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                other => panic!("expected Timeout, got {other:?}"),
            }
        };
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        assert!(!marker.exists(), "hung verify process was not killed");
    }

    #[tokio::test]
    async fn config_errors_parses_json_lines() {
        let dir = tempfile::tempdir().unwrap();
        let sock_dir = dir.path().join("hypr/SIG");
        std::fs::create_dir_all(&sock_dir).unwrap();
        let listener = UnixListener::bind(sock_dir.join(".socket.sock")).unwrap();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 1024];
            let _ = s.read(&mut buf).await.unwrap();
            s.write_all(b"[\n\t\"user.lua:3: oops\",\n\t\"\"\n]\n")
                .await
                .unwrap();
        });
        let h = Hyprland::new(Some(dir.path().into()), Some("SIG".into()));
        assert_eq!(
            h.config_errors().await.unwrap(),
            vec!["user.lua:3: oops".to_string()]
        );
    }

    #[tokio::test]
    async fn verify_reports_missing_binary_as_not_running() {
        let h = Hyprland::new(None, None).with_binary("/nonexistent/Hyprland");
        assert!(matches!(
            h.verify(std::path::Path::new("/tmp/x.lua")).await,
            Err(CompositorError::NotRunning)
        ));
    }

    #[tokio::test]
    async fn verify_uses_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("Hyprland");
        crate::test_util::write_script(
            &fake,
            "#!/bin/sh\necho \"Your config has errors: $3\"\nexit 1\n",
        );
        let h = Hyprland::new(None, None).with_binary(&fake);
        match verify_retrying_busy(&h, "/cfg/hyprland.lua").await {
            Err(CompositorError::Rejected(msg)) => {
                assert!(msg.contains("/cfg/hyprland.lua"), "{msg}")
            }
            other => panic!("expected Rejected, got {other:?}"),
        }
    }
}
