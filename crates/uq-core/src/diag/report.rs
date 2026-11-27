//! The report a person pastes into an issue (or hands to an agent): one Markdown file with
//! what happened, the versions, the session, uQ's settings that matter, the problems logged
//! around it, the last setting changes and the last lines the crashed part logged.
//! [`gather`] reads the computer; [`render`] only formats, so it is tested on fixed input.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::crash::{Crash, parse_qs_report};
use super::journal::{self, Entry, Query};
use super::redact::Redactor;
use super::{Registry, Severity};
use crate::history::Journal;
use crate::paths::Paths;
mod gather;
pub use gather::*;

/// How long one command may take.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(3);
/// The most output read from one command.
const MAX_OUTPUT: usize = 512 * 1024;
/// Lines of the crashed part's own log kept.
pub const LOG_TAIL: usize = 150;

/// Run `program args` with a short timeout; its standard output when it exits 0.
pub fn run(program: &str, args: &[&str]) -> Option<String> {
    run_for(program, args, COMMAND_TIMEOUT, false)
}

/// Run `program args`, stopping it after `timeout`; its standard output when it exits 0 (or
/// whatever the exit, with `any_exit`).
pub fn run_for(program: &str, args: &[&str], timeout: Duration, any_exit: bool) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    // Read while waiting, so a chatty command can't fill the pipe and stall.
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = (&mut stdout).take(MAX_OUTPUT as u64).read_to_end(&mut out);
        out
    });
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = reader.join().unwrap_or_default();
                return (status.success() || any_exit)
                    .then(|| String::from_utf8_lossy(&out).into_owned());
            }
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Everything a report shows, already read.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Input {
    /// Seconds since the epoch, when the report was made.
    pub made: i64,
    pub crash: Option<Crash>,
    /// The crash's Quickshell `report.txt` (redacted), when it has one.
    pub qs_report: Option<String>,
    /// The crashed program's backtrace (a panic).
    pub backtrace: Option<String>,
    pub uq_version: String,
    /// `name version` per installed package.
    pub packages: Vec<String>,
    pub kernel: String,
    pub os: String,
    pub gpus: Vec<String>,
    /// `NAME WxH@Hz scale S` per screen.
    pub monitors: Vec<String>,
    pub desktop: String,
    /// theme, variant, active profile, user.lua size, apps with uQ pages and so on.
    pub settings: Vec<(String, String)>,
    /// Problems logged in the window, grouped.
    pub problems: Vec<Entry>,
    /// The last setting changes: `time origin domain: summary`.
    pub changes: Vec<String>,
    /// The crashed part's last lines (Quickshell's log, decoded).
    pub log_tail: Vec<String>,
    /// uQ's journal lines in the window.
    pub journal: String,
}

/// `2026-11-22 11:02:13` in local time.
pub fn iso(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| secs.to_string())
}

/// Quickshell's binary log decoded by `qs log`, last `n` lines.
pub fn qs_log_tail(qs_dir: &Path, n: usize) -> Vec<String> {
    let log = qs_dir.join("log.qslog.log");
    if !log.exists() {
        return Vec::new();
    }
    let path = log.to_string_lossy().into_owned();
    let tail = n.to_string();
    run("qs", &["log", "--tail", &tail, &path])
        .map(|t| {
            journal::strip_ansi(&t)
                .lines()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn fence(out: &mut String, lang: &str, body: &str) {
    // A body holding ``` can't close the block early with four.
    let ticks = if body.contains("```") { "````" } else { "```" };
    out.push_str(ticks);
    out.push_str(lang);
    out.push('\n');
    out.push_str(body.trim_end());
    out.push('\n');
    out.push_str(ticks);
    out.push_str("\n\n");
}

/// The report as Markdown.
pub fn render(i: &Input, reg: &Registry) -> String {
    let mut o = String::new();
    match &i.crash {
        Some(c) => {
            o.push_str(&format!("# {} — uQ crash report\n\n", c.title()));
            o.push_str(&format!(
                "- **Code:** {} ({})\n- **When:** {}\n- **What:** {}\n",
                c.code,
                reg.get(c.code).map_or("", |x| x.title.as_str()),
                iso(c.time),
                c.reason
            ));
            if !c.place.is_empty() {
                o.push_str(&format!("- **Where:** {}\n", c.place.join(" ← ")));
            }
            o.push_str(&format!("- **Crash id:** `{}`\n", c.id));
        }
        None => o.push_str("# uQ problem report\n\n"),
    }
    o.push_str(&format!("- **Report made:** {}\n\n", iso(i.made)));
    o.push_str(
        "Window titles, your home folder, user and computer names, and e-mail and network \
                addresses are taken out of this report.\n\n",
    );

    o.push_str("## Versions\n\n");
    o.push_str(&format!("- uQ {}\n", i.uq_version));
    for p in &i.packages {
        o.push_str(&format!("- {p}\n"));
    }
    o.push_str(&format!("- Linux {} on {}\n", i.kernel, i.os));
    for g in &i.gpus {
        o.push_str(&format!("- GPU: {g}\n"));
    }
    o.push('\n');

    o.push_str("## Session\n\n");
    o.push_str(&format!(
        "- Desktop: {}\n",
        if i.desktop.is_empty() {
            "(not in a desktop session)"
        } else {
            &i.desktop
        }
    ));
    for m in &i.monitors {
        o.push_str(&format!("- Screen: {m}\n"));
    }
    for (k, v) in &i.settings {
        o.push_str(&format!("- {k}: {v}\n"));
    }
    o.push('\n');

    o.push_str(if i.crash.is_some() {
        "## Problems logged in the 15 minutes before\n\n"
    } else {
        "## Problems logged in the last 30 minutes\n\n"
    });
    if i.problems.is_empty() {
        o.push_str("None.\n\n");
    } else {
        for e in i.problems.iter().take(30) {
            let code = e.code.as_deref().unwrap_or("—");
            let times = if e.count > 1 {
                format!(" (×{})", e.count)
            } else {
                String::new()
            };
            let t = iso(e.time_us / 1_000_000);
            o.push_str(&format!(
                "- `{code}` {} {t} {}: {}{times}\n",
                e.severity.as_str(),
                e.source,
                e.message
            ));
        }
        o.push('\n');
    }

    o.push_str("## Last setting changes\n\n");
    if i.changes.is_empty() {
        o.push_str("None recorded.\n\n");
    } else {
        for c in &i.changes {
            o.push_str(&format!("- {c}\n"));
        }
        o.push('\n');
    }

    if let Some(c) = &i.crash
        && !i.log_tail.is_empty()
    {
        o.push_str(&format!(
            "## The last {} lines {} logged\n\n",
            i.log_tail.len(),
            c.kind.label()
        ));
        fence(&mut o, "text", &i.log_tail.join("\n"));
    }
    if let Some(b) = &i.backtrace {
        o.push_str("## Backtrace\n\n");
        fence(&mut o, "text", b);
    }
    if let Some(r) = &i.qs_report {
        o.push_str("## Quickshell's crash report\n\n");
        fence(&mut o, "text", r);
    }
    if !i.journal.trim().is_empty() {
        o.push_str("## uQ's journal\n\n");
        fence(&mut o, "text", &i.journal);
    }
    o
}

/// `~/.local/state/uq/reports`: reports made by hand (`uqctl report`, Copy report without a crash).
pub fn reports_dir(paths: &Paths) -> PathBuf {
    paths
        .state_file
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
        .join("reports")
}

/// Write the report for `crash` to its folder (`report.md`); the path.
pub fn save_crash(c: &Crash, text: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(&c.dir)?;
    let p = c.report_path();
    crate::atomic::atomic_write(&p, text)?;
    Ok(p)
}

/// Write a report made by hand under [`reports_dir`]; the path.
pub fn save_now(paths: &Paths, made: i64, text: &str) -> std::io::Result<PathBuf> {
    let dir = reports_dir(paths);
    std::fs::create_dir_all(&dir)?;
    let stamp = chrono::DateTime::from_timestamp(made, 0)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%Y%m%d-%H%M%S")
                .to_string()
        })
        .unwrap_or_else(|| made.to_string());
    let p = dir.join(format!("uq-report-{stamp}.md"));
    crate::atomic::atomic_write(&p, text)?;
    Ok(p)
}

/// Gather, render and save the report for `crash`; its path.
pub fn write_for(c: &Crash, reg: &Registry, paths: &Paths) -> std::io::Result<PathBuf> {
    let input = gather(Some(c), reg, paths, &Redactor::from_env());
    save_crash(c, &render(&input, reg))
}
