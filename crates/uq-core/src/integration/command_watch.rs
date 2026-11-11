//! `watch` on a settings page's `[[command]]`: re-run a read when a file changes or a D-Bus
//! signal arrives, so a page doesn't poll (docs/settings-pages.md, "Following changes").
//!
//! ```toml
//! [[command]]
//! id = "status"
//! argv = ["/usr/bin/example", "status", "--json"]
//! output = "json"
//! watch = { files = ["~/.config/example/config.toml"],
//!           signals = [{ bus = "system", interface = "org.example.Example1", member = "Changed" }] }
//! ```
//!
//! Narrow on purpose: a watched command takes no `args` and has no `confirm` (it is a read);
//! at most [`MAX_WATCH_FILES`] files and [`MAX_WATCH_SIGNALS`] signal matches. Files are
//! followed with inotify on their folder (so an editor's save-by-rename is seen, and a file or
//! folder that doesn't exist yet is picked up when it appears). Signals are a match rule on the
//! given bus: type signal, interface and member, and optionally path and sender. Every burst of
//! changes becomes one report once quiet for the debounce time. Nothing is read: the host only
//! re-runs the declared command.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use inotify::{Inotify, WatchDescriptor, WatchMask};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use zbus::names::{InterfaceName, MemberName};
use zbus::zvariant::ObjectPath;

use super::manifest::{Bus, valid_bus_name};

/// At most this many files per watched command.
pub const MAX_WATCH_FILES: usize = 8;
/// At most this many signal matches per watched command.
pub const MAX_WATCH_SIGNALS: usize = 8;
/// The quiet time after the last change before the command is re-run.
pub const WATCH_DEBOUNCE: Duration = Duration::from_millis(500);
const MAX_PATH_BYTES: usize = 1024;

/// A command's `watch` table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WatchSpec {
    /// Absolute paths, or `~/…` under the person's home folder.
    pub files: Vec<String>,
    pub signals: Vec<SignalMatch>,
}

/// One D-Bus signal that re-runs the command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalMatch {
    pub bus: Bus,
    pub interface: String,
    pub member: String,
    /// Only signals from this object path.
    pub path: Option<String>,
    /// Only signals from this (well-known) bus name.
    pub sender: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawWatch {
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    signals: Vec<RawSignal>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSignal {
    #[serde(default)]
    bus: Bus,
    interface: String,
    member: String,
    path: Option<String>,
    sender: Option<String>,
}

/// A watched file: absolute or `~/…`, already normalized (no `.`, `..`, `//` or trailing `/`),
/// at most 1024 bytes, no control characters.
pub fn valid_watch_path(path: &str) -> bool {
    let rest = match path.strip_prefix("~/") {
        Some(r) => r,
        None => match path.strip_prefix('/') {
            Some(r) => r,
            None => return false,
        },
    };
    path.len() <= MAX_PATH_BYTES
        && !path.chars().any(char::is_control)
        && rest
            .split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

/// `path` with a leading `~/` replaced by `home`.
pub fn expand(path: &str, home: &Path) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(path),
    }
}

/// Validate a `watch` table of a command with `has_args` / `has_confirm`.
pub(crate) fn watch_from(
    raw: RawWatch,
    has_args: bool,
    has_confirm: bool,
) -> Result<WatchSpec, String> {
    if has_args || has_confirm {
        return Err("watch is for reads: a watched command takes no args and no confirm".into());
    }
    if raw.files.is_empty() && raw.signals.is_empty() {
        return Err("watch needs files or signals".into());
    }
    if raw.files.len() > MAX_WATCH_FILES || raw.signals.len() > MAX_WATCH_SIGNALS {
        return Err(format!(
            "watch: at most {MAX_WATCH_FILES} files and {MAX_WATCH_SIGNALS} signals"
        ));
    }
    if let Some(f) = raw.files.iter().find(|f| !valid_watch_path(f)) {
        return Err(format!(
            "watch.files: {f:?} must be an absolute path or start with ~/, without . or .."
        ));
    }
    let mut signals = Vec::new();
    for s in raw.signals {
        if InterfaceName::try_from(s.interface.as_str()).is_err() {
            return Err(format!(
                "watch.signals: {:?} is not an interface name",
                s.interface
            ));
        }
        if MemberName::try_from(s.member.as_str()).is_err() {
            return Err(format!(
                "watch.signals: {:?} is not a member name",
                s.member
            ));
        }
        if let Some(p) = &s.path
            && ObjectPath::try_from(p.as_str()).is_err()
        {
            return Err(format!("watch.signals: {p:?} is not an object path"));
        }
        if let Some(n) = &s.sender
            && !valid_bus_name(n)
        {
            return Err(format!("watch.signals: sender {n:?} is not a bus name"));
        }
        signals.push(SignalMatch {
            bus: s.bus,
            interface: s.interface,
            member: s.member,
            path: s.path,
            sender: s.sender,
        });
    }
    Ok(WatchSpec {
        files: raw.files,
        signals,
    })
}

// ---------------------------------------------------------------------------------------------
// Running a watch

/// A running watch: a `()` arrives on `changes` after each quiet burst. Dropping it stops
/// everything it started.
pub struct Watch {
    pub changes: mpsc::Receiver<()>,
    tasks: Vec<JoinHandle<()>>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.tasks.iter().for_each(JoinHandle::abort);
    }
}

fn mask() -> WatchMask {
    WatchMask::CREATE
        | WatchMask::DELETE
        | WatchMask::MOVED_FROM
        | WatchMask::MOVED_TO
        | WatchMask::CLOSE_WRITE
        | WatchMask::MODIFY
        | WatchMask::ATTRIB
        | WatchMask::DELETE_SELF
        | WatchMask::MOVE_SELF
}

/// For each file: the folder to watch now and the entry name in it that matters. That is the
/// file's own folder and name, or, while the folder is missing, its nearest existing ancestor
/// and the next folder on the way.
pub fn file_targets(files: &[PathBuf]) -> Vec<(PathBuf, OsString)> {
    let mut out: Vec<(PathBuf, OsString)> = Vec::new();
    for f in files {
        let mut child = f.as_path();
        let target = loop {
            let Some(parent) = child.parent() else {
                break None;
            };
            if parent.is_dir() {
                break child
                    .file_name()
                    .map(|n| (parent.to_path_buf(), n.to_owned()));
            }
            child = parent;
        };
        if let Some(t) = target
            && !out.contains(&t)
        {
            out.push(t);
        }
    }
    out
}

fn arm(
    watches: &mut inotify::Watches,
    files: &[PathBuf],
) -> HashMap<WatchDescriptor, HashSet<OsString>> {
    let mut map: HashMap<WatchDescriptor, HashSet<OsString>> = HashMap::new();
    for (dir, name) in file_targets(files) {
        // A folder that vanished in between is caught by the next event.
        if let Ok(wd) = watches.add(&dir, mask()) {
            map.entry(wd).or_default().insert(name);
        }
    }
    map
}

fn watch_files(files: Vec<PathBuf>, tx: mpsc::Sender<()>) -> std::io::Result<JoinHandle<()>> {
    let inotify = Inotify::init()?;
    let mut watches = inotify.watches();
    let mut map = arm(&mut watches, &files);
    let mut stream = inotify.into_event_stream(vec![0u8; 16 * 1024])?;
    Ok(tokio::spawn(async move {
        while let Some(Ok(ev)) = stream.next().await {
            let hit = match (&ev.name, map.get(&ev.wd)) {
                (Some(name), Some(names)) => names.contains(name),
                // The watched folder itself went or moved.
                (None, Some(_)) => true,
                _ => false,
            };
            if hit {
                let _ = tx.try_send(());
            }
            map = arm(&mut stream.watches(), &files);
        }
    }))
}

async fn watch_signal(
    conn: &zbus::Connection,
    s: &SignalMatch,
    tx: mpsc::Sender<()>,
) -> Result<JoinHandle<()>, String> {
    let mut b = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(s.interface.as_str())
        .and_then(|b| b.member(s.member.as_str()))
        .map_err(|e| e.to_string())?;
    if let Some(p) = &s.path {
        b = b.path(p.as_str()).map_err(|e| e.to_string())?;
    }
    if let Some(n) = &s.sender {
        b = b.sender(n.as_str()).map_err(|e| e.to_string())?;
    }
    let mut stream = zbus::MessageStream::for_match_rule(b.build(), conn, Some(16))
        .await
        .map_err(|e| format!("can't follow {}.{}: {e}", s.interface, s.member))?;
    Ok(tokio::spawn(async move {
        while stream.next().await.is_some() {
            let _ = tx.try_send(());
        }
    }))
}

/// Start following `spec`. `home` expands `~/`; `conn(bus)` is the connection for a bus that a
/// signal match uses (`None`: not reachable, an error). Must run inside a tokio runtime.
pub async fn start(
    spec: &WatchSpec,
    home: &Path,
    session: Option<&zbus::Connection>,
    system: Option<&zbus::Connection>,
    debounce: Duration,
) -> Result<Watch, String> {
    let (raw_tx, mut raw_rx) = mpsc::channel::<()>(1);
    let (tx, changes) = mpsc::channel(1);
    // On an error below, dropping `w` aborts what was already started.
    let mut w = Watch {
        changes,
        tasks: Vec::new(),
    };
    if !spec.files.is_empty() {
        let files: Vec<PathBuf> = spec.files.iter().map(|f| expand(f, home)).collect();
        let t = watch_files(files, raw_tx.clone())
            .map_err(|e| format!("can't follow the files: {e}"))?;
        w.tasks.push(t);
    }
    for s in &spec.signals {
        let conn = match s.bus {
            Bus::Session => session.ok_or("the session bus is not reachable")?,
            Bus::System => system.ok_or("the system bus is not reachable")?,
        };
        w.tasks.push(watch_signal(conn, s, raw_tx.clone()).await?);
    }
    drop(raw_tx);
    w.tasks.push(tokio::spawn(async move {
        while raw_rx.recv().await.is_some() {
            loop {
                match tokio::time::timeout(debounce, raw_rx.recv()).await {
                    Ok(Some(())) => continue,
                    Ok(None) => return,
                    Err(_) => break,
                }
            }
            match tx.try_send(()) {
                Ok(()) | Err(mpsc::error::TrySendError::Full(())) => {}
                Err(mpsc::error::TrySendError::Closed(())) => return,
            }
        }
    }));
    Ok(w)
}

#[cfg(test)]
#[path = "command_watch_tests.rs"]
mod tests;
