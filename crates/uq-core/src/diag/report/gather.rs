//! Gathering what a report holds: OS, packages, monitors, GPUs, settings and recent changes.

use super::*;

/// The packages whose versions matter for uQ.
pub(crate) const PACKAGES: &[&str] = &[
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

pub(crate) fn os_name(root: &Path) -> String {
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

pub(crate) fn monitors() -> Vec<String> {
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

pub(crate) fn settings(paths: &Paths) -> Vec<(String, String)> {
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

pub(crate) fn changes(paths: &Paths, n: usize) -> Vec<String> {
    Journal::open(paths.history_dir.clone())
        .list(n)
        .into_iter()
        .map(|e| format!("{} {} {}: {}", iso(e.time), e.origin, e.domain, e.summary))
        .collect()
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
                .and_then(|v| super::super::log::PanicRecord::from_json(&v))
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
pub(crate) fn gpus_from_sys() -> Vec<String> {
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
