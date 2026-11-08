//! The hyprctl request socket: one request, one reply, bounded by a timeout.

use crate::compositor::CompositorError;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// A hung compositor must never wedge its callers (uqd holds the desktop lock while waiting).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct HyprSocket {
    runtime_dir: Option<PathBuf>,
    signature: Option<String>,
    request_timeout: Duration,
}

impl HyprSocket {
    pub fn new(runtime_dir: Option<PathBuf>, signature: Option<String>) -> Self {
        HyprSocket {
            runtime_dir,
            signature,
            request_timeout: REQUEST_TIMEOUT,
        }
    }

    pub fn from_env() -> Self {
        let get = |k| std::env::var(k).ok().filter(|v: &String| !v.is_empty());
        HyprSocket::new(
            get("XDG_RUNTIME_DIR").map(PathBuf::from),
            get("HYPRLAND_INSTANCE_SIGNATURE"),
        )
    }

    pub fn with_timeout(mut self, request: Duration) -> Self {
        self.request_timeout = request;
        self
    }

    fn path(&self) -> Option<PathBuf> {
        Some(
            self.runtime_dir
                .as_ref()?
                .join("hypr")
                .join(self.signature.as_ref()?)
                .join(".socket.sock"),
        )
    }

    /// Send one request and return the whole reply.
    pub async fn request(&self, req: &str) -> Result<String, CompositorError> {
        let path = self.path().ok_or(CompositorError::NotRunning)?;
        let exchange = async {
            let mut s = match UnixStream::connect(&path).await {
                Ok(s) => s,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                    ) =>
                {
                    return Err(CompositorError::NotRunning);
                }
                Err(e) => return Err(e.into()),
            };
            s.write_all(req.as_bytes()).await?;
            let mut reply = String::new();
            s.read_to_string(&mut reply).await?;
            Ok(reply)
        };
        let verb = req.split_whitespace().next().unwrap_or(req).to_string();
        tokio::time::timeout(self.request_timeout, exchange)
            .await
            .map_err(|_| CompositorError::Timeout(verb))?
    }

    pub async fn expect_ok(&self, req: &str) -> Result<(), CompositorError> {
        let reply = self.request(req).await?;
        match reply.trim() {
            "ok" => Ok(()),
            other => Err(CompositorError::Rejected(other.to_string())),
        }
    }

    /// Run Lua in the live compositor (`hyprctl eval`).
    pub async fn eval(&self, lua: &str) -> Result<(), CompositorError> {
        self.expect_ok(&format!("eval {lua}")).await
    }
}
