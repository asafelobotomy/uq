//! How the runner starts a process and reads back what it left: the [`Spawner`] trait, the
//! real one ([`SystemSpawner`], built only by `main.rs`, F1) and the test double.
//!
//! Every command is an argv handed to `tokio::process::Command`; nothing goes through a shell.

use async_trait::async_trait;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::Semaphore;

/// How a started process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExitInfo {
    Code(i32),
    Signal(i32),
    /// The wait itself failed, so nobody knows.
    Unknown,
}

/// A started process: its pid, and a future that resolves when it ends.
pub struct Child {
    pub pid: u32,
    pub wait: Pin<Box<dyn Future<Output = ExitInfo> + Send>>,
}

#[async_trait]
pub trait Spawner: Send + Sync {
    /// Start `argv` in its own scope `unit` (without `.scope`), its output tagged `tag` in the
    /// journal, with `env` on top of uqd's own environment, in `cwd`.
    async fn spawn(
        &self,
        unit: &str,
        tag: &str,
        argv: &[String],
        env: &[(String, String)],
        cwd: &Path,
    ) -> std::io::Result<Child>;

    /// Whether `pid` still lives in the scope `unit` (started by an earlier uqd).
    async fn alive(&self, pid: u32, unit: &str) -> bool;

    /// The last `lines` journal lines of the run in `unit` tagged `tag`, from `since` (Unix
    /// seconds) on. `None` when they couldn't be read (no journalctl, a failed start, the
    /// timeout), which the runner doesn't cache; `Some(vec![])` means the run logged nothing.
    async fn journal_tail(
        &self,
        unit: &str,
        tag: &str,
        since: i64,
        lines: usize,
    ) -> Option<Vec<String>>;
}

/// The variables uqd has as a systemd service that an app must not inherit (F35).
pub fn is_service_var(name: &str) -> bool {
    matches!(
        name,
        "INVOCATION_ID" | "JOURNAL_STREAM" | "SYSTEMD_EXEC_PID" | "MANAGERPID" | "NOTIFY_SOCKET"
    ) || name.starts_with("LISTEN_")
}

/// The whole command line for one autostart run (Global Constraints, R6, R22, F35):
/// `<systemd-run> --user --scope --quiet --collect --slice=app.slice --unit=<unit>
/// -- systemd-cat -t <tag> -- <argv…>`.
pub fn scope_argv(systemd_run: &Path, unit: &str, tag: &str, argv: &[String]) -> Vec<String> {
    let mut out = vec![systemd_run.to_string_lossy().into_owned()];
    out.extend(
        [
            "--user",
            "--scope",
            "--quiet",
            "--collect",
            "--slice=app.slice",
        ]
        .map(String::from),
    );
    out.push(format!("--unit={unit}"));
    out.extend(["--", "systemd-cat", "-t", tag, "--"].map(String::from));
    out.extend(argv.iter().cloned());
    out
}

/// F16's journal read for one run.
pub fn journal_args(unit: &str, tag: &str, since: i64, lines: usize) -> Vec<String> {
    vec![
        "--user".into(),
        format!("_SYSTEMD_USER_UNIT={unit}.scope"),
        "-t".into(),
        tag.into(),
        "--since".into(),
        format!("@{since}"),
        "-n".into(),
        lines.to_string(),
        "-o".into(),
        "cat".into(),
        "--no-pager".into(),
        "-q".into(),
    ]
}

/// The process for one run: stdin from `/dev/null`, uqd's environment without the service
/// variables, plus `env`. Never killed when uqd lets go of it.
pub fn scope_command(
    systemd_run: &Path,
    unit: &str,
    tag: &str,
    argv: &[String],
    env: &[(String, String)],
    cwd: &Path,
) -> tokio::process::Command {
    let full = scope_argv(systemd_run, unit, tag, argv);
    let mut cmd = tokio::process::Command::new(&full[0]);
    cmd.args(&full[1..])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .kill_on_drop(false);
    for (name, _) in std::env::vars_os() {
        if name.to_str().is_some_and(is_service_var) {
            cmd.env_remove(name);
        }
    }
    cmd.envs(env.iter().map(|(k, v)| (k, v)));
    cmd
}

/// Whether `/proc/<pid>/cgroup`'s `text` puts the process in `<unit>.scope`.
pub fn cgroup_has_unit(text: &str, unit: &str) -> bool {
    let want = format!("/{unit}.scope");
    text.lines().any(|l| l.trim_end().ends_with(&want))
}

/// An executable regular file called `name` in an absolute entry of uqd's own `PATH`.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .filter(|d| d.is_absolute())
        .map(|d| d.join(name))
        .find(|p| {
            std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

const JOURNAL_TIMEOUT: Duration = Duration::from_secs(2);
const JOURNAL_MAX_BYTES: u64 = 64 * 1024;

/// The real spawner: `systemd-run` and `journalctl` by the absolute paths found when it was
/// made, so a `PATH` in an app's environment never changes what uqd runs.
pub struct SystemSpawner {
    systemd_run: PathBuf,
    journalctl: Option<PathBuf>,
    tails: Semaphore,
}

impl SystemSpawner {
    /// `None` when `systemd-run` isn't on uqd's `PATH`.
    pub fn new() -> Option<SystemSpawner> {
        Some(SystemSpawner {
            systemd_run: find_on_path("systemd-run")?,
            journalctl: find_on_path("journalctl"),
            tails: Semaphore::new(super::MAX_TAILS),
        })
    }
}

fn exit_of(status: std::io::Result<std::process::ExitStatus>) -> ExitInfo {
    use std::os::unix::process::ExitStatusExt;
    match status {
        Ok(s) => s
            .code()
            .map(ExitInfo::Code)
            .or(s.signal().map(ExitInfo::Signal))
            .unwrap_or(ExitInfo::Unknown),
        Err(_) => ExitInfo::Unknown,
    }
}

#[async_trait]
impl Spawner for SystemSpawner {
    async fn spawn(
        &self,
        unit: &str,
        tag: &str,
        argv: &[String],
        env: &[(String, String)],
        cwd: &Path,
    ) -> std::io::Result<Child> {
        let mut child = scope_command(&self.systemd_run, unit, tag, argv, env, cwd).spawn()?;
        let pid = child.id().unwrap_or(0);
        Ok(Child {
            pid,
            wait: Box::pin(async move { exit_of(child.wait().await) }),
        })
    }

    async fn alive(&self, pid: u32, unit: &str) -> bool {
        tokio::fs::read_to_string(format!("/proc/{pid}/cgroup"))
            .await
            .is_ok_and(|t| cgroup_has_unit(&t, unit))
    }

    async fn journal_tail(
        &self,
        unit: &str,
        tag: &str,
        since: i64,
        lines: usize,
    ) -> Option<Vec<String>> {
        let journalctl = self.journalctl.as_ref()?;
        let _permit = self.tails.acquire().await.ok()?;
        let read = async {
            let mut child = tokio::process::Command::new(journalctl)
                .args(journal_args(unit, tag, since, lines))
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .ok()?;
            let mut buf = Vec::new();
            child
                .stdout
                .take()?
                .take(JOURNAL_MAX_BYTES)
                .read_to_end(&mut buf)
                .await
                .ok()?;
            let _ = child.start_kill();
            let _ = child.wait().await;
            Some(buf)
        };
        let buf = tokio::time::timeout(JOURNAL_TIMEOUT, read).await.ok()??;
        Some(
            String::from_utf8_lossy(&buf)
                .lines()
                .map(str::to_string)
                .collect(),
        )
    }
}

#[cfg(any(test, feature = "fake"))]
pub use fake::{FakeSpawner, SpawnCall};

#[cfg(any(test, feature = "fake"))]
mod fake {
    use super::*;
    use std::collections::{BTreeSet, HashMap};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

    /// What the fake was asked to start.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SpawnCall {
        pub unit: String,
        pub tag: String,
        /// The app's own argv.
        pub argv: Vec<String>,
        /// The whole command the real spawner would run ([`scope_argv`]).
        pub command: Vec<String>,
        pub env: Vec<(String, String)>,
        pub cwd: PathBuf,
    }

    /// Records calls. Exits are scripted by journal tag; an unscripted run never ends.
    pub struct FakeSpawner {
        pub systemd_run: PathBuf,
        pub calls: Mutex<Vec<SpawnCall>>,
        pub exits: Mutex<HashMap<String, ExitInfo>>,
        /// The `(pid, unit)` pairs `alive` answers true for.
        pub live: Mutex<BTreeSet<(u32, String)>>,
        /// `(pid, unit)` of every `alive` call.
        pub alive_calls: Mutex<Vec<(u32, String)>>,
        pub journal: Mutex<Vec<String>>,
        /// Journal reads fail (`None`) while this is set.
        pub journal_fails: AtomicBool,
        pub journal_delay: Mutex<Option<Duration>>,
        /// `(unit, tag, since)` of every journal read.
        pub tails: Mutex<Vec<(String, String, i64)>>,
        pub tails_now: AtomicUsize,
        pub tails_max: AtomicUsize,
        next_pid: AtomicU32,
    }

    impl Default for FakeSpawner {
        fn default() -> Self {
            FakeSpawner {
                systemd_run: PathBuf::from("/fake/bin/systemd-run"),
                calls: Mutex::default(),
                exits: Mutex::default(),
                live: Mutex::default(),
                alive_calls: Mutex::default(),
                journal: Mutex::default(),
                journal_fails: AtomicBool::new(false),
                journal_delay: Mutex::default(),
                tails: Mutex::default(),
                tails_now: AtomicUsize::new(0),
                tails_max: AtomicUsize::new(0),
                next_pid: AtomicU32::new(1000),
            }
        }
    }

    impl FakeSpawner {
        pub fn calls(&self) -> Vec<SpawnCall> {
            self.calls.lock().unwrap().clone()
        }

        pub fn exit(&self, tag: &str, e: ExitInfo) {
            self.exits.lock().unwrap().insert(tag.to_string(), e);
        }

        /// Make `pid` alive in `unit`'s scope.
        pub fn set_alive(&self, pid: u32, unit: &str) {
            self.live.lock().unwrap().insert((pid, unit.to_string()));
        }
    }

    #[async_trait]
    impl Spawner for FakeSpawner {
        async fn spawn(
            &self,
            unit: &str,
            tag: &str,
            argv: &[String],
            env: &[(String, String)],
            cwd: &Path,
        ) -> std::io::Result<Child> {
            self.calls.lock().unwrap().push(SpawnCall {
                unit: unit.into(),
                tag: tag.into(),
                argv: argv.to_vec(),
                command: scope_argv(&self.systemd_run, unit, tag, argv),
                env: env.to_vec(),
                cwd: cwd.into(),
            });
            let pid = self.next_pid.fetch_add(1, Ordering::SeqCst);
            let exit = self.exits.lock().unwrap().get(tag).copied();
            Ok(Child {
                pid,
                wait: Box::pin(async move {
                    match exit {
                        Some(e) => e,
                        None => std::future::pending().await,
                    }
                }),
            })
        }

        async fn alive(&self, pid: u32, unit: &str) -> bool {
            let key = (pid, unit.to_string());
            self.alive_calls.lock().unwrap().push(key.clone());
            self.live.lock().unwrap().contains(&key)
        }

        async fn journal_tail(
            &self,
            unit: &str,
            tag: &str,
            since: i64,
            lines: usize,
        ) -> Option<Vec<String>> {
            self.tails
                .lock()
                .unwrap()
                .push((unit.into(), tag.into(), since));
            let now = self.tails_now.fetch_add(1, Ordering::SeqCst) + 1;
            self.tails_max.fetch_max(now, Ordering::SeqCst);
            let delay = *self.journal_delay.lock().unwrap();
            if let Some(d) = delay {
                tokio::time::sleep(d).await;
            }
            self.tails_now.fetch_sub(1, Ordering::SeqCst);
            if self.journal_fails.load(Ordering::SeqCst) {
                return None;
            }
            let all = self.journal.lock().unwrap().clone();
            Some(all[all.len().saturating_sub(lines)..].to_vec())
        }
    }
}
