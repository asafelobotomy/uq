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

/// The pages to show at startup: those that wait for no bus name. Pages that do are added by
/// [`watch`] once their name is seen to have an owner.
pub fn scan_configured() -> Vec<PluginPage> {
    set_shown(shown(&scan_logged(), |_, _| false))
}

/// Follow the plugin directory (inotify, debounced 1 s; Phase 4 spec §2.3) and the bus names
/// pages wait for, and post the pages to show whenever they change. Runs on `handle`; without
/// inotify it logs once and the list stays as it was at startup.
pub fn watch(handle: &tokio::runtime::Handle, post: impl Fn(Vec<PluginPage>) + Send + 'static) {
    handle.spawn(async move {
        let mut changes = match watch_dirs(vec![absolute(dir())], DEBOUNCE) {
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
        let mut last: Option<Vec<Found>> = None;
        loop {
            let found = scan_logged();
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
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn write(root: &Path, dir: &str, toml: &str) {
        let d = root.join(dir);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("page.toml"), toml).unwrap();
        fs::write(d.join("Page.qml"), "import QtQuick\nItem {}\n").unwrap();
    }

    fn manifest(id: &str, extra: &str) -> String {
        format!("id = \"{id}\"\nname = \"Name {id}\"\nqml = \"Page.qml\"\n{extra}")
    }

    fn ids(found: &[Found]) -> Vec<&str> {
        found.iter().map(|f| f.page.id.as_str()).collect()
    }

    #[test]
    fn scan_skips_bad_manifests() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        let bins = tempfile::tempdir().unwrap();
        fs::write(bins.path().join("tailscale"), "").unwrap();
        write(
            r,
            "a-good",
            &manifest(
                "a-good",
                "icon = \"folder\"\ncategory = \"apps\"\ndbus_name = \"org.example.Good\"\n\
                 keywords = [\"tools\"]\n[requires]\nbins = [\"tailscale\"]\n\
                 [[setting]]\nid = \"mode\"\nlabel = \"Mode\"\nkeywords = [\"kind\"]\n",
            ),
        );
        fs::create_dir_all(r.join("b-no-manifest")).unwrap();
        fs::write(r.join("a-file.txt"), "not a directory").unwrap();
        write(
            r,
            "c-unknown-key",
            &manifest("c-unknown-key", "surprise = 1\n"),
        );
        write(r, "d-not-folder-name", &manifest("dup", ""));
        write(r, "appearance", &manifest("appearance", ""));
        write(
            r,
            "f-dotdot",
            &manifest("f-dotdot", "").replace("Page.qml", "../x.qml"),
        );
        write(
            r,
            "f-slash",
            &manifest("f-slash", "").replace("Page.qml", "sub/x.qml"),
        );
        write(
            r,
            "f-ext",
            &manifest("f-ext", "").replace("Page.qml", "Page.js"),
        );
        write(r, "g-bad-id", &manifest("Bad_Id", ""));
        write(
            r,
            "g-category",
            &manifest("g-category", "category = \"system\"\n"),
        );
        write(r, "h-syntax", "id = \n");
        write(
            r,
            "i-dup-setting",
            &manifest(
                "i-dup-setting",
                "[[setting]]\nid = \"a\"\nlabel = \"A\"\n[[setting]]\nid = \"a\"\nlabel = \"B\"\n",
            ),
        );
        write(
            r,
            "j-unknown-setting-key",
            &manifest(
                "j-unknown-setting-key",
                "[[setting]]\nid = \"a\"\nlabel = \"A\"\nx = 1\n",
            ),
        );
        write(
            r,
            "j-missing-bin",
            &manifest("j-missing-bin", "[requires]\nbins = [\"nope\"]\n"),
        );
        write(r, "j-newer", &manifest("j-newer", "uq_api = 2\n"));
        write(r, "z-after", &manifest("z-after", "uq_api = 1\n"));

        // Symlinks: a page directory pointing outside, a qml file pointing outside, a manifest
        // pointing outside, and a missing qml file.
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "k-dir-link", &manifest("k-dir-link", ""));
        symlink(outside.path().join("k-dir-link"), r.join("k-dir-link")).unwrap();
        write(r, "l-qml-link", &manifest("l-qml-link", ""));
        fs::remove_file(r.join("l-qml-link/Page.qml")).unwrap();
        fs::write(outside.path().join("Out.qml"), "import QtQuick\nItem {}\n").unwrap();
        symlink(
            outside.path().join("Out.qml"),
            r.join("l-qml-link/Page.qml"),
        )
        .unwrap();
        write(r, "m-toml-link", "");
        fs::remove_file(r.join("m-toml-link/page.toml")).unwrap();
        fs::write(outside.path().join("x.toml"), manifest("m-toml-link", "")).unwrap();
        symlink(
            outside.path().join("x.toml"),
            r.join("m-toml-link/page.toml"),
        )
        .unwrap();
        write(r, "n-no-qml", &manifest("n-no-qml", ""));
        fs::remove_file(r.join("n-no-qml/Page.qml")).unwrap();
        // A link that stays inside its own page directory is fine.
        write(r, "o-inner-link", &manifest("o-inner-link", ""));
        fs::rename(
            r.join("o-inner-link/Page.qml"),
            r.join("o-inner-link/Real.qml"),
        )
        .unwrap();
        symlink("Real.qml", r.join("o-inner-link/Page.qml")).unwrap();

        let (pages, skipped) = scan(r, bins.path());
        assert_eq!(
            ids(&pages),
            ["a-good", "o-inner-link", "z-after"],
            "in directory-name order"
        );
        let r_canon = fs::canonicalize(r).unwrap();
        assert_eq!(pages[0].page.qml_path, r_canon.join("a-good/Page.qml"));
        assert_eq!(
            pages[1].page.qml_path,
            r_canon.join("o-inner-link/Real.qml")
        );
        assert_eq!(pages[0].page.icon, "folder");
        assert_eq!(pages[0].page.dbus_name.as_deref(), Some("org.example.Good"));
        assert_eq!(pages[0].page.settings[0].keywords, ["kind"]);
        let joined = skipped.join("\n");
        for dir in [
            "b-no-manifest",
            "c-unknown-key",
            "d-not-folder-name",
            "appearance",
            "f-dotdot",
            "f-slash",
            "f-ext",
            "g-bad-id",
            "g-category",
            "h-syntax",
            "i-dup-setting",
            "j-unknown-setting-key",
            "j-missing-bin",
            "j-newer",
            "k-dir-link",
            "l-qml-link",
            "m-toml-link",
            "n-no-qml",
        ] {
            assert!(
                skipped.iter().any(|s| s.starts_with(&format!("{dir}: "))),
                "{dir} not skipped with a reason:\n{joined}"
            );
        }
        assert_eq!(skipped.len(), 18, "{joined}");
        assert!(joined.contains("unknown field"), "{joined}");
        assert!(joined.contains("needs a newer uQ"), "{joined}");
        assert!(joined.contains("is a core page"), "{joined}");
    }

    #[test]
    fn pages_waiting_for_a_bus_name_show_only_with_an_owner() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        write(r, "plain", &manifest("plain", ""));
        write(
            r,
            "waits",
            &manifest("waits", "[requires]\nbus_name = \"org.example.Daemon1\"\n"),
        );
        write(
            r,
            "system",
            &manifest(
                "system",
                "[requires]\nbus_name = \"org.example.Sys1\"\n\
                 [dbus]\nname = \"org.example.Sys1\"\npath = \"/\"\n\
                 interface = \"org.example.Sys1\"\nbus = \"system\"\n",
            ),
        );
        let (found, _) = scan(r, Path::new("/nonexistent"));
        assert_eq!(ids(&found), ["plain", "system", "waits"]);
        assert_eq!(ids(&shown(&found, |_, _| false)), ["plain"]);
        assert_eq!(
            ids(&shown(&found, |n, b| n == "org.example.Daemon1"
                && b == Bus::Session)),
            ["plain", "waits"]
        );
        assert_eq!(
            ids(&shown(&found, |n, b| n == "org.example.Sys1" && b == Bus::System)),
            ["plain", "system"]
        );
    }

    #[test]
    fn shown_pages_keep_their_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            tmp.path(),
            "cmds",
            &manifest(
                "cmds",
                "[[command]]\nid = \"status\"\nargv = [\"/usr/bin/true\"]\noutput = \"none\"\n",
            ),
        );
        let (found, _) = scan(tmp.path(), Path::new("/nonexistent"));
        let pages = set_shown(found);
        assert_eq!(pages[0].id, "cmds");
        assert_eq!(super::manifest("cmds").unwrap().commands[0].id, "status");
        assert!(super::manifest("nope").is_none());
        set_shown(Vec::new());
        assert!(super::manifest("cmds").is_none());
    }

    /// Live discovery: a page installed while the app runs is posted, and so is its removal.
    /// The only test that sets the plugin directory (it is process-wide).
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn installed_and_removed_pages_are_posted() {
        let tmp = tempfile::tempdir().unwrap();
        let pages = tmp.path().join("pages");
        set_dir(pages.clone());
        assert_eq!(dir(), pages.as_path());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<String>>();
        watch(&tokio::runtime::Handle::current(), move |p| {
            let _ = tx.send(p.into_iter().map(|p| p.id).collect());
        });
        async fn next(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<String>>) -> Vec<String> {
            tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
                .await
                .expect("no update within 10 s")
                .unwrap()
        }
        assert!(next(&mut rx).await.is_empty(), "nothing installed yet");

        // The directory itself doesn't exist yet: installing the first page creates it.
        let staged = tempfile::tempdir().unwrap();
        write(staged.path(), "live", &manifest("live", ""));
        fs::create_dir_all(&pages).unwrap();
        fs::rename(staged.path().join("live"), pages.join("live")).unwrap();
        assert_eq!(next(&mut rx).await, ["live"]);

        fs::remove_dir_all(pages.join("live")).unwrap();
        assert!(next(&mut rx).await.is_empty(), "removed");
    }

    #[test]
    fn a_missing_directory_is_no_plugins() {
        let (pages, skipped) = scan(Path::new("/nonexistent/uq/pages"), Path::new("/usr/bin"));
        assert!(pages.is_empty() && skipped.is_empty());
    }

    #[test]
    fn file_url_encodes() {
        assert_eq!(
            file_url(Path::new("/a b/c#d/E.qml")),
            "file:///a%20b/c%23d/E.qml"
        );
    }
}
