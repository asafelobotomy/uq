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
    write(r, "j-newer", &manifest("j-newer", "uq_api = 4\n"));
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
    // The startup scan is the one `scan_configured` shows; it isn't posted again.
    assert!(scan_configured().is_empty(), "nothing installed yet");
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(300), rx.recv())
            .await
            .is_err(),
        "no post at startup"
    );

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
