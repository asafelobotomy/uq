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
    /// Open windows as JSON (`hyprctl clients -j`).
    async fn clients(&self) -> Result<String, CompositorError>;
    /// The cursor position as JSON (`hyprctl cursorpos -j`).
    async fn cursor_pos(&self) -> Result<String, CompositorError>;
    /// Open workspaces as JSON (`hyprctl workspaces -j`).
    async fn workspaces(&self) -> Result<String, CompositorError>;
    /// Every bind Hyprland has, as JSON (`hyprctl binds -j`).
    async fn binds(&self) -> Result<String, CompositorError>;
    /// One option's live value (`hyprctl -j getoption <path>`, path like `general:gaps_in`):
    /// the reply's value field (`int`, `float`, `bool`, `str`, `custom`, `css`, …).
    async fn get_option(&self, path: &str) -> Result<serde_json::Value, CompositorError>;
}

/// The value fields a `j/getoption` reply can carry, one per option type.
const OPTION_VALUE_FIELDS: [&str; 9] = [
    "int",
    "float",
    "bool",
    "str",
    "custom",
    "css",
    "vec2",
    "gradient",
    "font_weight",
];

/// The value field of a `j/getoption` reply; anything else ("no such option") is `Rejected`.
fn option_value(reply: &str) -> Result<serde_json::Value, CompositorError> {
    let rejected = || CompositorError::Rejected(reply.trim().to_string());
    let v: serde_json::Value = serde_json::from_str(reply).map_err(|_| rejected())?;
    OPTION_VALUE_FIELDS
        .iter()
        .find_map(|f| v.get(*f))
        .cloned()
        .ok_or_else(rejected)
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

    async fn clients(&self) -> Result<String, CompositorError> {
        self.socket.request("j/clients").await
    }

    async fn cursor_pos(&self) -> Result<String, CompositorError> {
        self.socket.request("j/cursorpos").await
    }

    async fn workspaces(&self) -> Result<String, CompositorError> {
        self.socket.request("j/workspaces").await
    }

    async fn binds(&self) -> Result<String, CompositorError> {
        self.socket.request("j/binds").await
    }

    async fn get_option(&self, path: &str) -> Result<serde_json::Value, CompositorError> {
        option_value(&self.socket.request(&format!("j/getoption {path}")).await?)
    }
}

/// In-memory compositor for tests: records calls, returns configurable results.
#[cfg(any(test, feature = "fake"))]
pub mod fake;

#[cfg(test)]
mod tests;
