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

    /// Stop everything in the scope `unit`. Whether it was asked to.
    async fn stop(&self, unit: &str) -> bool;

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
/// -- <systemd-cat> -t <tag> -- <argv…>`, both tools by absolute path.
pub fn scope_argv(
    systemd_run: &Path,
    systemd_cat: &Path,
    unit: &str,
    tag: &str,
    argv: &[String],
) -> Vec<String> {
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
    out.push("--".into());
    out.push(systemd_cat.to_string_lossy().into_owned());
    out.extend(["-t", tag, "--"].map(String::from));
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
    systemd_cat: &Path,
    unit: &str,
    tag: &str,
    argv: &[String],
    env: &[(String, String)],
    cwd: &Path,
) -> tokio::process::Command {
    let full = scope_argv(systemd_run, systemd_cat, unit, tag, argv);
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

/// The real spawner: `systemd-run`, `systemd-cat` and `journalctl` by the absolute paths found
/// when it was made, so a `PATH` in an app's environment never changes what uqd runs.
pub struct SystemSpawner {
    systemd_run: PathBuf,
    systemd_cat: PathBuf,
    journalctl: Option<PathBuf>,
    tails: Semaphore,
}

impl SystemSpawner {
    /// `None` when `systemd-run` or `systemd-cat` isn't on uqd's `PATH`.
    pub fn new() -> Option<SystemSpawner> {
        SystemSpawner::with_finder(find_on_path)
    }

    /// [`SystemSpawner::new`] with `find` in place of the `PATH` lookup.
    pub fn with_finder(find: impl Fn(&str) -> Option<PathBuf>) -> Option<SystemSpawner> {
        Some(SystemSpawner {
            systemd_run: find("systemd-run")?,
            systemd_cat: find("systemd-cat")?,
            journalctl: find("journalctl"),
            tails: Semaphore::new(super::MAX_TAILS),
        })
    }

    /// The `systemd-run` and `systemd-cat` it runs.
    pub fn tools(&self) -> (&Path, &Path) {
        (&self.systemd_run, &self.systemd_cat)
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
        let mut child = scope_command(
            &self.systemd_run,
            &self.systemd_cat,
            unit,
            tag,
            argv,
            env,
            cwd,
        )
        .spawn()?;
        let pid = child.id().unwrap_or(0);
        Ok(Child {
            pid,
            wait: Box::pin(async move { exit_of(child.wait().await) }),
        })
    }

    async fn stop(&self, unit: &str) -> bool {
        let Some(systemctl) = find_on_path("systemctl") else {
            return false;
        };
        tokio::process::Command::new(systemctl)
            .args(["--user", "stop", &format!("{unit}.scope")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|s| s.success())
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
mod fake;
