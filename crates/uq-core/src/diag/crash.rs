//! Crashes uQ knows about: Quickshell's own reports (`$XDG_CACHE_HOME/quickshell/crashes/<run>/`,
//! written when the shell, the lock screen or another app's Quickshell part dies) and the
//! [`PanicRecord`]s uQ's Rust programs leave in `~/.local/state/uq/crashes`.
//!
//! Each crash gets a folder of its own under uQ's crash folder, holding the report people
//! read (`report.md`). Its existence means the crash was seen; `dismissed` hides it from the
//! Problems page.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::log::{PanicRecord, crash_dir_from};

/// What crashed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "name", rename_all = "lowercase")]
pub enum Kind {
    /// uQ's shell (`/usr/share/uq/shell`, or a checkout's `shell/`).
    Shell,
    /// uQ's lock screen.
    Lock,
    /// uQ's login screen.
    Greeter,
    /// Another app's Quickshell config, by the name of its folder (`notifd`).
    App(String),
    /// One of uQ's Rust programs (`uqd`, `uqlip`, …).
    Program(String),
}

impl Kind {
    /// The code its log line and Problems entry carry.
    pub fn code(&self) -> &'static str {
        match self {
            Kind::Shell => "CRASH-001",
            Kind::Lock | Kind::Greeter => "CRASH-002",
            Kind::App(_) => "CRASH-003",
            Kind::Program(_) => "CRASH-004",
        }
    }

    /// What to call it in a sentence: "uQ's shell", "noti-fi's notification daemon".
    pub fn label(&self) -> String {
        match self {
            Kind::Shell => "uQ's shell".into(),
            Kind::Lock => "uQ's lock screen".into(),
            Kind::Greeter => "uQ's login screen".into(),
            Kind::App(name) => app_label(name),
            Kind::Program(p) => p.clone(),
        }
    }
}

/// Known Quickshell folders of other apps, by folder name.
fn app_label(folder: &str) -> String {
    match folder {
        "notifd" => "noti-fi's notification daemon".into(),
        other => format!("{other} (a Quickshell app)"),
    }
}

/// What Quickshell's `report.txt` says.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct QsReport {
    pub quickshell: String,
    pub qt: String,
    pub signal: String,
    pub pid: String,
    pub run_id: String,
    pub config_path: String,
    pub frames: Vec<String>,
    /// `Driver: … / Model: …` per GPU.
    pub gpus: Vec<String>,
}

/// Parse a Quickshell `report.txt`.
pub fn parse_qs_report(text: &str) -> QsReport {
    let mut r = QsReport::default();
    let mut section = "";
    for line in text.lines() {
        if let Some(s) = line
            .strip_prefix("===== ")
            .and_then(|s| s.strip_suffix(" ====="))
        {
            section = match s {
                "Version Information" => "version",
                "System Information" => "system",
                "Instance Information" => "instance",
                "Stacktrace" => "stack",
                _ => "",
            };
            continue;
        }
        let kv = |p: &str| line.strip_prefix(p).map(|v| v.trim().to_string());
        match section {
            "version" => {
                if let Some(v) = kv("Quickshell:") {
                    r.quickshell = v;
                } else if let Some(v) = kv("Qt:") {
                    r.qt = v;
                }
            }
            "system" => {
                if let Some(v) = kv("  Driver:") {
                    r.gpus.push(v);
                }
            }
            "instance" => {
                if let Some(v) = kv("Signal:") {
                    r.signal = v;
                } else if let Some(v) = kv("Crashed process ID:") {
                    r.pid = v;
                } else if let Some(v) = kv("Run ID:") {
                    r.run_id = v;
                } else if let Some(v) = kv("Config Path:") {
                    r.config_path = v;
                }
            }
            "stack" if line.starts_with('#') => r.frames.push(line.trim().to_string()),
            _ => {}
        }
    }
    r
}

/// Which Quickshell config a path is.
pub fn classify(config_path: &str) -> Kind {
    let p = config_path
        .trim_end_matches("/shell.qml")
        .trim_end_matches('/');
    let mut parts = p.rsplit('/');
    let last = parts.next().unwrap_or("");
    let parent = parts.next().unwrap_or("");
    match (parent, last) {
        ("uq", "shell") => Kind::Shell,
        ("uq", "lock") => Kind::Lock,
        ("uq", "greeter") => Kind::Greeter,
        (_, name) if !name.is_empty() => Kind::App(name.to_string()),
        _ => Kind::App("unknown".into()),
    }
}

/// Where a C++ crash happened, from uq-diag's `backtrace.txt` (glibc's `backtrace_symbols_fd`
/// lines, `/usr/lib/libQt6Core.so.6(_ZN…+0x72) [0x7f…]`): the frames after the signal
/// trampoline (the first libc frame with no name), as `name (lib)` or `lib+offset`.
pub fn glibc_frames(text: &str, n: usize) -> Vec<String> {
    let frames: Vec<(String, String)> = text
        .lines()
        .filter_map(|l| {
            let (path, rest) = l.split_once('(')?;
            let inner = rest.split_once(')')?.0;
            let lib = path.rsplit('/').next().unwrap_or(path).to_string();
            Some((lib, inner.to_string()))
        })
        .collect();
    let start = frames
        .iter()
        .position(|(lib, inner)| lib.starts_with("libc.so") && inner.starts_with('+'))
        .map_or(0, |i| i + 1);
    frames[start.min(frames.len())..]
        .iter()
        .take(n)
        .map(|(lib, inner)| match inner.split_once('+') {
            Some((name, _)) if !name.is_empty() => format!("{name} ({lib})"),
            _ => format!("{lib}{inner}"),
        })
        .collect()
}

/// The frames that say where it happened: libc's and the signal machinery's own are skipped,
/// and an address-only frame is kept only if nothing better follows.
pub fn telling_frames(frames: &[String], n: usize) -> Vec<String> {
    let named: Vec<&String> = frames
        .iter()
        .filter(|f| f.contains(" in "))
        .filter(|f| {
            ![
                "raise",
                "abort",
                "std::terminate",
                "__cxa_",
                "__pthread",
                "QMessageLogger",
            ]
            .iter()
            .any(|s| f.contains(s))
        })
        .collect();
    named
        .into_iter()
        .take(n)
        .map(|f| {
            // `#3  0x… in QFile::QFile(QString const&) at /usr/lib/libQt6Core.so.6`
            let body = f.split_once(" in ").map_or(f.as_str(), |(_, b)| b);
            let (func, lib) = body.split_once(" at ").unwrap_or((body, ""));
            let lib = lib.rsplit('/').next().unwrap_or(lib);
            if lib.is_empty() {
                func.to_string()
            } else {
                format!("{func} ({lib})")
            }
        })
        .collect()
}

/// One crash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Crash {
    /// `qs-<run id>` or the panic folder's name; also the folder under uQ's crash folder.
    pub id: String,
    /// Seconds since the epoch.
    pub time: i64,
    #[serde(flatten)]
    pub kind: Kind,
    pub code: &'static str,
    /// "Segmentation fault (11)", or the panic's message.
    pub reason: String,
    /// Where: the first telling frames, or the panic's source line.
    pub place: Vec<String>,
    /// Quickshell's own folder (with `report.txt` and `log.qslog.log`), for a Quickshell crash.
    pub qs_dir: Option<PathBuf>,
    /// uQ's folder for this crash.
    pub dir: PathBuf,
    /// Whether `report.md` has been written (the crash was seen).
    pub reported: bool,
    pub dismissed: bool,
}

impl Crash {
    pub fn title(&self) -> String {
        format!("{} crashed", self.kind.label())
    }

    pub fn report_path(&self) -> PathBuf {
        self.dir.join("report.md")
    }
}

/// Where crashes are found.
#[derive(Debug, Clone)]
pub struct Dirs {
    /// `$XDG_CACHE_HOME/quickshell/crashes`.
    pub quickshell: PathBuf,
    /// `$XDG_STATE_HOME/uq/crashes`.
    pub uq: PathBuf,
}

impl Dirs {
    pub fn from_env() -> Dirs {
        Dirs::from_vars(&|k| std::env::var(k).ok())
    }

    pub fn from_vars(get: &dyn Fn(&str) -> Option<String>) -> Dirs {
        let home = PathBuf::from(get("HOME").unwrap_or_else(|| "/tmp".into()));
        let cache = get("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".cache"));
        Dirs {
            quickshell: cache.join(crate::uq_consts::paths::QUICKSHELL_CRASHES_DIR),
            uq: crash_dir_from(get),
        }
    }
}

fn mtime(p: &Path) -> i64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

/// Every crash, newest first. A Quickshell folder still being written (no `report.txt` yet) is
/// left for the next look.
pub fn scan(dirs: &Dirs) -> Vec<Crash> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dirs.quickshell) {
        for e in rd.flatten() {
            let qs_dir = e.path();
            let report = qs_dir.join("report.txt");
            let Ok(text) = std::fs::read_to_string(&report) else {
                continue;
            };
            let r = parse_qs_report(&text);
            let run = e.file_name().to_string_lossy().into_owned();
            let id = format!("qs-{run}");
            let dir = dirs.uq.join(&id);
            let kind = classify(&r.config_path);
            out.push(Crash {
                time: mtime(&report),
                code: kind.code(),
                kind,
                reason: if r.signal.is_empty() {
                    "crashed".into()
                } else {
                    r.signal.clone()
                },
                place: telling_frames(&r.frames, 3),
                qs_dir: Some(qs_dir),
                reported: dir.join("report.md").exists(),
                dismissed: dir.join("dismissed").exists(),
                dir,
                id,
            });
        }
    }
    if let Ok(rd) = std::fs::read_dir(&dirs.uq) {
        for e in rd.flatten() {
            let dir = e.path();
            let Ok(text) = std::fs::read_to_string(dir.join("crash.json")) else {
                continue;
            };
            let Some(rec) = serde_json::from_str(&text)
                .ok()
                .and_then(|v| PanicRecord::from_json(&v))
            else {
                continue;
            };
            let kind = Kind::Program(rec.program.clone());
            out.push(Crash {
                id: e.file_name().to_string_lossy().into_owned(),
                time: rec.time,
                code: kind.code(),
                kind,
                reason: rec.message.clone(),
                place: if rec.location.is_empty() {
                    std::fs::read_to_string(dir.join("backtrace.txt"))
                        .map(|t| glibc_frames(&t, 3))
                        .unwrap_or_default()
                } else {
                    vec![rec.location.clone()]
                },
                qs_dir: None,
                reported: dir.join("report.md").exists(),
                dismissed: dir.join("dismissed").exists(),
                dir,
            });
        }
    }
    out.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| a.id.cmp(&b.id)));
    out
}

/// A crash by id; `latest` is the newest.
pub fn find(dirs: &Dirs, id: &str) -> Option<Crash> {
    let all = scan(dirs);
    if id == "latest" {
        return all.into_iter().next();
    }
    all.into_iter()
        .find(|c| c.id == id || c.id == format!("qs-{id}"))
}

/// Hide a crash from the Problems page (its files stay).
pub fn dismiss(c: &Crash) -> std::io::Result<()> {
    std::fs::create_dir_all(&c.dir)?;
    std::fs::write(c.dir.join("dismissed"), b"")
}
