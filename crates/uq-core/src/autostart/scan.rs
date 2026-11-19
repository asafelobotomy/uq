//! The autostart folders: the one bounded folder read, and the pure run plan over what it found.
//!
//! [`read_folders`] is the only function here that touches the disk. [`resolve`] applies the
//! XDG rules, the user's choices and the app fixes to its listings, so the runner, the page and
//! the tests all see the same plan.

use super::{Added, Autostart, HIDDEN_COPY_HEADER, MAX_DELAY};
use crate::compat::{Compat, launch};
use crate::desktop_entry::{
    Entry, FieldCtx, UQ_DESKTOPS, clean_display, exec_argv, exec_tokens, read_entry, set_key,
    shows_in, valid_desktop_id,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// The most entries read from one folder.
pub const MAX_PER_FOLDER: usize = 200;
/// The largest desktop file read; a bigger one is skipped.
pub const MAX_BYTES: u64 = 64 * 1024;
/// The most directory entries looked at in one folder.
const MAX_LISTED: usize = 4096;
const MAX_NAME_CHARS: usize = 100;
const MAX_ICON_CHARS: usize = 256;

/// Linux's `O_NONBLOCK` and `O_NOCTTY` on the architectures uQ builds for (the generic values,
/// shared by x86, Arm, RISC-V, PowerPC, s390x and LoongArch); uq-core has no libc dependency.
#[cfg(any(target_arch = "mips", target_arch = "mips64", target_arch = "sparc64"))]
compile_error!("O_NONBLOCK and O_NOCTTY differ on this architecture");
pub(crate) const O_NONBLOCK: i32 = 0o4000;
pub(crate) const O_NOCTTY: i32 = 0o400;

const NOT_APP: &str = "Not an app";
const HIDDEN_SYSTEM: &str = "Turned off by the system";
const HIDDEN_YOURS: &str = "Turned off by a file in your autostart folder";
const NOT_HERE: &str = "Not for this desktop";
const NOT_INSTALLED: &str = "Not installed";
const TERMINAL: &str = "Needs a terminal, which uQ doesn't open at login";
const UNREADABLE: &str = "Its command can't be read";

/// One file id found in the folders, with the texts already read and capped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub id: String,
    /// The user's folder's file of this id.
    pub user: Option<(PathBuf, String)>,
    /// The first system folder's file of this id.
    pub system: Option<(PathBuf, String)>,
}

/// Where an entry comes from, as the page says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    System,
    Yours,
    Added,
}

/// What the runner does with an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Start `argv` (already through `compat::launch::argv`) after `delay` seconds. `cwd` is
    /// the entry's absolute `Path=`; the runner uses the home folder when it is `None` or isn't
    /// a directory.
    Run {
        argv: Vec<String>,
        cwd: Option<PathBuf>,
        delay: u32,
        /// Hide the app's window once it opens: to the tray when it has an icon there,
        /// minimised otherwise.
        minimize: bool,
        /// Start it again once, ten seconds later, if it fails.
        retry: bool,
    },
    /// The user, or the entry itself, turned it off.
    Off,
    /// It can't run here, for this plain reason.
    Skip(&'static str),
}

/// One row of the Startup page, and one item of the run plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub id: String,
    /// Cleaned for one line (F36).
    pub name: String,
    /// Cleaned for one line (F36).
    pub icon: Option<String>,
    pub source: Source,
    pub enabled: bool,
    /// False for skipped entries.
    pub can_toggle: bool,
    pub delay: u32,
    pub minimize: bool,
    pub plan: Plan,
    /// uQ's hidden copy of a system entry that no longer exists, for the sync to remove.
    pub orphan_copy: Option<PathBuf>,
}

/// Read a desktop file safely (F14): opened non-blocking so a FIFO can't stall (and without
/// becoming a controlling terminal should the name be a terminal device), a regular file
/// once opened, at most [`MAX_BYTES`] (a bigger file is `None`), decoded lossily. Symlinks are
/// followed for reading.
pub fn read_capped(path: &Path) -> Option<String> {
    read_capped_if(path, std::fs::Metadata::is_file)
}

/// [`read_capped`], read only when `accept` takes the opened file's metadata (for example the
/// same device and inode as a no-follow check made just before).
pub fn read_capped_if(
    path: &Path,
    accept: impl FnOnce(&std::fs::Metadata) -> bool,
) -> Option<String> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(O_NONBLOCK | O_NOCTTY)
        .open(path)
        .ok()?;
    let m = f.metadata().ok()?;
    if !m.is_file() || !accept(&m) {
        return None;
    }
    let mut buf = Vec::new();
    f.take(MAX_BYTES + 1).read_to_end(&mut buf).ok()?;
    if buf.len() as u64 > MAX_BYTES {
        return None;
    }
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// The readable `<valid id>.desktop` files of one folder, by name, at most [`MAX_PER_FOLDER`].
fn read_folder(dir: &Path) -> Vec<(String, PathBuf, String)> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = rd
        .take(MAX_LISTED)
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".desktop") && valid_desktop_id(n))
        .collect();
    names.sort();
    let mut out = Vec::new();
    for name in names {
        if out.len() == MAX_PER_FOLDER {
            break;
        }
        let path = dir.join(&name);
        if let Some(text) = read_capped(&path) {
            out.push((name, path, text));
        }
    }
    out
}

/// The user's folder and the system folders, merged by id (sorted). The user's file shadows
/// the system ones, and the first system folder with an id wins.
pub fn read_folders(user: &Path, system: &[PathBuf]) -> Vec<Found> {
    let mut by_id: BTreeMap<String, Found> = BTreeMap::new();
    for (id, path, text) in read_folder(user) {
        by_id.entry(id.clone()).or_insert_with(|| empty(&id)).user = Some((path, text));
    }
    for dir in system {
        for (id, path, text) in read_folder(dir) {
            let f = by_id.entry(id.clone()).or_insert_with(|| empty(&id));
            if f.system.is_none() {
                f.system = Some((path, text));
            }
        }
    }
    by_id.into_values().collect()
}

fn empty(id: &str) -> Found {
    Found {
        id: id.to_string(),
        user: None,
        system: None,
    }
}

fn is_marked(text: &str) -> bool {
    read_entry(text).uq_autostart
}

/// The XDG rules in order (types, `Hidden`, show-in, `TryExec`, `Terminal`, `Exec`), giving the
/// argv for no files and the working directory, or the plain reason it can't run.
fn check_entry(
    e: &Entry,
    path: &Path,
    hidden: &'static str,
    for_desktop: bool,
    exists: &dyn Fn(&str) -> bool,
) -> Result<(Vec<String>, Option<PathBuf>), &'static str> {
    if e.kind.as_deref() != Some("Application") {
        return Err(NOT_APP);
    }
    if e.hidden {
        return Err(hidden);
    }
    if for_desktop && !shows_in(e, &UQ_DESKTOPS) {
        return Err(NOT_HERE);
    }
    if e.try_exec.as_deref().is_some_and(|t| !exists(t)) {
        return Err(NOT_INSTALLED);
    }
    if e.terminal {
        return Err(TERMINAL);
    }
    let exec = e.exec.as_deref().ok_or(UNREADABLE)?;
    if exec_tokens(exec).map_err(|_| UNREADABLE)?.lossy {
        return Err(UNREADABLE);
    }
    let ctx = FieldCtx {
        icon: e.icon.as_deref(),
        name: e.name.as_deref(),
        path: path.to_str(),
    };
    let argv = exec_argv(exec, &ctx).map_err(|_| UNREADABLE)?;
    let cwd = e
        .path
        .as_deref()
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    Ok((argv, cwd))
}

fn icon_of(e: &Entry) -> Option<String> {
    e.icon
        .as_deref()
        .map(|i| clean_display(i, MAX_ICON_CHARS))
        .filter(|i| !i.trim().is_empty())
}

/// The fields every row shares, before the plan.
struct Row {
    id: String,
    name: String,
    icon: Option<String>,
    source: Source,
    enabled: bool,
    delay: u32,
    minimize: bool,
    retry: bool,
}

impl Row {
    /// `checked` is the entry's argv and working directory, or why it is skipped. A fix for
    /// `fix_id` is applied; a fix that can't be applied leaves the command as it is.
    fn finish(
        self,
        checked: Result<(Vec<String>, Option<PathBuf>), &'static str>,
        fix_id: Option<&str>,
        compat: &Compat,
    ) -> Resolved {
        let (plan, enabled, can_toggle) = match checked {
            Err(reason) => (Plan::Skip(reason), false, false),
            Ok(_) if !self.enabled => (Plan::Off, false, true),
            Ok((argv, cwd)) => {
                let argv = match fix_id {
                    Some(id) => launch::argv(compat.fix_for(id), id, argv.clone()).unwrap_or(argv),
                    None => argv,
                };
                // An app that has its own way to start hidden is asked to use it.
                let argv = if self.minimize {
                    with_hidden_flag(argv)
                } else {
                    argv
                };
                let (delay, minimize, retry) = (self.delay, self.minimize, self.retry);
                let run = Plan::Run {
                    argv,
                    cwd,
                    delay,
                    minimize,
                    retry,
                };
                (run, true, true)
            }
        };
        Resolved {
            id: self.id,
            name: self.name,
            icon: self.icon,
            source: self.source,
            enabled,
            can_toggle,
            delay: self.delay,
            minimize: self.minimize,
            plan,
            orphan_copy: None,
        }
    }
}

fn resolve_found(
    f: &Found,
    state: &Autostart,
    compat: &Compat,
    exists: &dyn Fn(&str) -> bool,
) -> Option<Resolved> {
    let stem = f.id.strip_suffix(".desktop").unwrap_or(&f.id);
    let own = f.user.as_ref().filter(|(_, t)| !is_marked(t));
    let (source, (path, text)) = match (own, &f.system) {
        (Some(u), _) => (Source::Yours, u),
        (None, Some(s)) => (Source::System, s),
        (None, None) => {
            // Only uQ's hidden copy is left: its system entry is gone.
            let (path, text) = f.user.as_ref()?;
            let e = read_entry(text);
            return Some(Resolved {
                id: f.id.clone(),
                name: clean_display(e.name.as_deref().unwrap_or(stem), MAX_NAME_CHARS),
                icon: icon_of(&e),
                source: Source::System,
                enabled: false,
                can_toggle: false,
                delay: 0,
                minimize: false,
                plan: Plan::Skip(NOT_INSTALLED),
                orphan_copy: Some(path.clone()),
            });
        }
    };
    let e = read_entry(text);
    let choice = state.choice(&f.id);
    let row = Row {
        id: f.id.clone(),
        name: clean_display(e.name.as_deref().unwrap_or(stem), MAX_NAME_CHARS),
        icon: icon_of(&e),
        source,
        enabled: choice
            .and_then(|c| c.enabled)
            .unwrap_or(e.gnome_enabled.unwrap_or(true)),
        delay: choice
            .and_then(|c| c.delay)
            .unwrap_or(e.gnome_delay.unwrap_or(0).min(MAX_DELAY)),
        minimize: choice.and_then(|c| c.minimize).unwrap_or(false),
        retry: state.retry,
    };
    let hidden = match source {
        Source::Yours => HIDDEN_YOURS,
        _ => HIDDEN_SYSTEM,
    };
    let checked = check_entry(&e, path, hidden, true, exists);
    Some(row.finish(checked, Some(&f.id), compat))
}

fn resolve_added(
    a: &Added,
    state: &Autostart,
    compat: &Compat,
    apps: &dyn Fn(&str) -> Option<(PathBuf, String)>,
    exists: &dyn Fn(&str) -> bool,
) -> Resolved {
    let mut row = Row {
        id: a.id().to_string(),
        name: clean_display(a.name(), MAX_NAME_CHARS),
        icon: None,
        source: Source::Added,
        enabled: a.enabled(),
        delay: a.delay().unwrap_or(0).min(MAX_DELAY),
        minimize: a.minimize(),
        retry: state.retry,
    };
    match a {
        Added::Command { argv, .. } => row.finish(Ok((argv.clone(), None)), None, compat),
        Added::App { app, .. } => {
            let Some((path, text)) = apps(app) else {
                return row.finish(Err(NOT_INSTALLED), None, compat);
            };
            let e = read_entry(&text);
            row.icon = icon_of(&e);
            // The user chose this app, so the show-in lists don't apply; a hidden application
            // file means the app was removed.
            let checked = check_entry(&e, &path, NOT_INSTALLED, false, exists);
            // A marked override already carries its fix, so it is never given it again (R19);
            // `apps` gives the override's own source instead when the fix should apply.
            let fix_id = (!e.uq_override).then_some(app.as_str());
            row.finish(checked, fix_id, compat)
        }
    }
}

/// The run plan and the page's rows: the found entries by id, then the added ones in list
/// order. Pure: `apps` finds an added app's desktop file (path and capped text) and `exists`
/// answers `TryExec`.
pub fn resolve(
    found: &[Found],
    state: &Autostart,
    compat: &Compat,
    apps: &dyn Fn(&str) -> Option<(PathBuf, String)>,
    exists: &dyn Fn(&str) -> bool,
) -> Vec<Resolved> {
    let mut rows: Vec<Resolved> = found
        .iter()
        .filter_map(|f| resolve_found(f, state, compat, exists))
        .chain(
            state
                .added
                .iter()
                .map(|a| resolve_added(a, state, compat, apps, exists)),
        )
        .collect();
    apply_stagger(&mut rows, state.stagger);
    rows
}

/// The seconds each start waits when apps are `gap` seconds apart: in order of their own delay
/// (ties in list order), each waits for its own delay or `gap` after the one before, whichever
/// is later. With no gap they are the delays as they are.
pub fn staggered(delays: &[u32], gap: u32) -> Vec<u32> {
    let mut order: Vec<usize> = (0..delays.len()).collect();
    order.sort_by_key(|i| (delays[*i], *i));
    let mut out = delays.to_vec();
    let mut last: Option<u32> = None;
    for i in order {
        let at = last.map_or(delays[i], |l| delays[i].max(l.saturating_add(gap)));
        out[i] = at;
        last = Some(at);
    }
    out
}

/// Spread the apps that will run (`Plan::Run`) by the state's gap. The page's own `delay`
/// stays what the person set.
fn apply_stagger(rows: &mut [Resolved], gap: u32) {
    if gap == 0 {
        return;
    }
    let runs: Vec<usize> = (0..rows.len())
        .filter(|i| matches!(rows[*i].plan, Plan::Run { .. }))
        .collect();
    let own: Vec<u32> = runs.iter().map(|i| rows[*i].delay).collect();
    for (i, at) in runs.into_iter().zip(staggered(&own, gap)) {
        if let Plan::Run { delay, .. } = &mut rows[i].plan {
            *delay = at;
        }
    }
}

/// Every found id and every added id.
pub fn known_ids(found: &[Found], state: &Autostart) -> BTreeSet<String> {
    found
        .iter()
        .map(|f| f.id.clone())
        .chain(state.added.iter().map(|a| a.id().to_string()))
        .collect()
}

/// uQ's copy of a system entry that turns it off: the file line for line, with `Hidden=true`
/// and the marker set in `[Desktop Entry]`, under a header.
pub fn hidden_copy(system_text: &str) -> String {
    const ENTRY: &str = "Desktop Entry";
    let text = set_key(system_text, ENTRY, "Hidden", "true");
    let text = set_key(&text, ENTRY, "X-uQ-Autostart", "true");
    format!("{HIDDEN_COPY_HEADER}\n{text}")
}

/// The hidden copies the user folder should hold, as `(id, text)`, and uQ's copies to delete.
/// A system entry that is off in state, with no file of the user's own in its place, gets a
/// copy; a marked copy that isn't wanted (the entry is on again, or its system file is gone) is
/// deleted. A file without the marker is never listed.
pub fn wanted_copies(found: &[Found], state: &Autostart) -> (Vec<(String, String)>, Vec<PathBuf>) {
    let mut write = Vec::new();
    let mut delete = Vec::new();
    for f in found {
        let marked = f.user.as_ref().filter(|(_, t)| is_marked(t));
        if f.user.is_some() && marked.is_none() {
            continue;
        }
        let off = state.choice(&f.id).and_then(|c| c.enabled) == Some(false);
        match (&f.system, marked) {
            (Some((_, text)), _) if off => write.push((f.id.clone(), hidden_copy(text))),
            (_, Some((path, _))) => delete.push(path.clone()),
            _ => {}
        }
    }
    (write, delete)
}

/// Programs that can start hidden on their own: the program's file name and the option that
/// does it. These are the apps people most often start at login; the window watcher still
/// covers whatever the option leaves open.
const OWN_HIDDEN_FLAG: &[(&str, &str)] = &[
    ("steam", "-silent"),
    ("discord", "--start-minimized"),
    ("telegram-desktop", "-startintray"),
    ("signal-desktop", "--start-in-tray"),
    ("element-desktop", "--hidden"),
    ("slack", "--startup"),
    ("nextcloud", "--background"),
    ("syncthing-gtk", "--minimized"),
    ("1password", "--silent"),
];

/// The option the program (by file name, with no path) starts hidden with.
pub fn own_hidden_flag(program: &str) -> Option<&'static str> {
    let name = program.rsplit('/').next().unwrap_or(program);
    OWN_HIDDEN_FLAG
        .iter()
        .find(|(p, _)| *p == name)
        .map(|(_, f)| *f)
}

/// `argv` with the program's own start-hidden option right after the program, unless it is
/// already there. A program that isn't known is left as it is.
pub fn with_hidden_flag(mut argv: Vec<String>) -> Vec<String> {
    if let Some(flag) = argv.first().and_then(|p| own_hidden_flag(p))
        && !argv.iter().any(|a| a == flag)
    {
        argv.insert(1, flag.to_string());
    }
    argv
}
