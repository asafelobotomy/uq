//! The Restore page's side of Bindle: which `bindle` to run, the arguments of each action, and
//! a run whose output the page shows as it comes. Bindle does the work (its root helper asks
//! for the password through uQ's polkit agent); nothing here touches the system itself.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Notify;

/// Where Bindle's uQ files expect it (a link to /usr/local/bin/bindle), then the package's.
const PLACES: &[&str] = &[
    "/usr/lib/bindle/bindle",
    "/usr/bin/bindle",
    "/usr/local/bin/bindle",
];

/// `bindle snapshot list|settings --no-ask` exits with this when it would need the password.
pub const NEEDS_PASSWORD: i32 = 3;

/// The installed `bindle`, if there is one.
pub fn find() -> Option<PathBuf> {
    PLACES.iter().map(PathBuf::from).find(|p| p.is_file())
}

/// The arguments of a recovery action, everything already confirmed on the page; `None` for
/// an unknown one. The same as Bindle's Recovery window runs.
pub fn recover_args(kind: &str, number: u32) -> Option<Vec<String>> {
    let a: &[&str] = match kind {
        "rollback" if number > 0 => {
            return Some(vec!["rollback".into(), number.to_string(), "--yes".into()]);
        }
        "rebase" => &["rebase", "--yes"],
        "restore" => &["restore", "--erase-confirmed"],
        "keep-on" => &["restore", "--keep-image", "on"],
        "keep-off" => &["restore", "--keep-image", "off"],
        "cancel" => &["restore", "--cancel"],
        _ => return None,
    };
    Some(a.iter().map(|s| s.to_string()).collect())
}

/// What a finished command said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub ok: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// Its last line of complaint, for the page's error card.
    pub fn why(&self) -> String {
        let last = |s: &str| {
            s.lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .map(str::to_string)
        };
        last(&self.stderr)
            .or_else(|| last(&self.stdout))
            .map(|l| l.trim_start_matches("Error: ").to_string())
            .unwrap_or_else(|| "Bindle couldn't do that".to_string())
    }
}

/// Each line `r` prints joins `run`'s log.
async fn pump<R: tokio::io::AsyncRead + Unpin>(r: Option<R>, run: Arc<Run>) {
    let Some(r) = r else { return };
    let mut lines = BufReader::new(r).lines();
    while let Ok(Some(l)) = lines.next_line().await {
        run.update(|s| {
            s.log.push(l);
            if s.log.len() > MAX_LOG {
                s.log.remove(0);
            }
        });
    }
}

/// Run `exe args` to the end, stdin closed.
pub async fn output(exe: &Path, args: &[&str]) -> Output {
    match Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
    {
        Ok(o) => Output {
            ok: o.status.success(),
            code: o.status.code(),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        },
        Err(e) => Output {
            ok: false,
            code: None,
            stdout: String::new(),
            stderr: format!("couldn't start {}: {e}", exe.display()),
        },
    }
}

/// A command's JSON answer, or why there isn't one.
pub async fn json(exe: &Path, args: &[&str]) -> Result<Value, Output> {
    let o = output(exe, args).await;
    if !o.ok {
        return Err(o);
    }
    serde_json::from_str(o.stdout.trim()).map_err(|_| Output { ok: false, ..o })
}

/// The run the page shows: `{state: idle|running|done|failed, kind, log, restart}`.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct RunState {
    pub state: String,
    pub kind: String,
    pub log: Vec<String>,
    /// The next start does something (a rollback or Factory Restore): offer to restart.
    pub restart: bool,
}

/// The longest log kept; older lines go first.
const MAX_LOG: usize = 2000;

/// A run in progress, shared with the page's watch, which reads it again on each `changed`.
#[derive(Default)]
pub struct Run {
    pub state: Mutex<RunState>,
    pub changed: Notify,
}

impl Run {
    pub fn get(&self) -> RunState {
        self.state.lock().unwrap().clone()
    }

    #[cfg(test)]
    pub fn running(&self) -> bool {
        self.state.lock().unwrap().state == "running"
    }

    fn update(&self, f: impl FnOnce(&mut RunState)) {
        f(&mut self.state.lock().unwrap());
        self.changed.notify_one();
    }

    /// Start `exe args` for action `kind`; each line of its output joins the log as it comes.
    /// Refused while another run is going.
    pub fn start(
        self: &Arc<Self>,
        exe: &Path,
        args: Vec<String>,
        kind: &str,
    ) -> Result<(), String> {
        {
            let mut s = self.state.lock().unwrap();
            if s.state == "running" {
                return Err("Bindle is still busy with the last action".into());
            }
            *s = RunState {
                state: "running".into(),
                kind: kind.into(),
                log: vec![],
                restart: false,
            };
        }
        self.changed.notify_one();
        let child = Command::new(exe)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(false)
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                self.update(|s| {
                    s.state = "failed".into();
                    s.log.push(format!("couldn't start {}: {e}", exe.display()));
                });
                return Ok(());
            }
        };
        let restart = matches!(kind, "rollback" | "restore");
        let run = self.clone();
        tokio::spawn(async move {
            let (out, err) = (child.stdout.take(), child.stderr.take());
            tokio::join!(pump(out, run.clone()), pump(err, run.clone()));
            let ok = child.wait().await.is_ok_and(|s| s.success());
            run.update(|s| {
                s.state = if ok { "done" } else { "failed" }.into();
                s.restart = ok && restart;
            });
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_is_one_bindle_knows() {
        assert_eq!(
            recover_args("rollback", 7).unwrap(),
            ["rollback", "7", "--yes"]
        );
        assert!(recover_args("rollback", 0).is_none());
        assert_eq!(
            recover_args("restore", 0).unwrap(),
            ["restore", "--erase-confirmed"]
        );
        assert!(recover_args("rm -rf", 0).is_none());
    }

    #[test]
    fn why_is_the_last_line_said() {
        let o = Output {
            ok: false,
            code: Some(1),
            stdout: "a\n".into(),
            stderr: "x\nError: no snapper\n\n".into(),
        };
        assert_eq!(o.why(), "no snapper");
        let quiet = Output {
            ok: false,
            code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
        };
        assert_eq!(quiet.why(), "Bindle couldn't do that");
    }

    #[tokio::test]
    async fn a_run_collects_its_output_then_ends() {
        let run = Arc::new(Run::default());
        let sh = Path::new("/bin/sh");
        run.start(
            sh,
            vec!["-c".into(), "echo one; echo two >&2".into()],
            "rebase",
        )
        .unwrap();
        assert!(
            run.start(sh, vec!["-c".into(), "true".into()], "rebase")
                .is_err()
        );
        while run.running() {
            run.changed.notified().await;
        }
        let s = run.get();
        assert_eq!(s.state, "done");
        let mut log = s.log.clone();
        log.sort();
        assert_eq!(log, ["one", "two"]);
        assert!(!s.restart);
        run.start(sh, vec!["-c".into(), "exit 2".into()], "rollback")
            .unwrap();
        while run.running() {
            run.changed.notified().await;
        }
        assert_eq!(run.get().state, "failed");
    }
}
