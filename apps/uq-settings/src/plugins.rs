//! Plugin settings pages (Phase 3 spec §2.5, Phase 4 spec §2.3): `<dir>/<id>/page.toml` plus one
//! QML file. Read only from the system directory (or `--plugin-dir`); a bad page is skipped with
//! a reason, never fatal. The manifest is `uq_core::integration::manifest::PageManifest`; pages
//! whose `[requires]` don't hold are hidden, and the directory is watched so an installed or
//! removed page appears or goes without restarting the app.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use uq_core::integration::manifest::{BIN_DIR, Bus, Loaded, PageManifest};
use uq_core::integration::scan::scan_pages;
use uq_core::integration::watch::{DEBOUNCE, watch_dirs};

use crate::registry::{self, OwnedSearchEntry};

/// The only directory read by default: root-owned, never a user-writable one.
pub const SYSTEM_DIR: &str = "/usr/share/uq/settings/pages";

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Override the plugin directory (`--plugin-dir`); only the first call counts.
pub fn set_dir(dir: PathBuf) {
    let _ = DIR.set(dir);
}

pub fn dir() -> &'static Path {
    DIR.get_or_init(|| PathBuf::from(SYSTEM_DIR))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPage {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub qml_path: PathBuf,
    #[allow(dead_code, reason = "the page's context reads it from the manifest")]
    pub dbus_name: Option<String>,
    pub keywords: Vec<String>,
    pub settings: Vec<OwnedSearchEntry>,
}

/// A loaded page and its whole manifest (`[[command]]`, `[requires]`, `[dbus]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub page: PluginPage,
    pub manifest: PageManifest,
}

impl Found {
    /// `requires.bus_name` and the bus it is watched on (the `[dbus]` table's; session if none).
    pub fn waits_for(&self) -> Option<(String, Bus)> {
        let bus = self.manifest.dbus.as_ref().map_or(Bus::Session, |d| d.bus);
        self.manifest.requires.bus_name.clone().map(|n| (n, bus))
    }
}

fn found(loaded: Loaded<PageManifest>) -> Found {
    let m = loaded.manifest;
    Found {
        page: PluginPage {
            qml_path: loaded.qml_path,
            id: m.id.clone(),
            name: m.name.clone(),
            icon: m
                .icon
                .clone()
                .unwrap_or_else(|| "application-x-executable".into()),
            dbus_name: m.dbus_name.clone(),
            keywords: m.keywords.clone(),
            settings: m
                .settings
                .iter()
                .map(|s| OwnedSearchEntry {
                    id: s.id.clone(),
                    label: s.label.clone(),
                    keywords: s.keywords.clone(),
                })
                .collect(),
        },
        manifest: m,
    }
}

/// Every valid plugin page under `dir` whose `requires.bins` are installed (in `bin_dir`), in
/// directory-name order, and one `"<dir name>: <reason>"` line for each directory skipped. A
/// page's id must equal its directory name (Phase 4 spec §2.6); an id that collides with a core
/// page is skipped. A missing `dir` is no plugins, not an error. Pages that wait for a bus name
/// are included; [`shown`] filters them.
///
/// Loading follows `uq_core::integration::manifest::load_page`: a page directory, its manifest
/// or its QML file that is a symlink resolving outside it is refused. This is hygiene, not a
/// sandbox: plugin QML can still `import "../x"` or read any file the user can.
pub fn scan(dir: &Path, bin_dir: &Path) -> (Vec<Found>, Vec<String>) {
    let mut pages: Vec<Found> = Vec::new();
    let mut skipped = Vec::new();
    let list = match scan_pages(dir) {
        Ok(list) => list,
        Err(e) => {
            skipped.push(e);
            return (pages, skipped);
        }
    };
    for (path, result) in list {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        match result.map(found) {
            Ok(f) if registry::find(&f.page.id).is_some() => {
                skipped.push(format!("{name}: id {:?} is a core page", f.page.id));
            }
            Ok(f) => {
                let missing = f.manifest.requires.missing_bins(bin_dir);
                if missing.is_empty() {
                    pages.push(f);
                } else {
                    skipped.push(format!(
                        "{name}: hidden until {} is installed",
                        missing.join(", ")
                    ));
                }
            }
            Err(reason) => skipped.push(format!("{name}: {reason}")),
        }
    }
    (pages, skipped)
}

/// The pages to show: those that wait for no bus name, and those whose name has an owner.
pub fn shown(found: &[Found], has_owner: impl Fn(&str, Bus) -> bool) -> Vec<Found> {
    found
        .iter()
        .filter(|f| f.waits_for().is_none_or(|(n, bus)| has_owner(&n, bus)))
        .cloned()
        .collect()
}

/// The manifests of the pages on show, for the page context (`crate::plugin_host`).
static SHOWN: Mutex<Vec<Found>> = Mutex::new(Vec::new());

/// Remember the pages on show and return them as the sidebar takes them.
pub fn set_shown(found: Vec<Found>) -> Vec<PluginPage> {
    let pages = found.iter().map(|f| f.page.clone()).collect();
    *SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = found;
    pages
}

/// The manifest of page `id`, if it is on show.
pub fn manifest(id: &str) -> Option<PageManifest> {
    SHOWN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|f| f.page.id == id)
        .map(|f| f.manifest.clone())
}

/// Scan the configured directory, logging each reason a page was skipped once per run.
fn scan_logged() -> Vec<Found> {
    static LOGGED: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let (pages, skipped) = scan(&absolute(dir()), Path::new(BIN_DIR));
    let mut logged = LOGGED.lock().unwrap_or_else(|e| e.into_inner());
    for line in skipped {
        if !logged.contains(&line) {
            eprintln!("uq-settings: skipping plugin page {line}");
            logged.push(line);
        }
    }
    pages
}

/// The scan `watch` made at startup, for `scan_configured` to take instead of scanning again.
static STARTUP: Mutex<Option<Vec<Found>>> = Mutex::new(None);

/// The pages to show at startup: those that wait for no bus name. Pages that do are added by
/// [`watch`] once their name is seen to have an owner. Uses the scan `watch` already made, if
/// it ran first (it does, except for screenshots).
pub fn scan_configured() -> Vec<PluginPage> {
    let found = STARTUP
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .unwrap_or_else(scan_logged);
    set_shown(shown(&found, |_, _| false))
}

/// Follow the plugin directory (inotify, debounced 1 s; Phase 4 spec §2.3) and the bus names
/// pages wait for, and post the pages to show whenever they change. Runs on `handle`; without
/// inotify it logs once and the list stays as it was at startup. The directory is watched
/// first and then scanned once, here; that scan also serves `scan_configured`.
pub fn watch(handle: &tokio::runtime::Handle, post: impl Fn(Vec<PluginPage>) + Send + 'static) {
    let changes = {
        let _rt = handle.enter();
        watch_dirs(vec![absolute(dir())], DEBOUNCE)
    };
    let first = scan_logged();
    *STARTUP.lock().unwrap_or_else(|e| e.into_inner()) = Some(first.clone());
    // What `scan_configured` shows: no post unless the bus names change it.
    let mut last = Some(shown(&first, |_, _| false));
    let mut first = Some(first);
    handle.spawn(async move {
        let mut changes = match changes {
            Ok(rx) => rx,
            Err(e) => {
                eprintln!(
                    "uq-settings: can't watch {} for new pages: {e}",
                    dir().display()
                );
                return;
            }
        };
        let (owner_tx, mut owner_rx) = tokio::sync::mpsc::channel::<()>(1);
        let mut owners = Owners::default();
        loop {
            let found = first.take().unwrap_or_else(scan_logged);
            owners.follow(&found, &owner_tx).await;
            let now = owners.filter(&found).await;
            if last.as_ref() != Some(&now) {
                last = Some(now.clone());
                post(set_shown(now));
            }
            tokio::select! {
                c = changes.recv() => if c.is_none() { return },
                _ = owner_rx.recv() => {}
            }
        }
    });
}

/// The session and system bus connections used to check `requires.bus_name`, each with a task
/// that pings on a `NameOwnerChanged` for one of the names pages wait for.
#[derive(Default)]
struct Owners {
    session: Option<Option<zbus::Connection>>,
    system: Option<Option<zbus::Connection>>,
    wanted: std::sync::Arc<Mutex<Vec<String>>>,
}

impl Owners {
    async fn conn(
        &mut self,
        bus: Bus,
        ping: &tokio::sync::mpsc::Sender<()>,
    ) -> Option<zbus::Connection> {
        let slot = match bus {
            Bus::Session => &mut self.session,
            Bus::System => &mut self.system,
        };
        if slot.is_none() {
            let conn = match bus {
                Bus::Session => match crate::session::bus() {
                    Some((_, c)) => Ok(c),
                    None => zbus::Connection::session().await,
                },
                Bus::System => zbus::Connection::system().await,
            };
            let conn = conn
                .map_err(|e| {
                    eprintln!("uq-settings: plugin pages can't watch the {bus:?} bus: {e}")
                })
                .ok();
            if let Some(c) = &conn {
                follow_owner_changes(c.clone(), ping.clone(), self.wanted.clone()).await;
            }
            *slot = Some(conn);
        }
        slot.clone().flatten()
    }

    /// Connect (once) to each bus a page waits on.
    async fn follow(&mut self, found: &[Found], ping: &tokio::sync::mpsc::Sender<()>) {
        *self.wanted.lock().unwrap_or_else(|e| e.into_inner()) = found
            .iter()
            .filter_map(|f| f.waits_for())
            .map(|(n, _)| n)
            .collect();
        for f in found {
            if let Some((_, bus)) = f.waits_for() {
                self.conn(bus, ping).await;
            }
        }
    }

    async fn filter(&mut self, found: &[Found]) -> Vec<Found> {
        let mut owned: Vec<(String, Bus)> = Vec::new();
        for f in found {
            let Some((name, bus)) = f.waits_for() else {
                continue;
            };
            let conn = match bus {
                Bus::Session => self.session.clone().flatten(),
                Bus::System => self.system.clone().flatten(),
            };
            if let Some(c) = conn
                && has_owner(&c, &name).await
            {
                owned.push((name, bus));
            }
        }
        shown(found, |n, b| owned.iter().any(|(o, ob)| o == n && *ob == b))
    }
}

async fn has_owner(conn: &zbus::Connection, name: &str) -> bool {
    let Ok(dbus) = zbus::fdo::DBusProxy::new(conn).await else {
        return false;
    };
    let Ok(name) = zbus::names::BusName::try_from(name) else {
        return false;
    };
    matches!(
        tokio::time::timeout(std::time::Duration::from_secs(2), dbus.name_has_owner(name)).await,
        Ok(Ok(true))
    )
}

async fn follow_owner_changes(
    conn: zbus::Connection,
    ping: tokio::sync::mpsc::Sender<()>,
    wanted: std::sync::Arc<Mutex<Vec<String>>>,
) {
    use futures_util::StreamExt;
    let Ok(dbus) = zbus::fdo::DBusProxy::new(&conn).await else {
        return;
    };
    let Ok(mut stream) = dbus.receive_name_owner_changed().await else {
        return;
    };
    tokio::spawn(async move {
        while let Some(sig) = stream.next().await {
            let relevant = sig.args().is_ok_and(|a| {
                wanted
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .iter()
                    .any(|w| w == a.name().as_str())
            });
            // A full channel already holds a ping: the rescan will see this change too.
            if relevant && ping.try_send(()).is_err() && ping.is_closed() {
                return;
            }
        }
    });
}

fn absolute(dir: &Path) -> PathBuf {
    fs::canonicalize(dir).unwrap_or_else(|_| std::path::absolute(dir).unwrap_or(dir.into()))
}

/// An absolute path as a `file://` URL, percent-encoding everything but unreserved characters.
pub fn file_url(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut url = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'.' | b'_' | b'~') {
            url.push(b as char);
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

#[cfg(test)]
mod tests;
