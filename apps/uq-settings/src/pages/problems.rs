//! The Problems page (docs/diagnostics.md): crashes, each with a report to copy or open, and the
//! warnings and errors uQ (and apps with code files) logged lately, one card per problem with
//! what it means and what to do. Everything is read from files and the journal; nothing here
//! needs uqd.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify};
use uq_core::diag::crash::{self, Crash, Dirs};
use uq_core::diag::journal::{self, Entry, Query};
use uq_core::diag::redact::Redactor;
use uq_core::diag::{Registry, Severity, report};
use uq_core::integration::watch::watch_dirs;
use uq_core::paths::Paths;

use super::runner::PageLogic;
use super::sysbus::failure;

/// How far back the page looks unless asked otherwise.
pub const DEFAULT_HOURS: u32 = 24;
/// The journal is read again this often while the page is open.
const JOURNAL_EVERY: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    /// Copy a crash's report, or a report on now with `id` "".
    Copy {
        id: String,
    },
    /// Open a crash's report (or a new report on now) in the text editor.
    Open {
        id: String,
    },
    Dismiss {
        id: String,
    },
    /// Look back this many hours (1 to 168).
    SetHours {
        hours: u32,
    },
}

pub struct ProblemsState {
    dirs: Dirs,
    paths: Paths,
    reg: Registry,
    hours: AtomicU32,
    write: Mutex<()>,
    notice: StdMutex<(u64, String)>,
    /// Puts text on the clipboard; `wl-copy` normally.
    copy: fn(&str) -> Result<(), String>,
    /// Opens a file; `xdg-open` normally.
    open: fn(&std::path::Path) -> Result<(), String>,
    /// Reads the journal; `journalctl` normally.
    read: fn(&Query, &Registry) -> std::io::Result<Vec<Entry>>,
}

/// `wl-copy`, which keeps serving the text after it returns.
pub fn wl_copy(text: &str) -> Result<(), String> {
    let mut child = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "copying needs wl-clipboard, which isn't installed".to_string()
            } else {
                format!("couldn't start wl-copy: {e}")
            }
        })?;
    child
        .stdin
        .take()
        .ok_or("wl-copy has no input")?
        .write_all(text.as_bytes())
        .map_err(|e| format!("couldn't copy: {e}"))?;
    match child.wait() {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("wl-copy failed ({s})")),
        Err(e) => Err(format!("wl-copy failed: {e}")),
    }
}

pub fn xdg_open(path: &std::path::Path) -> Result<(), String> {
    Command::new("xdg-open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
        .map_err(|e| format!("couldn't open the report: {e}"))
}

impl ProblemsState {
    pub fn new(_conn: zbus::Connection) -> Self {
        Self::with(Dirs::from_env(), Paths::from_env(), Registry::installed())
    }

    pub fn with(dirs: Dirs, paths: Paths, reg: Registry) -> Self {
        ProblemsState {
            dirs,
            paths,
            reg,
            hours: AtomicU32::new(DEFAULT_HOURS),
            write: Mutex::new(()),
            notice: StdMutex::new((0, String::new())),
            copy: wl_copy,
            open: xdg_open,
            read: journal::read,
        }
    }

    #[cfg(test)]
    pub fn with_io(
        mut self,
        copy: fn(&str) -> Result<(), String>,
        open: fn(&std::path::Path) -> Result<(), String>,
        read: fn(&Query, &Registry) -> std::io::Result<Vec<Entry>>,
    ) -> Self {
        self.copy = copy;
        self.open = open;
        self.read = read;
        self
    }

    fn say(&self, text: impl Into<String>) {
        let mut n = self.notice.lock().unwrap();
        *n = (n.0 + 1, text.into());
    }

    /// [`report_for`] off the async threads (it reads files and runs commands).
    async fn report(&self, id: String) -> zbus::Result<(std::path::PathBuf, String)> {
        let (d, p, r) = (self.dirs.clone(), self.paths.clone(), self.reg.clone());
        tokio::task::spawn_blocking(move || report_for(&d, &p, &r, &id))
            .await
            .map_err(|e| failure(e.to_string()))?
            .map_err(failure)
    }
}

/// The saved report for crash `id` (written now if it hasn't been), or a new one on now for "".
/// Its path and text.
pub fn report_for(
    dirs: &Dirs,
    paths: &Paths,
    reg: &Registry,
    id: &str,
) -> Result<(std::path::PathBuf, String), String> {
    if id.is_empty() {
        let input = report::gather(None, reg, paths, &Redactor::from_env());
        let text = report::render(&input, reg);
        let p = report::save_now(paths, input.made, &text).map_err(|e| e.to_string())?;
        return Ok((p, text));
    }
    let c = crash::find(dirs, id).ok_or("that crash is no longer there")?;
    let p = c.report_path();
    let text = match std::fs::read_to_string(&p) {
        Ok(t) => t,
        Err(_) => {
            report::write_for(&c, reg, paths).map_err(|e| e.to_string())?;
            std::fs::read_to_string(&p).map_err(|e| e.to_string())?
        }
    };
    Ok((p, text))
}

fn crash_json(c: &Crash, reg: &Registry) -> Value {
    json!({
        "id": c.id,
        "time": c.time,
        "code": c.code,
        "title": c.title(),
        "reason": c.reason,
        "place": c.place,
        "kind": c.kind,
        "fix": reg.get(c.code).map_or("", |x| x.fix.as_str()),
    })
}

fn problem_json(e: &Entry, reg: &Registry) -> Value {
    let code = e.code.as_deref().and_then(|c| reg.get(c));
    json!({
        "time": e.time_us / 1_000_000,
        "first": e.first_us / 1_000_000,
        "count": e.count,
        "code": e.code,
        "severity": e.severity.as_str(),
        "source": e.source,
        "message": e.message,
        "title": code.map_or("", |c| c.title.as_str()),
        "means": code.map_or("", |c| c.means.as_str()),
        "fix": code.map_or("", |c| c.fix.as_str()),
        "owner": code.map_or("", |c| c.owner.as_str()),
    })
}

/// The page's state from what was read.
pub fn state(crashes: &[Crash], entries: Vec<Entry>, reg: &Registry, hours: u32) -> Value {
    let (coded, other): (Vec<Entry>, Vec<Entry>) =
        entries.into_iter().partition(|e| e.code.is_some());
    let grouped = journal::group(coded);
    let errors = grouped
        .iter()
        .filter(|e| e.severity >= Severity::Error)
        .count();
    json!({
        "hours": hours,
        "crashes": crashes.iter().filter(|c| !c.dismissed).map(|c| crash_json(c, reg)).collect::<Vec<_>>(),
        "problems": grouped.iter().map(|e| problem_json(e, reg)).collect::<Vec<_>>(),
        "errors": errors,
        "other": other.iter().map(|e| e.count).sum::<u32>(),
    })
}

impl PageLogic for ProblemsState {
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
            Op::SetHours { hours } => {
                self.hours.store(hours.clamp(1, 168), Ordering::Relaxed);
                Ok(())
            }
            Op::Dismiss { id } => {
                let c = crash::find(&self.dirs, &id)
                    .ok_or_else(|| failure("that crash is no longer there"))?;
                crash::dismiss(&c).map_err(|e| failure(e.to_string()))
            }
            Op::Copy { id } => {
                let (path, text) = self.report(id).await?;
                match (self.copy)(&text) {
                    Ok(()) => {
                        self.say("Report copied. Paste it into an issue or a message; window titles and names are already taken out.");
                        Ok(())
                    }
                    Err(e) => Err(failure(format!(
                        "{e}. The report is saved in {}",
                        path.display()
                    ))),
                }
            }
            Op::Open { id } => {
                let (path, _) = self.report(id).await?;
                (self.open)(&path).map_err(failure)
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let hours = self.hours.load(Ordering::Relaxed);
        let (dirs, reg, read) = (self.dirs.clone(), self.reg.clone(), self.read);
        let mut v = tokio::task::spawn_blocking(move || {
            let q = Query {
                since: Some(format!("-{hours}h")),
                min: Severity::Warn,
                ..Query::default()
            };
            let entries = read(&q, &reg).unwrap_or_default();
            state(&crash::scan(&dirs), entries, &reg, hours)
        })
        .await
        .map_err(|e| failure(e.to_string()))?;
        let (seq, text) = self.notice.lock().unwrap().clone();
        v["notice"] = json!({"seq": seq, "text": text});
        Ok(v)
    }

    /// New crash folders as they appear, and the journal every half minute.
    fn watch(&self, changed: Arc<Notify>) -> impl Future<Output = ()> + Send {
        let dirs = vec![self.dirs.quickshell.clone(), self.dirs.uq.clone()];
        async move {
            let mut rx = watch_dirs(dirs, Duration::from_secs(1)).ok();
            let mut tick = tokio::time::interval(JOURNAL_EVERY);
            tick.tick().await;
            loop {
                tokio::select! {
                    got = async {
                        match &mut rx {
                            Some(r) => r.recv().await,
                            None => std::future::pending().await,
                        }
                    } => {
                        if got.is_none() {
                            rx = None;
                        }
                    }
                    _ = tick.tick() => {}
                }
                changed.notify_one();
            }
        }
    }
}

#[cfg(test)]
mod tests;
