//! Startup › Services: the person's background services (`systemctl --user` units), with
//! whether each starts at login, its run state, start, stop and restart, and its last log
//! lines. It talks to the service manager directly, never to uqd, so it keeps working when
//! uqd is down. uQ's own services are listed but locked: the desktop needs them.

mod parse;
#[cfg(test)]
mod tests;

use std::io;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify};

use crate::pages::runner::PageLogic;

/// How many log lines a service's log shows.
pub const LOG_LINES: usize = 40;
/// How often the page looks again while a service is starting or stopping.
const SETTLE_POLL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    /// List every service, not only the ones that can be turned on or off.
    SetShowAll {
        on: bool,
    },
    /// Start `unit` at login, or not.
    SetEnabled {
        unit: String,
        on: bool,
    },
    /// "start", "stop" or "restart" `unit` now.
    Act {
        unit: String,
        action: String,
    },
    /// Show `unit`'s last log lines; "" closes the log.
    ShowLog {
        unit: String,
    },
}

/// What a command printed.
#[derive(Debug, Clone, Default)]
pub struct Ran {
    pub ok: bool,
    pub out: String,
    pub err: String,
}

/// Runs `program` (`systemctl` or `journalctl`) with `args`.
pub type Runner = Arc<dyn Fn(&str, &[String]) -> io::Result<Ran> + Send + Sync>;

/// The real commands.
pub fn run_command(program: &str, args: &[String]) -> io::Result<Ran> {
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .env("SYSTEMD_PAGER", "")
        .output()?;
    Ok(Ran {
        ok: out.status.success(),
        out: String::from_utf8_lossy(&out.stdout).into_owned(),
        err: String::from_utf8_lossy(&out.stderr).trim().to_string(),
    })
}

pub struct ServicesState {
    run: Runner,
    write: Mutex<()>,
    show_all: AtomicBool,
    settling: AtomicBool,
    log: StdMutex<Option<String>>,
}

fn failure(text: impl Into<String>) -> zbus::Error {
    zbus::Error::Failure(text.into())
}

/// A unit name that is safe to hand to the commands: `name.service` of plain characters.
pub fn valid_unit(unit: &str) -> bool {
    unit.len() <= 255
        && unit.strip_suffix(".service").is_some_and(|stem| {
            !stem.is_empty()
                && !stem.starts_with('-')
                && stem.chars().all(|c| {
                    c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '.' | '-' | '\\' | '@')
                })
        })
}

impl ServicesState {
    pub fn new(_conn: zbus::Connection) -> Self {
        Self::with(Arc::new(run_command))
    }

    pub fn with(run: Runner) -> Self {
        ServicesState {
            run,
            write: Mutex::new(()),
            show_all: AtomicBool::new(false),
            settling: AtomicBool::new(false),
            log: StdMutex::new(None),
        }
    }

    /// Run `program` off the async threads; a missing program reads as a plain sentence.
    async fn call(&self, program: &'static str, args: Vec<String>) -> Result<Ran, String> {
        let run = self.run.clone();
        let ran = tokio::task::spawn_blocking(move || run(program, &args))
            .await
            .map_err(|e| e.to_string())?;
        ran.map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => format!("{program} isn't installed"),
            _ => format!("{program} couldn't run: {e}"),
        })
    }

    async fn systemctl(&self, args: &[&str]) -> Result<Ran, String> {
        let mut all = vec!["--user".to_string(), "--no-pager".to_string()];
        all.extend(args.iter().map(|a| a.to_string()));
        self.call("systemctl", all).await
    }

    /// A unit the page may change: well formed and not one of uQ's own.
    fn changeable(unit: &str) -> zbus::Result<()> {
        if !valid_unit(unit) {
            return Err(failure(format!("{unit:?} isn't a service name")));
        }
        if parse::locked(unit) {
            return Err(failure("uQ's own services can't be changed here"));
        }
        Ok(())
    }

    async fn checked(&self, args: &[&str]) -> zbus::Result<()> {
        let ran = self.systemctl(args).await.map_err(failure)?;
        if ran.ok {
            Ok(())
        } else if ran.err.is_empty() {
            Err(failure("the service manager said no"))
        } else {
            Err(failure(ran.err))
        }
    }

    async fn rows(&self) -> Result<(Vec<Value>, usize), String> {
        let list = self
            .systemctl(&[
                "list-unit-files",
                "--type=service",
                "--no-legend",
                "--plain",
            ])
            .await?;
        if !list.ok {
            return Err(list.err);
        }
        let all = parse::unit_files(&list.out);
        let show_all = self.show_all.load(Ordering::Relaxed);
        let files: Vec<_> = all
            .iter()
            .filter(|f| show_all || parse::listed_by_default(&f.state))
            .cloned()
            .collect();
        let hidden = all.len() - files.len();
        let props = if files.is_empty() {
            Default::default()
        } else {
            let mut args = vec![
                "show",
                "--property=Id,Description,ActiveState,SubState",
                "--",
            ];
            args.extend(files.iter().map(|f| f.unit.as_str()));
            let shown = self.systemctl(&args).await?;
            parse::shown(&shown.out)
        };
        let mut rows: Vec<Value> = files
            .iter()
            .map(|f| parse::row(f, props.get(&f.unit)))
            .collect();
        rows.sort_by_key(|r| {
            (
                r["locked"] == true,
                r["name"].as_str().unwrap_or("").to_lowercase(),
            )
        });
        Ok((rows, hidden))
    }

    async fn read_log(&self, unit: &str) -> Value {
        let args = [
            "--user",
            "-u",
            unit,
            "-n",
            &LOG_LINES.to_string(),
            "--no-pager",
            "-o",
            "short",
        ]
        .map(String::from)
        .to_vec();
        match self.call("journalctl", args).await {
            Ok(r) if r.ok => {
                let lines: Vec<&str> = r.out.lines().filter(|l| !l.starts_with("-- ")).collect();
                json!({"unit": unit, "lines": lines})
            }
            Ok(r) => json!({"unit": unit, "lines": [], "error": r.err}),
            Err(e) => json!({"unit": unit, "lines": [], "error": e}),
        }
    }
}

impl PageLogic for ServicesState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        match op {
            Op::Refresh => Ok(()),
            Op::SetShowAll { on } => {
                self.show_all.store(on, Ordering::Relaxed);
                Ok(())
            }
            Op::SetEnabled { unit, on } => {
                Self::changeable(&unit)?;
                let verb = if on { "enable" } else { "disable" };
                self.checked(&[verb, "--", &unit]).await
            }
            Op::Act { unit, action } => {
                Self::changeable(&unit)?;
                if !matches!(action.as_str(), "start" | "stop" | "restart") {
                    return Err(failure(format!("unknown action {action:?}")));
                }
                self.checked(&["--no-block", &action, "--", &unit]).await
            }
            Op::ShowLog { unit } => {
                if !unit.is_empty() && !valid_unit(&unit) {
                    return Err(failure(format!("{unit:?} isn't a service name")));
                }
                *self.log.lock().unwrap() = (!unit.is_empty()).then_some(unit);
                Ok(())
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let show_all = self.show_all.load(Ordering::Relaxed);
        let log = self.log.lock().unwrap().clone();
        let log = match log {
            Some(unit) => self.read_log(&unit).await,
            None => Value::Null,
        };
        Ok(match self.rows().await {
            Ok((rows, hidden)) => {
                self.settling
                    .store(parse::settling(&rows), Ordering::Relaxed);
                json!({"services": rows, "hidden": hidden, "show_all": show_all, "log": log})
            }
            Err(e) => {
                json!({"services": [], "hidden": 0, "show_all": show_all, "log": log, "error": e})
            }
        })
    }

    /// Look again every few seconds while a service is starting or stopping.
    async fn watch(&self, changed: Arc<Notify>) {
        loop {
            tokio::time::sleep(SETTLE_POLL).await;
            if self.settling.load(Ordering::Relaxed) {
                changed.notify_one();
            }
        }
    }
}
