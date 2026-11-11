//! The declared-command runner (Phase 4 spec §2.3), shared by the bridge (widgets) and
//! `uq-settings` (pages).
//!
//! - Only a manifest's `[[command]]` runs; `argv[0]` is an absolute path under an allowed prefix
//!   (`/usr/bin`, `/usr/lib`), checked again here.
//! - The caller's arguments are appended as separate argv entries, only as many as declared,
//!   each matching its anchored pattern in full and never starting with `-`.
//! - No shell; a scrubbed environment; its own process group; stdin closed.
//! - stdout is capped at 4 MiB and stderr at 64 KiB. Over a cap or over the timeout, the group
//!   gets `SIGTERM`, then `SIGKILL` 2 s later, and the run is an error.
//! - One run per (owner, command id) at a time: [`BusySet`].

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::integration::manifest::{CommandSpec, Output, command_prefixes, path_under};

/// stdout above this kills the command (§2.3).
pub const STDOUT_CAP: usize = 4 << 20;
/// stderr above this kills the command (§2.3).
pub const STDERR_CAP: usize = 64 << 10;
/// Between `SIGTERM` and `SIGKILL`.
pub const KILL_GRACE: Duration = Duration::from_secs(2);
/// How much of stderr goes to the journal and the error text (§5).
pub const STDERR_LOG_BYTES: usize = 2048;

/// Why a run failed (a non-zero exit is not a failure here; see [`RunResult`]).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    #[error("no command {0:?} is declared")]
    Unknown(String),
    #[error("{0}")]
    BadArgs(String),
    #[error("{0} is not an allowed program")]
    NotAllowed(String),
    #[error("already running")]
    Busy,
    #[error("couldn't start {program}: {reason}")]
    Spawn { program: String, reason: String },
    #[error("timed out after {0} ms and was stopped")]
    Timeout(u64),
    #[error("printed more than 4 MiB and was stopped")]
    StdoutTooLarge,
    #[error("wrote more than 64 KiB of errors and was stopped")]
    StderrTooLarge,
    #[error("ended by signal {0}")]
    Signal(i32),
    #[error("its output isn't valid JSON: {0}")]
    BadJson(String),
    #[error("{0}")]
    Io(String),
}

/// A finished run. `code` is the exit code; `output` is parsed per the command's `output`
/// (`null` for `none`). A non-zero exit with JSON output that doesn't parse keeps the text.
#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    pub code: i32,
    pub output: Value,
    /// The first [`STDERR_LOG_BYTES`] of stderr, for the journal.
    pub stderr_head: String,
}

impl RunResult {
    /// `{code, output}` for exit 0; `{code, error, output}` otherwise (§5).
    pub fn to_json(&self) -> Value {
        if self.code == 0 {
            json!({"code": 0, "output": self.output})
        } else {
            let first = self.stderr_head.lines().next().unwrap_or("").trim();
            let error = if first.is_empty() {
                format!("exited with code {}", self.code)
            } else {
                format!("exited with code {}: {first}", self.code)
            };
            json!({"code": self.code, "error": error, "output": self.output})
        }
    }
}

/// `{error}` for a failed run.
pub fn error_json(e: &CommandError) -> Value {
    json!({"error": e.to_string()})
}

/// The variables a command keeps (§2.3); everything else is dropped.
fn keep_var(name: &str) -> bool {
    matches!(
        name,
        "HOME" | "USER" | "LANG" | "DBUS_SESSION_BUS_ADDRESS" | "WAYLAND_DISPLAY"
    ) || name.starts_with("LC_")
        || name.starts_with("XDG_")
}

/// The environment a command runs with: the kept variables of `from`, then `PATH=/usr/bin` and
/// `UQ_INTEGRATION=1`.
pub fn scrub_env(
    from: impl IntoIterator<Item = (OsString, OsString)>,
) -> Vec<(OsString, OsString)> {
    let mut env: Vec<(OsString, OsString)> = from
        .into_iter()
        .filter(|(k, _)| k.to_str().is_some_and(keep_var))
        .collect();
    env.sort();
    env.push(("PATH".into(), "/usr/bin".into()));
    env.push(("UQ_INTEGRATION".into(), "1".into()));
    env
}

/// Check the caller's arguments against the declared ones: the exact count, each matching its
/// pattern in full and not starting with `-`.
pub fn check_args(spec: &CommandSpec, args: &[String]) -> Result<(), CommandError> {
    if args.len() != spec.args.len() {
        return Err(CommandError::BadArgs(format!(
            "{:?} takes {} argument(s), not {}",
            spec.id,
            spec.args.len(),
            args.len()
        )));
    }
    for (decl, value) in spec.args.iter().zip(args) {
        if value.starts_with('-') {
            return Err(CommandError::BadArgs(format!(
                "argument {:?} may not start with '-'",
                decl.name
            )));
        }
        if value.contains('\0') || !decl.regex.is_match(value) {
            return Err(CommandError::BadArgs(format!(
                "argument {:?} doesn't match {}",
                decl.name, decl.pattern
            )));
        }
    }
    Ok(())
}

/// Runs declared commands. [`Runner::system`] is the real one; tests inject other `argv[0]`
/// prefixes and environments.
#[derive(Debug, Clone)]
pub struct Runner {
    prefixes: Vec<PathBuf>,
    env: Vec<(OsString, OsString)>,
    grace: Duration,
}

impl Runner {
    /// `/usr/bin` and `/usr/lib`, with this process's environment scrubbed.
    pub fn system() -> Runner {
        Runner {
            prefixes: command_prefixes(),
            env: scrub_env(std::env::vars_os()),
            grace: KILL_GRACE,
        }
    }

    /// Another set of allowed `argv[0]` prefixes (tests).
    pub fn with_prefixes(mut self, prefixes: Vec<PathBuf>) -> Runner {
        self.prefixes = prefixes;
        self
    }

    /// Build the environment from `from` instead of this process's (tests).
    pub fn with_env_from(mut self, from: Vec<(OsString, OsString)>) -> Runner {
        self.env = scrub_env(from);
        self
    }

    /// The argv that would run: the fixed argv, then the checked arguments.
    pub fn argv(&self, spec: &CommandSpec, args: &[String]) -> Result<Vec<String>, CommandError> {
        let program = spec.argv.first().cloned().unwrap_or_default();
        if !path_under(&program, &self.prefixes) {
            return Err(CommandError::NotAllowed(program));
        }
        check_args(spec, args)?;
        Ok(spec.argv.iter().chain(args).cloned().collect())
    }

    /// Run `spec` with `args`. Errors are failures to run to the end; a non-zero exit is a
    /// [`RunResult`] with its code.
    pub async fn run(
        &self,
        spec: &CommandSpec,
        args: &[String],
    ) -> Result<RunResult, CommandError> {
        let argv = self.argv(spec, args)?;
        let mut cmd = tokio::process::Command::new(&argv[0]);
        cmd.args(&argv[1..])
            .env_clear()
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .current_dir("/")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| CommandError::Spawn {
            program: argv[0].clone(),
            reason: e.to_string(),
        })?;
        let pgid = child.id().and_then(|p| i32::try_from(p).ok());
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            return Err(CommandError::Io("no pipes to the command".into()));
        };
        let timeout = Duration::from_millis(spec.timeout_ms);
        let work = async {
            let (out, err) = tokio::try_join!(
                read_capped(stdout, STDOUT_CAP, CommandError::StdoutTooLarge),
                read_capped(stderr, STDERR_CAP, CommandError::StderrTooLarge),
            )?;
            let status = child
                .wait()
                .await
                .map_err(|e| CommandError::Io(e.to_string()))?;
            Ok::<_, CommandError>((out, err, status))
        };
        let outcome = match tokio::time::timeout(timeout, work).await {
            Ok(r) => r,
            Err(_) => Err(CommandError::Timeout(spec.timeout_ms)),
        };
        match outcome {
            Ok((out, err, status)) => finish(spec, out, err, status),
            Err(e) => {
                if let Some(pgid) = pgid {
                    stop_group(pgid, &mut child, self.grace).await;
                }
                Err(e)
            }
        }
    }
}

/// Read to the end, failing with `over` as soon as more than `cap` bytes arrive.
async fn read_capped(
    mut r: impl AsyncRead + Unpin,
    cap: usize,
    over: CommandError,
) -> Result<Vec<u8>, CommandError> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        let n = r
            .read(&mut chunk)
            .await
            .map_err(|e| CommandError::Io(e.to_string()))?;
        if n == 0 {
            return Ok(buf);
        }
        if buf.len() + n > cap {
            return Err(over);
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

fn killpg(pgid: i32, sig: i32) {
    // SAFETY: killpg only sends a signal; a stale or empty group is ESRCH, which is ignored.
    unsafe {
        libc::killpg(pgid, sig);
    }
}

/// `SIGTERM` the group, wait up to `grace` for the leader, then `SIGKILL` whatever is left of
/// the group and reap the leader.
async fn stop_group(pgid: i32, child: &mut tokio::process::Child, grace: Duration) {
    killpg(pgid, libc::SIGTERM);
    let _ = tokio::time::timeout(grace, child.wait()).await;
    killpg(pgid, libc::SIGKILL);
    let _ = child.wait().await;
}

fn finish(
    spec: &CommandSpec,
    out: Vec<u8>,
    err: Vec<u8>,
    status: ExitStatus,
) -> Result<RunResult, CommandError> {
    use std::os::unix::process::ExitStatusExt;
    let stderr_head = String::from_utf8_lossy(&err[..err.len().min(STDERR_LOG_BYTES)]).into_owned();
    let Some(code) = status.code() else {
        return Err(CommandError::Signal(status.signal().unwrap_or(0)));
    };
    let output = match spec.output {
        Output::None => Value::Null,
        Output::Text => Value::String(String::from_utf8_lossy(&out).into_owned()),
        Output::Json => match serde_json::from_slice::<Value>(&out) {
            Ok(v) => v,
            Err(_) if code != 0 => Value::String(String::from_utf8_lossy(&out).into_owned()),
            Err(e) => return Err(CommandError::BadJson(e.to_string())),
        },
    };
    Ok(RunResult {
        code,
        output,
        stderr_head,
    })
}

/// One run per (owner, command id) at a time (§2.3). The owner is a widget or page id.
#[derive(Debug, Clone, Default)]
pub struct BusySet(Arc<Mutex<HashSet<(String, String)>>>);

/// Held while a command runs; dropping it frees the slot.
#[derive(Debug)]
pub struct BusyToken {
    set: BusySet,
    key: (String, String),
}

impl BusySet {
    pub fn new() -> BusySet {
        BusySet::default()
    }

    /// Take the slot, or `Err(Busy)` while another run holds it.
    pub fn try_acquire(&self, owner: &str, command: &str) -> Result<BusyToken, CommandError> {
        let key = (owner.to_string(), command.to_string());
        let mut set = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !set.insert(key.clone()) {
            return Err(CommandError::Busy);
        }
        Ok(BusyToken {
            set: self.clone(),
            key,
        })
    }
}

impl Drop for BusyToken {
    fn drop(&mut self) {
        let mut set = self.set.0.lock().unwrap_or_else(|e| e.into_inner());
        set.remove(&self.key);
    }
}

/// Find `id` among `commands`.
pub fn find<'a>(commands: &'a [CommandSpec], id: &str) -> Result<&'a CommandSpec, CommandError> {
    commands
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| CommandError::Unknown(id.to_string()))
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
