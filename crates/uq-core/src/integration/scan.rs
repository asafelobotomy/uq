//! Everything the integration roots hold, valid or not (Phase 4 spec §2.7 `uqctl integrations
//! list`, §5): widgets, settings pages, launcher providers, app actions and window rule files, each
//! with its source path, whether it is valid, whether it shows, and why not.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::integration::appaction::{AppAction, scan_app_actions};
use crate::integration::manifest::{
    BIN_DIR, Bus, Loaded, ManifestError, PageManifest, Requires, WidgetManifest, load_page,
    load_widget,
};
use crate::integration::rules::{RuleFile, scan_rules};
use crate::launcher_providers::scan_defs;

pub const WIDGETS_DIR: &str = "/usr/share/uq/shell/widgets";
pub const PAGES_DIR: &str = "/usr/share/uq/settings/pages";
pub const LAUNCHER_DIR: &str = "/usr/share/uq/launcher";
pub const ACTIONS_DIR: &str = "/usr/share/uq/launcher/actions";
/// `<name>/rules.toml` window rules (§3.3).
pub const INTEGRATIONS_DIR: &str = "/usr/share/uq/integrations";

/// Where to look. [`Roots::system`] is what uQ reads; tests and `uqctl integrations check`
/// point it elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roots {
    pub widgets: PathBuf,
    pub pages: PathBuf,
    /// Provider directories in priority order (system, then the user's, as Phase 2 defined).
    pub providers: Vec<PathBuf>,
    pub actions: PathBuf,
    /// `<name>/rules.toml` folders.
    pub integrations: PathBuf,
    /// Where `[requires] bins` must exist.
    pub bin_dir: PathBuf,
}

impl Roots {
    pub fn system() -> Roots {
        Roots {
            widgets: WIDGETS_DIR.into(),
            pages: PAGES_DIR.into(),
            providers: crate::launcher_providers::default_dirs(),
            actions: ACTIONS_DIR.into(),
            integrations: INTEGRATIONS_DIR.into(),
            bin_dir: BIN_DIR.into(),
        }
    }
}

/// What a scanned item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Widget,
    Page,
    Provider,
    AppAction,
    /// An integration folder's `rules.toml`; its id is the folder name.
    Rules,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Widget => "widget",
            Kind::Page => "page",
            Kind::Provider => "provider",
            Kind::AppAction => "app_action",
            Kind::Rules => "rules",
        }
    }
}

/// One scanned item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub kind: Kind,
    /// The folder or file id; a provider's bus name.
    pub id: String,
    /// The display name, when the file parsed.
    pub name: Option<String>,
    /// The folder (widgets, pages) or file (providers, app actions).
    pub source: PathBuf,
    pub valid: bool,
    /// Valid and every condition that can be checked from files holds. A `requires.bus_name`
    /// is not checked here; see `waits_for`.
    pub visible: bool,
    /// Why the file was refused (`valid` false) or why it is hidden.
    pub error: Option<String>,
    /// `requires.bus_name`: shown only while this name has an owner on `bus`.
    pub waits_for: Option<String>,
    pub bus: Bus,
}

impl Entry {
    fn refused(kind: Kind, id: String, source: PathBuf, error: String) -> Entry {
        Entry {
            kind,
            id,
            name: None,
            source,
            valid: false,
            visible: false,
            error: Some(error),
            waits_for: None,
            bus: Bus::Session,
        }
    }
}

/// One folder of a widgets or pages root.
pub type Found<M> = (PathBuf, Result<Loaded<M>, ManifestError>);

/// Every sub-folder of `root`, in name order, loaded with `load`. A missing root is empty; a
/// root that can't be read is one error.
fn scan_folders<M>(
    root: &Path,
    load: fn(&Path, &Path) -> Result<Loaded<M>, ManifestError>,
) -> Result<Vec<Found<M>>, String> {
    let entries = match std::fs::read_dir(root) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", root.display())),
    };
    let croot = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    Ok(dirs
        .into_iter()
        .map(|d| {
            let r = load(&d, &croot);
            (d, r)
        })
        .collect())
}

/// Every widget folder under `root`.
pub fn scan_widgets(root: &Path) -> Result<Vec<Found<WidgetManifest>>, String> {
    scan_folders(root, load_widget)
}

/// Every page folder under `root`.
pub fn scan_pages(root: &Path) -> Result<Vec<Found<PageManifest>>, String> {
    scan_folders(root, load_page)
}

fn folder_id(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn requires_entry(
    kind: Kind,
    source: PathBuf,
    id: String,
    name: String,
    requires: &Requires,
    bus: Bus,
    bin_dir: &Path,
) -> Entry {
    let missing = requires.missing_bins(bin_dir);
    Entry {
        kind,
        id,
        name: Some(name),
        source,
        valid: true,
        visible: missing.is_empty(),
        error: (!missing.is_empty()).then(|| {
            format!(
                "hidden: needs {}",
                missing
                    .iter()
                    .map(|b| bin_dir.join(b).display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }),
        waits_for: requires.bus_name.clone(),
        bus,
    }
}

fn folder_entries<M>(
    kind: Kind,
    root: &Path,
    found: Result<Vec<Found<M>>, String>,
    ok: impl Fn(PathBuf, Loaded<M>) -> Entry,
) -> Vec<Entry> {
    match found {
        Err(e) => vec![Entry::refused(kind, String::new(), root.to_path_buf(), e)],
        Ok(list) => list
            .into_iter()
            .map(|(path, r)| match r {
                Ok(loaded) => ok(path, loaded),
                Err(e) => Entry::refused(kind, folder_id(&path), path, e.to_string()),
            })
            .collect(),
    }
}

/// Widgets, then pages, then providers, then app actions, then rule files, each in name order.
pub fn scan_all(roots: &Roots) -> Vec<Entry> {
    let bin_dir = roots.bin_dir.as_path();
    let mut out = folder_entries(
        Kind::Widget,
        &roots.widgets,
        scan_widgets(&roots.widgets),
        |path, l| {
            let m = l.manifest;
            let bus = m.dbus.as_ref().map_or(Bus::Session, |d| d.bus);
            requires_entry(Kind::Widget, path, m.id, m.name, &m.requires, bus, bin_dir)
        },
    );
    out.extend(folder_entries(
        Kind::Page,
        &roots.pages,
        scan_pages(&roots.pages),
        |path, l| {
            let m = l.manifest;
            let bus = m.dbus.as_ref().map_or(Bus::Session, |d| d.bus);
            requires_entry(Kind::Page, path, m.id, m.name, &m.requires, bus, bin_dir)
        },
    ));
    for (path, r) in scan_defs(&roots.providers) {
        out.push(match r {
            Ok(d) => Entry {
                kind: Kind::Provider,
                id: d.bus_name,
                name: Some(d.name),
                source: path,
                valid: true,
                visible: true,
                error: None,
                waits_for: None,
                bus: Bus::Session,
            },
            Err(e) => Entry::refused(
                Kind::Provider,
                path.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path,
                e,
            ),
        });
    }
    for (path, r) in scan_app_actions(&roots.actions) {
        out.push(app_action_entry(path, r));
    }
    match scan_rules(&roots.integrations) {
        Ok(files) => out.extend(files.into_iter().map(rules_entry)),
        Err(e) => out.push(Entry::refused(
            Kind::Rules,
            String::new(),
            roots.integrations.clone(),
            e,
        )),
    }
    out
}

/// A rule file is never "hidden": each rule is off until the person turns it on.
fn rules_entry(f: RuleFile) -> Entry {
    match f.rules {
        Ok(rules) => Entry {
            kind: Kind::Rules,
            id: f.integration,
            name: Some(match rules.len() {
                1 => "1 window rule".to_string(),
                n => format!("{n} window rules"),
            }),
            source: f.path,
            valid: true,
            visible: true,
            error: None,
            waits_for: None,
            bus: Bus::Session,
        },
        Err(e) => Entry::refused(Kind::Rules, f.integration, f.path, e.to_string()),
    }
}

fn app_action_entry(path: PathBuf, r: Result<AppAction, String>) -> Entry {
    match r {
        Ok(a) => {
            let exists = a.program_exists();
            Entry {
                kind: Kind::AppAction,
                error: (!exists).then(|| format!("hidden: {} is missing", a.program())),
                id: a.id,
                name: Some(a.name),
                source: path,
                valid: true,
                visible: exists,
                waits_for: None,
                bus: Bus::Session,
            }
        }
        Err(e) => Entry::refused(
            Kind::AppAction,
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path,
            e,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn roots(base: &Path) -> Roots {
        Roots {
            widgets: base.join("shell/widgets"),
            pages: base.join("settings/pages"),
            providers: vec![base.join("launcher")],
            actions: base.join("launcher/actions"),
            integrations: base.join("integrations"),
            bin_dir: base.join("bin"),
        }
    }

    fn put(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn lists_every_kind_with_state_and_errors() {
        let d = tempfile::tempdir().unwrap();
        let b = d.path();
        let r = roots(b);
        put(&b.join("bin/hauler"), "");
        put(
            &r.widgets.join("hauler/widget.toml"),
            "uq_api = 1\nid = \"hauler\"\nname = \"Downloads\"\nslot = \"bar\"\nqml = \"Widget.qml\"\n\
             [requires]\nbus_name = \"org.hauler.Hauler1\"\nbins = [\"hauler\"]\n",
        );
        put(&r.widgets.join("hauler/Widget.qml"), "Item {}\n");
        put(
            &r.widgets.join("needs/widget.toml"),
            "uq_api = 1\nid = \"needs\"\nname = \"N\"\nslot = \"quick\"\nqml = \"W.qml\"\n\
             [requires]\nbins = [\"nope\"]\n",
        );
        put(&r.widgets.join("needs/W.qml"), "Item {}\n");
        put(
            &r.widgets.join("future/widget.toml"),
            "uq_api = 2\nid = \"future\"\nshiny = true\n",
        );
        put(
            &r.pages.join("tailscale/page.toml"),
            "id = \"tailscale\"\nname = \"Tailscale\"\nqml = \"Page.qml\"\n",
        );
        put(&r.pages.join("tailscale/Page.qml"), "Item {}\n");
        put(
            &r.pages.join("wrong/page.toml"),
            "id = \"other\"\nname = \"W\"\nqml = \"Page.qml\"\n",
        );
        put(&r.pages.join("wrong/Page.qml"), "Item {}\n");
        put(
            &b.join("launcher/emobie.provider"),
            "[Launcher Provider]\nBusName=org.emobie.Provider\nName=Emoji\nPrefix=:\nActivatable=true\n",
        );
        put(
            &b.join("launcher/bad.provider"),
            "[Launcher Provider]\nName=x\n",
        );
        put(
            &r.actions.join("uninstall.appaction"),
            "[App Action]\nName=Uninstall…\nExec=/usr/bin/uq-no-such-crumbs uninstall %d\n",
        );
        put(
            &r.actions.join("sh.appaction"),
            "[App Action]\nName=Shell\nExec=/bin/sh -c true\n",
        );

        put(
            &r.integrations.join("romforge/rules.toml"),
            "[[window_rule]]\nid = \"romforge.emulators\"\nclass = [\"^retroarch$\"]\n\
             workspace = \"name:game\"\nfullscreen = true\n",
        );
        put(
            &r.integrations.join("broken/rules.toml"),
            "[[window_rule]]\nid = \"other.x\"\nclass = [\"a\"]\nfloat = true\n",
        );
        put(&r.integrations.join("no-rules/README.md"), "");

        let all = scan_all(&r);
        let find = |kind: Kind, id: &str| {
            all.iter()
                .find(|e| e.kind == kind && e.id == id)
                .unwrap_or_else(|| panic!("{kind:?} {id} missing from {all:#?}"))
        };
        let h = find(Kind::Widget, "hauler");
        assert!(h.valid && h.visible, "{h:?}");
        assert_eq!(h.waits_for.as_deref(), Some("org.hauler.Hauler1"));
        assert_eq!(h.name.as_deref(), Some("Downloads"));
        let n = find(Kind::Widget, "needs");
        assert!(n.valid && !n.visible);
        assert!(n.error.as_ref().unwrap().contains("bin/nope"), "{n:?}");
        let f = find(Kind::Widget, "future");
        assert!(!f.valid);
        assert!(
            f.error.as_ref().unwrap().contains("needs a newer uQ"),
            "{f:?}"
        );
        assert!(find(Kind::Page, "tailscale").visible);
        let w = find(Kind::Page, "wrong");
        assert!(w.error.as_ref().unwrap().contains("folder name"), "{w:?}");
        let p = find(Kind::Provider, "org.emobie.Provider");
        assert!(p.valid && p.visible);
        assert!(!find(Kind::Provider, "bad").valid);
        let u = find(Kind::AppAction, "uninstall");
        assert!(u.valid && !u.visible, "a missing program hides the action");
        assert!(u.error.as_ref().unwrap().contains("missing"));
        assert!(find(Kind::AppAction, "sh").visible);
        let rf = find(Kind::Rules, "romforge");
        assert!(rf.valid && rf.visible, "{rf:?}");
        assert_eq!(rf.name.as_deref(), Some("1 window rule"));
        assert!(rf.source.ends_with("integrations/romforge/rules.toml"));
        let bad = find(Kind::Rules, "broken");
        assert!(!bad.valid);
        assert!(
            bad.error.as_ref().unwrap().contains("broken.<rule>"),
            "{bad:?}"
        );
        assert_eq!(all.len(), 11, "{all:#?}");
        let kinds: Vec<Kind> = all.iter().map(|e| e.kind).collect();
        let mut sorted = kinds.clone();
        sorted.sort_by_key(|k| *k as u8);
        assert_eq!(kinds, sorted, "grouped by kind");
    }

    #[test]
    fn missing_roots_are_empty() {
        let d = tempfile::tempdir().unwrap();
        assert!(scan_all(&roots(&d.path().join("none"))).is_empty());
    }
}
