//! uQ's lines read back from the journal: `journalctl --user -o json` for uQ's units (and the
//! units apps list in their code files), each line given its code, severity and source.

use serde::Serialize;
use serde_json::Value;

use super::{Registry, Severity, code_in};

/// One journal line, or several identical ones folded together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// Microseconds since the epoch: the newest of the folded lines.
    pub time_us: i64,
    /// The oldest of the folded lines (the same as `time_us` for one line).
    pub first_us: i64,
    /// Which part of uQ said it: `uqd`, `shell`, `lock`, `uqlip`, or the unit's name.
    pub source: String,
    pub severity: Severity,
    pub code: Option<String>,
    /// The line without its level word, colours or trailing space.
    pub message: String,
    /// How many identical lines this stands for.
    pub count: u32,
}

/// What to read.
#[derive(Debug, Clone)]
pub struct Query {
    /// A `journalctl --since` value (`-1h`, `today`, `2026-11-22 10:00`); `None` is this boot.
    pub since: Option<String>,
    /// A `journalctl --until` value; `None` is now.
    pub until: Option<String>,
    /// The least serious line kept.
    pub min: Severity,
    /// Only lines with a code.
    pub coded_only: bool,
    /// Only this area (`HYPR`) or this code (`HYPR-003`).
    pub only: Option<String>,
    /// The most lines read from the journal (newest kept).
    pub limit: usize,
}

impl Default for Query {
    fn default() -> Self {
        Query {
            since: None,
            until: None,
            min: Severity::Warn,
            coded_only: false,
            only: None,
            limit: 20_000,
        }
    }
}

/// `uqd.service` → `uqd`, `uq-shell.service` → `shell`.
pub fn source_of(unit: &str) -> String {
    let u = unit.trim_end_matches(".service");
    match u {
        "uq-shell" => "shell".into(),
        "uq-lock" => "lock".into(),
        other => other.into(),
    }
}

/// Remove ANSI colour codes.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            if it.peek() == Some(&'[') {
                it.next();
                for d in it.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// A leading level word the way Quickshell and uQ's terminal style write it (`  WARN qml: …`,
/// `ERROR uqd: …`), and the rest of the line.
fn leading_level(s: &str) -> (Option<Severity>, &str) {
    let t = s.trim_start();
    let (word, rest) = t.split_once([' ', ':']).unwrap_or((t, ""));
    match word {
        "FATAL" | "CRITICAL" | "ERROR" => (
            Some(Severity::Error),
            rest.trim_start_matches(':').trim_start(),
        ),
        "WARN" | "WARNING" => (
            Some(Severity::Warn),
            rest.trim_start_matches(':').trim_start(),
        ),
        "INFO" => (
            Some(Severity::Info),
            rest.trim_start_matches(':').trim_start(),
        ),
        "DEBUG" | "TRACE" => (
            Some(Severity::Debug),
            rest.trim_start_matches(':').trim_start(),
        ),
        _ => (None, t),
    }
}

/// A journal field as text: journald sends non-UTF-8 values as arrays of bytes.
fn field(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) => Some(s.clone()),
        Value::Array(bytes) => {
            let b: Vec<u8> = bytes
                .iter()
                .filter_map(|x| x.as_u64().map(|n| n as u8))
                .collect();
            Some(String::from_utf8_lossy(&b).into_owned())
        }
        other => Some(other.to_string()),
    }
}

/// One `journalctl -o json` line as an [`Entry`]; `None` for a line without a message.
pub fn parse_line(line: &str, reg: &Registry) -> Option<Entry> {
    let v: Value = serde_json::from_str(line).ok()?;
    let raw = field(&v, "MESSAGE")?;
    let time_us = field(&v, "__REALTIME_TIMESTAMP")
        .and_then(|t| t.parse().ok())
        .unwrap_or(0);
    let priority: u8 = field(&v, "PRIORITY")
        .and_then(|p| p.parse().ok())
        .unwrap_or(6);
    let unit = field(&v, "_SYSTEMD_USER_UNIT")
        .or_else(|| field(&v, "_SYSTEMD_UNIT"))
        .or_else(|| field(&v, "SYSLOG_IDENTIFIER"))
        .unwrap_or_default();
    let clean = strip_ansi(&raw);
    let (word, rest) = leading_level(&clean);
    let message = rest.trim_end().to_string();
    let code = code_in(&message).map(str::to_string);
    let severity = code
        .as_deref()
        .and_then(|c| reg.get(c))
        .map(|c| c.severity)
        .or(word)
        .unwrap_or_else(|| Severity::from_priority(priority));
    // An app uQ started logs into uQ's unit (the shell's, usually) as `reelo: [REELO-001] …`:
    // name it rather than the unit.
    let app = code
        .as_deref()
        .and_then(|c| reg.get(c))
        .filter(|c| c.owner != "uQ")
        .and_then(|_| message.split_once(": "))
        .map(|(p, _)| p)
        .filter(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        });
    Some(Entry {
        time_us,
        first_us: time_us,
        source: app.map_or_else(|| source_of(&unit), str::to_string),
        severity,
        code,
        message,
        count: 1,
    })
}

/// Whether `e` passes `q`'s filters.
pub fn keep(e: &Entry, q: &Query) -> bool {
    if e.severity < q.min {
        return false;
    }
    if q.coded_only && e.code.is_none() {
        return false;
    }
    if let Some(only) = &q.only {
        let Some(code) = &e.code else { return false };
        let hit = if only.contains('-') {
            code.eq_ignore_ascii_case(only)
        } else {
            super::area(code).eq_ignore_ascii_case(only)
        };
        if !hit {
            return false;
        }
    }
    true
}

/// `journalctl` output, oldest first, filtered.
pub fn parse(text: &str, reg: &Registry, q: &Query) -> Vec<Entry> {
    text.lines()
        .filter_map(|l| parse_line(l, reg))
        .filter(|e| keep(e, q))
        .collect()
}

/// Fold runs of the same line (same source and message) into one, oldest first.
pub fn collapse_runs(entries: Vec<Entry>) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::with_capacity(entries.len());
    for e in entries {
        if let Some(last) = out.last_mut()
            && last.source == e.source
            && last.message == e.message
        {
            last.count += 1;
            last.time_us = e.time_us;
            continue;
        }
        out.push(e);
    }
    out
}

/// One entry per distinct problem (source and code, or the message for a line with none),
/// newest first: what the Problems page lists.
pub fn group(entries: Vec<Entry>) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for e in entries {
        let key = |x: &Entry| {
            (
                x.source.clone(),
                x.code.clone().unwrap_or_else(|| x.message.clone()),
            )
        };
        if let Some(g) = out.iter_mut().find(|g| key(g) == key(&e)) {
            g.count += e.count;
            g.first_us = g.first_us.min(e.first_us);
            if e.time_us >= g.time_us {
                g.time_us = e.time_us;
                // The newest wording: it names the latest file or window.
                g.message = e.message;
                g.severity = e.severity;
            }
            continue;
        }
        out.push(e);
    }
    out.sort_by_key(|e| std::cmp::Reverse(e.time_us));
    out
}

/// The `journalctl` arguments for `q` over `units`.
pub fn args(q: &Query, units: &[String]) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "--user".into(),
        "--no-pager".into(),
        "-o".into(),
        "json".into(),
        "-n".into(),
        q.limit.to_string(),
    ];
    match &q.since {
        Some(s) => {
            a.push("--since".into());
            a.push(s.clone());
        }
        None => a.push("-b".into()),
    }
    if let Some(u) = &q.until {
        a.push("--until".into());
        a.push(u.clone());
    }
    for u in units {
        a.push("-u".into());
        a.push(u.clone());
    }
    a
}

/// Read the journal. An error only when `journalctl` can't run at all; no lines is `Ok(vec![])`.
pub fn read(q: &Query, reg: &Registry) -> std::io::Result<Vec<Entry>> {
    if which("journalctl").is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "journalctl isn't installed",
        ));
    }
    let a = args(q, reg.units());
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    // journalctl exits 1 when no entry matches; that is no lines, not a failure.
    let out = super::report::run_for("journalctl", &a, READ_TIMEOUT, true).unwrap_or_default();
    Ok(parse(&out, reg, q))
}

/// How long reading the journal may take.
const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn which(program: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH")?
        .to_str()?
        .split(':')
        .map(|d| std::path::Path::new(d).join(program))
        .find(|p| p.is_file())
}

/// Raw lines (for a report's attachment): `journalctl --user -o short-iso` over uQ's units.
pub fn read_text(since: &str, until: Option<&str>, units: &[String], limit: usize) -> String {
    let mut a: Vec<String> = vec![
        "--user".into(),
        "--no-pager".into(),
        "-o".into(),
        "short-iso".into(),
        "-n".into(),
        limit.to_string(),
        "--since".into(),
        since.into(),
    ];
    if let Some(u) = until {
        a.push("--until".into());
        a.push(u.into());
    }
    for u in units {
        a.push("-u".into());
        a.push(u.clone());
    }
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    super::report::run_for("journalctl", &a, READ_TIMEOUT, true)
        .map(|o| strip_ansi(&o))
        .unwrap_or_default()
}
