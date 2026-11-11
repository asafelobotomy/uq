//! Telling X11 apps which screen is the main one (`xrandr --output NAME --primary`).
//!
//! The binary is looked up once in `main.rs`; the call is argv only, never a shell.

use async_trait::async_trait;
use std::path::PathBuf;
use std::time::Duration;
use uq_core::settings::valid_output_name;

const TIMEOUT: Duration = Duration::from_secs(2);

#[async_trait]
pub trait Xrandr: Send + Sync {
    async fn set_primary(&self, output: &str) -> Result<(), String>;
}

/// Does nothing: tests, a dev session, and a machine without `xrandr`.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoXrandr;

#[async_trait]
impl Xrandr for NoXrandr {
    async fn set_primary(&self, _output: &str) -> Result<(), String> {
        Ok(())
    }
}

/// The real `xrandr`, by absolute path.
#[derive(Debug, Clone)]
pub struct SystemXrandr {
    path: PathBuf,
}

impl SystemXrandr {
    /// `xrandr` from `PATH`; `None` when it isn't installed or `DISPLAY` is unset.
    pub fn find() -> Option<SystemXrandr> {
        std::env::var_os("DISPLAY").filter(|d| !d.is_empty())?;
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path)
            .map(|d| d.join("xrandr"))
            .find(|p| p.is_absolute() && p.is_file())
            .map(|path| SystemXrandr { path })
    }

    #[cfg(test)]
    pub(crate) fn at(path: PathBuf) -> Self {
        SystemXrandr { path }
    }
}

#[async_trait]
impl Xrandr for SystemXrandr {
    async fn set_primary(&self, output: &str) -> Result<(), String> {
        if !valid_output_name(output) || output.starts_with('-') {
            return Err(format!("not a screen name: {output:?}"));
        }
        let mut cmd = tokio::process::Command::new(&self.path);
        cmd.args(["--output", output, "--primary"])
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);
        let out = tokio::time::timeout(TIMEOUT, cmd.output())
            .await
            .map_err(|_| "xrandr did not answer in time".to_string())?
            .map_err(|e| format!("running xrandr: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "xrandr: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }
}

/// Remembers the screens it was asked to make main.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct RecordingXrandr {
    pub calls: std::sync::Mutex<Vec<String>>,
}

#[cfg(test)]
#[async_trait]
impl Xrandr for RecordingXrandr {
    async fn set_primary(&self, output: &str) -> Result<(), String> {
        self.calls.lock().unwrap().push(output.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn argv_is_output_and_primary_only() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("argv");
        let script = dir.path().join("xrandr");
        crate::test_util::write_script(
            &script,
            &format!("#!/bin/sh\nprintf '%s|' \"$@\" > {}\n", log.display()),
        );
        let x = SystemXrandr::at(script);
        let mut tries = 0;
        loop {
            match x.set_primary("DP-1").await {
                Ok(()) => break,
                Err(e) if e.contains("busy") && tries < 100 => {
                    tries += 1;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(e) => panic!("{e}"),
            }
        }
        assert_eq!(
            std::fs::read_to_string(log).unwrap(),
            "--output|DP-1|--primary|"
        );
        // Names that aren't plain connectors never reach the process.
        assert!(x.set_primary("DP-1; rm -rf ~").await.is_err());
        assert!(x.set_primary("-x").await.is_err());
        assert!(x.set_primary("").await.is_err());
    }

    #[tokio::test]
    async fn a_failing_xrandr_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("xrandr");
        crate::test_util::write_script(&script, "#!/bin/sh\necho nope >&2\nexit 1\n");
        let x = SystemXrandr::at(script);
        let mut tries = 0;
        loop {
            match x.set_primary("DP-1").await {
                Err(e) if e.contains("busy") && tries < 100 => {
                    tries += 1;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(e) => {
                    assert!(e.contains("nope"), "{e}");
                    break;
                }
                Ok(()) => panic!("expected a failure"),
            }
        }
    }
}
