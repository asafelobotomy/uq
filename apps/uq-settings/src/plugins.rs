//! Plugin settings pages (spec §2.5): `<dir>/<id>/page.toml` plus one QML file. Read only from
//! the system directory (or `--plugin-dir`); a bad page is skipped with a reason, never fatal.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::registry::{self, OwnedSearchEntry};

/// The only directory read by default: root-owned, never a user-writable one.
pub const SYSTEM_DIR: &str = "/usr/share/uq/settings/pages";

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_SETTINGS: usize = 200;
const MAX_KEYWORDS: usize = 32;

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
    #[allow(dead_code, reason = "the dbus context helper comes in slice 6")]
    pub dbus_name: Option<String>,
    pub keywords: Vec<String>,
    pub settings: Vec<OwnedSearchEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    id: String,
    name: String,
    icon: Option<String>,
    category: Option<String>,
    qml: String,
    dbus_name: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    setting: Vec<RawSetting>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSetting {
    id: String,
    label: String,
    #[serde(default)]
    keywords: Vec<String>,
}

/// `^[a-z0-9-]{1,40}$`
fn valid_page_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// `^[A-Za-z0-9._-]{1,64}$`
fn valid_setting_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn valid_text(s: &str, max: usize) -> bool {
    (1..=max).contains(&s.chars().count()) && !s.chars().any(char::is_control)
}

fn valid_keywords(words: &[String]) -> bool {
    words.len() <= MAX_KEYWORDS && words.iter().all(|k| valid_text(k, 64))
}

/// A bare file name ending `.qml`: no directories, no `..`, no hidden or empty stem.
fn valid_qml_name(name: &str) -> bool {
    name.len() <= 100
        && name.len() > ".qml".len()
        && name.ends_with(".qml")
        && !name.starts_with('.')
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | '%' | '?' | '#'))
}

fn parse(dir: &Path, text: &str) -> Result<PluginPage, String> {
    let m: Manifest = toml::from_str(text).map_err(|e| format!("page.toml: {}", e.message()))?;
    if !valid_page_id(&m.id) {
        return Err(format!("id {:?} must match [a-z0-9-]{{1,40}}", m.id));
    }
    if let Some(c) = &m.category
        && c != "apps"
    {
        return Err(format!("category must be \"apps\", not {c:?}"));
    }
    if !valid_text(&m.name, 100) {
        return Err("name must be 1-100 characters without control characters".into());
    }
    if let Some(icon) = &m.icon
        && !valid_text(icon, 100)
    {
        return Err("icon must be 1-100 characters without control characters".into());
    }
    if !valid_qml_name(&m.qml) {
        return Err(format!(
            "qml {:?} must be a bare file name ending .qml",
            m.qml
        ));
    }
    if let Some(n) = &m.dbus_name
        && !valid_text(n, 255)
    {
        return Err("dbus_name must be 1-255 characters without control characters".into());
    }
    if !valid_keywords(&m.keywords) {
        return Err("keywords: at most 32, each 1-64 characters".into());
    }
    if m.setting.len() > MAX_SETTINGS {
        return Err(format!("at most {MAX_SETTINGS} [[setting]] entries"));
    }
    let mut settings: Vec<OwnedSearchEntry> = Vec::new();
    for s in m.setting {
        if !valid_setting_id(&s.id) {
            return Err(format!(
                "setting id {:?} must match [A-Za-z0-9._-]{{1,64}}",
                s.id
            ));
        }
        if settings.iter().any(|o| o.id == s.id) {
            return Err(format!("duplicate setting id {:?}", s.id));
        }
        if !valid_text(&s.label, 200) || !valid_keywords(&s.keywords) {
            return Err(format!("setting {:?}: bad label or keywords", s.id));
        }
        settings.push(OwnedSearchEntry {
            id: s.id,
            label: s.label,
            keywords: s.keywords,
        });
    }
    Ok(PluginPage {
        qml_path: dir.join(&m.qml),
        id: m.id,
        name: m.name,
        icon: m.icon.unwrap_or_else(|| "application-x-executable".into()),
        dbus_name: m.dbus_name,
        keywords: m.keywords,
        settings,
    })
}

/// Load one page directory. `root` is the canonical plugin directory. A page directory, its
/// manifest or its QML file that is a symlink resolving outside it is refused, and `qml_path` is
/// the canonical path of an existing regular file.
///
/// This is hygiene, not a sandbox: plugin QML can still `import "../x"` or read any file the
/// user can.
fn load(dir: &Path, root: &Path) -> Result<PluginPage, String> {
    let dir = fs::canonicalize(dir).map_err(|e| format!("{e}"))?;
    if !dir.starts_with(root) {
        return Err("resolves outside the plugin directory".into());
    }
    let manifest =
        fs::canonicalize(dir.join("page.toml")).map_err(|e| format!("page.toml: {e}"))?;
    if !manifest.starts_with(&dir) {
        return Err("page.toml resolves outside the page directory".into());
    }
    let meta = fs::metadata(&manifest).map_err(|e| format!("page.toml: {e}"))?;
    if !meta.is_file() || meta.len() > MAX_MANIFEST_BYTES {
        return Err("page.toml is not a regular file under 64 KiB".into());
    }
    let text = fs::read_to_string(&manifest).map_err(|e| format!("page.toml: {e}"))?;
    let mut page = parse(&dir, &text)?;
    let qml = fs::canonicalize(&page.qml_path).map_err(|e| {
        format!(
            "qml {:?}: {e}",
            page.qml_path.file_name().unwrap_or_default()
        )
    })?;
    if !qml.starts_with(&dir) || !qml.is_file() {
        return Err("qml resolves outside the page directory or is not a file".into());
    }
    page.qml_path = qml;
    Ok(page)
}

/// Every valid plugin page under `dir`, in directory-name order, and one
/// `"<dir name>: <reason>"` line for each directory skipped. An id that collides with a core
/// page or an earlier plugin is skipped. A missing `dir` is no plugins, not an error.
pub fn scan(dir: &Path) -> (Vec<PluginPage>, Vec<String>) {
    let mut pages: Vec<PluginPage> = Vec::new();
    let mut skipped = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                skipped.push(format!("{}: {e}", dir.display()));
            }
            return (pages, skipped);
        }
    };
    let root = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let mut names: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    names.sort();
    for path in names {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if !path.is_dir() {
            continue;
        }
        match load(&path, &root) {
            Ok(page) if registry::find(&page.id).is_some() => {
                skipped.push(format!("{name}: id {:?} is a core page", page.id));
            }
            Ok(page) if pages.iter().any(|p| p.id == page.id) => {
                skipped.push(format!("{name}: duplicate id {:?}", page.id));
            }
            Ok(page) => pages.push(page),
            Err(reason) => skipped.push(format!("{name}: {reason}")),
        }
    }
    (pages, skipped)
}

/// Scan the configured directory, logging what was skipped.
pub fn scan_configured() -> Vec<PluginPage> {
    let (pages, skipped) = scan(&absolute(dir()));
    for line in skipped {
        eprintln!("uq-settings: skipping plugin page {line}");
    }
    pages
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

    #[test]
    fn scan_skips_bad_manifests() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        write(
            r,
            "a-good",
            &manifest(
                "good",
                "icon = \"folder\"\ncategory = \"apps\"\ndbus_name = \"org.example.Good\"\n\
                 keywords = [\"tools\"]\n[[setting]]\nid = \"mode\"\nlabel = \"Mode\"\n\
                 keywords = [\"kind\"]\n",
            ),
        );
        fs::create_dir_all(r.join("b-no-manifest")).unwrap();
        fs::write(r.join("a-file.txt"), "not a directory").unwrap();
        write(r, "c-unknown-key", &manifest("unknown", "surprise = 1\n"));
        write(r, "d-dup-1", &manifest("dup", ""));
        write(r, "d-dup-2", &manifest("dup", ""));
        write(r, "e-core", &manifest("appearance", ""));
        write(
            r,
            "f-dotdot",
            &manifest("dotdot", "").replace("Page.qml", "../x.qml"),
        );
        write(
            r,
            "f-slash",
            &manifest("slash", "").replace("Page.qml", "sub/x.qml"),
        );
        write(
            r,
            "f-ext",
            &manifest("ext", "").replace("Page.qml", "Page.js"),
        );
        write(r, "g-bad-id", &manifest("Bad_Id", ""));
        write(r, "g-category", &manifest("cat", "category = \"system\"\n"));
        write(r, "h-syntax", "id = \n");
        write(
            r,
            "i-dup-setting",
            &manifest(
                "dupset",
                "[[setting]]\nid = \"a\"\nlabel = \"A\"\n[[setting]]\nid = \"a\"\nlabel = \"B\"\n",
            ),
        );
        write(
            r,
            "j-unknown-setting-key",
            &manifest("us", "[[setting]]\nid = \"a\"\nlabel = \"A\"\nx = 1\n"),
        );
        write(r, "z-after", &manifest("after", ""));

        // Symlinks: a page directory pointing outside, a qml file pointing outside, a manifest
        // pointing outside, and a missing qml file.
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "evil", &manifest("evil", ""));
        symlink(outside.path().join("evil"), r.join("k-dir-link")).unwrap();
        write(r, "l-qml-link", &manifest("qmllink", ""));
        fs::remove_file(r.join("l-qml-link/Page.qml")).unwrap();
        fs::write(outside.path().join("Out.qml"), "import QtQuick\nItem {}\n").unwrap();
        symlink(
            outside.path().join("Out.qml"),
            r.join("l-qml-link/Page.qml"),
        )
        .unwrap();
        write(r, "m-toml-link", "");
        fs::remove_file(r.join("m-toml-link/page.toml")).unwrap();
        fs::write(outside.path().join("x.toml"), manifest("tomllink", "")).unwrap();
        symlink(
            outside.path().join("x.toml"),
            r.join("m-toml-link/page.toml"),
        )
        .unwrap();
        write(r, "n-no-qml", &manifest("noqml", ""));
        fs::remove_file(r.join("n-no-qml/Page.qml")).unwrap();
        // A link that stays inside its own page directory is fine.
        write(r, "o-inner-link", &manifest("inner", ""));
        fs::rename(
            r.join("o-inner-link/Page.qml"),
            r.join("o-inner-link/Real.qml"),
        )
        .unwrap();
        symlink("Real.qml", r.join("o-inner-link/Page.qml")).unwrap();

        let (pages, skipped) = scan(r);
        let ids: Vec<_> = pages.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            ["good", "dup", "inner", "after"],
            "in directory-name order"
        );
        let r_canon = fs::canonicalize(r).unwrap();
        assert_eq!(pages[0].qml_path, r_canon.join("a-good/Page.qml"));
        assert_eq!(pages[2].qml_path, r_canon.join("o-inner-link/Real.qml"));
        assert_eq!(pages[0].icon, "folder");
        assert_eq!(pages[0].dbus_name.as_deref(), Some("org.example.Good"));
        assert_eq!(pages[0].settings[0].keywords, ["kind"]);
        let joined = skipped.join("\n");
        for dir in [
            "b-no-manifest",
            "c-unknown-key",
            "d-dup-2",
            "e-core",
            "f-dotdot",
            "f-slash",
            "f-ext",
            "g-bad-id",
            "g-category",
            "h-syntax",
            "i-dup-setting",
            "j-unknown-setting-key",
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
        assert_eq!(skipped.len(), 16, "{joined}");
        assert!(joined.contains("unknown field"), "{joined}");
    }

    #[test]
    fn a_missing_directory_is_no_plugins() {
        let (pages, skipped) = scan(Path::new("/nonexistent/uq/pages"));
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
