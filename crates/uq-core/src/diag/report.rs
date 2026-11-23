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

/// How long one command may take.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(3);
/// The most output read from one command.
const MAX_OUTPUT: usize = 512 * 1024;
/// Lines of the crashed part's own log kept.
pub const LOG_TAIL: usize = 150;

/// The packages whose versions matter for uQ.
const PACKAGES: &[&str] = &[
    "uq",
    "uq-hyprland-pin",
    "hyprland",
    "quickshell",
    "qt6-base",
    "qt6-declarative",
    "mesa",
    "nvidia-utils",
    "bindle",
    "noti-fi",
    "kyko",
];

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

fn os_name(root: &Path) -> String {
    std::fs::read_to_string(root.join("etc/os-release"))
        .ok()
        .and_then(|t| {
            t.lines().find_map(|l| {
                l.strip_prefix("PRETTY_NAME=")
                    .map(|v| v.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| "unknown".into())
}

fn monitors() -> Vec<String> {
    let Some(text) = run("hyprctl", &["monitors", "-j"]) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    v.as_array()
        .into_iter()
        .flatten()
        .map(|m| {
            let s = |k: &str| {
                m.get(k)
                    .map(|x| x.to_string().trim_matches('"').to_string())
                    .unwrap_or_default()
            };
            format!(
                "{} {}x{}@{:.0} scale {} ({} {})",
                s("name"),
                s("width"),
                s("height"),
                m.get("refreshRate").and_then(|x| x.as_f64()).unwrap_or(0.0),
                s("scale"),
                s("make"),
                s("model"),
            )
        })
        .collect()
}

fn settings(paths: &Paths) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let state = std::fs::read_to_string(&paths.state_file)
        .ok()
        .and_then(|t| t.parse::<toml::Table>().ok());
    if let Some(s) = &state {
        for key in ["theme", "variant"] {
            if let Some(v) = s.get(key) {
                out.push((key.into(), v.to_string().trim_matches('"').to_string()));
            }
        }
        let mut sections: Vec<&String> = s.keys().filter(|k| s[*k].is_table()).collect();
        sections.sort();
        out.push((
            "sections set".into(),
            sections
                .into_iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", "),
        ));
    } else {
        out.push(("state.toml".into(), "missing or unreadable".into()));
    }
    if let Some(active) = crate::profile::load_active(&paths.profile_active) {
        out.push(("profile".into(), format!("{} ({})", active.name, active.id)));
    }
    match std::fs::read_to_string(&paths.user_lua) {
        Ok(t) => {
            let lines = t
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with("--"))
                .count();
            out.push(("user.lua".into(), format!("{lines} lines of your own")));
        }
        Err(_) => out.push(("user.lua".into(), "none".into())),
    }
    let names = |dir: &Path| -> String {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        if v.is_empty() {
            "none".into()
        } else {
            v.join(", ")
        }
    };
    out.push((
        "apps' Settings pages".into(),
        names(Path::new("/usr/share/uq/settings/pages")),
    ));
    out.push(("apps' window rules".into(), names(&paths.integrations_dir)));
    out
}

fn changes(paths: &Paths, n: usize) -> Vec<String> {
    Journal::open(paths.history_dir.clone())
        .list(n)
        .into_iter()
        .map(|e| format!("{} {} {}: {}", iso(e.time), e.origin, e.domain, e.summary))
        .collect()
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

/// Read everything for a report on `crash` (or on now, without one). `redact` cleans every
/// text that came from the computer.
pub fn gather(crash: Option<&Crash>, reg: &Registry, paths: &Paths, redact: &Redactor) -> Input {
    let made = chrono::Utc::now().timestamp();
    // The window: a quarter of an hour up to the crash (and a minute after, for the restart),
    // or the last half hour.
    let (since, until) = match crash {
        Some(c) => (iso(c.time - 15 * 60), Some(iso(c.time + 60))),
        None => (iso(made - 30 * 60), None),
    };
    let q = Query {
        since: Some(since.clone()),
        until: until.clone(),
        min: Severity::Warn,
        ..Query::default()
    };
    let problems = journal::group(journal::read(&q, reg).unwrap_or_default());
    let mut qs_report = None;
    let mut gpus = Vec::new();
    let mut log_tail = Vec::new();
    let mut backtrace = None;
    if let Some(c) = crash {
        if let Some(dir) = &c.qs_dir {
            if let Ok(text) = std::fs::read_to_string(dir.join("report.txt")) {
                gpus = parse_qs_report(&text).gpus;
                qs_report = Some(redact.apply(&text));
            }
            log_tail = qs_log_tail(dir, LOG_TAIL);
        } else if let Ok(text) = std::fs::read_to_string(c.dir.join("crash.json"))
            && let Some(rec) = serde_json::from_str(&text)
                .ok()
                .and_then(|v| super::log::PanicRecord::from_json(&v))
        {
            // A C++ app's crash handler can't format its backtrace into JSON: it is beside it.
            backtrace =
                Some(std::fs::read_to_string(c.dir.join("backtrace.txt")).unwrap_or(rec.backtrace));
        }
    }
    if gpus.is_empty() {
        gpus = gpus_from_sys();
    }
    let q_args: Vec<&str> = std::iter::once("-Q")
        .chain(PACKAGES.iter().copied())
        .collect();
    // `pacman -Q a b` exits 1 when any one is missing, after printing the rest.
    let packages = run_for("pacman", &q_args, COMMAND_TIMEOUT, true)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect();
    let journal_text = journal::read_text_all(&since, until.as_deref(), reg, 400);
    Input {
        made,
        crash: crash.cloned(),
        qs_report,
        backtrace: backtrace.map(|b| redact.apply(&b)),
        uq_version: env!("CARGO_PKG_VERSION").into(),
        packages,
        kernel: std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|s| s.trim().to_string())
            .unwrap_or_default(),
        os: os_name(Path::new("/")),
        gpus,
        monitors: monitors(),
        desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        settings: settings(paths)
            .into_iter()
            .map(|(k, v)| (k, redact.apply(&v)))
            .collect(),
        problems: problems
            .into_iter()
            .map(|mut e| {
                e.message = redact.apply(&e.message);
                e
            })
            .collect(),
        changes: changes(paths, 10)
            .into_iter()
            .map(|c| redact.apply(&c))
            .collect(),
        log_tail: log_tail.into_iter().map(|l| redact.apply(&l)).collect(),
        journal: redact.apply(&journal_text),
    }
}

/// `Driver …` per GPU from /sys, when there is no Quickshell report to say.
fn gpus_from_sys() -> Vec<String> {
    let mut out = Vec::new();
    for e in std::fs::read_dir("/sys/class/drm")
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let dev = e.path().join("device");
        let driver = std::fs::read_link(dev.join("driver"))
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "?".into());
        let read = |f: &str| {
            std::fs::read_to_string(dev.join(f))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };
        out.push(format!(
            "{name}: {driver} {}:{}",
            read("vendor"),
            read("device")
        ));
    }
    out.sort();
    out
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
