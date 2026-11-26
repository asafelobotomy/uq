//! Shared checks: api, paths, folders and validators.

use super::*;

#[test]
fn newer_uq_api_is_refused_before_strict_checks() {
    let text = "uq_api = 4\nid = \"x\"\nbrand_new_key = true\n";
    assert_eq!(parse_widget(text), Err(ManifestError::NeedsNewerUq(4)));
    assert!(
        parse_widget(text)
            .unwrap_err()
            .to_string()
            .contains("needs a newer uQ")
    );
    assert_eq!(parse_page(text), Err(ManifestError::NeedsNewerUq(4)));
}

#[test]
fn path_under_is_lexical_and_by_component() {
    let pre = command_prefixes();
    for good in ["/usr/bin/x", "/usr/lib/x/y", "/usr/lib/a b"] {
        assert!(path_under(good, &pre), "{good}");
    }
    for bad in [
        "/usr/bin",
        "/usr/bin/",
        "/usr/bin/./x",
        "/usr/bin/../sbin/x",
        "/usr//bin/x",
        "usr/bin/x",
        "/usr/libexec/x",
        "/usr/bin/x\0",
        "",
    ] {
        assert!(!path_under(bad, &pre), "{bad:?}");
    }
}

#[test]
fn folders_need_matching_ids_and_contained_files() {
    let tmp = tempfile::tempdir().unwrap();
    let r = fs::canonicalize(tmp.path()).unwrap();
    let page = |id: &str| format!("id = \"{id}\"\nname = \"N\"\nqml = \"Page.qml\"\n");
    folder(&r, "good", "page.toml", "Page.qml", &page("good"));
    folder(&r, "other", "page.toml", "Page.qml", &page("good"));
    folder(&r, "noqml", "page.toml", "Other.qml", &page("noqml"));
    let outside = tempfile::tempdir().unwrap();
    folder(
        outside.path(),
        "evil",
        "page.toml",
        "Page.qml",
        &page("evil"),
    );
    symlink(outside.path().join("evil"), r.join("evil")).unwrap();
    folder(&r, "qmllink", "page.toml", "Real.qml", &page("qmllink"));
    symlink(
        outside.path().join("evil/Page.qml"),
        r.join("qmllink/Page.qml"),
    )
    .unwrap();
    folder(&r, "inner", "page.toml", "Real.qml", &page("inner"));
    symlink("Real.qml", r.join("inner/Page.qml")).unwrap();
    fs::create_dir_all(r.join("big")).unwrap();
    fs::write(r.join("big/page.toml"), "#".repeat(70 * 1024)).unwrap();

    let ok = load_page(&r.join("good"), &r).unwrap();
    assert_eq!(ok.qml_path, r.join("good/Page.qml"));
    assert_eq!(ok.dir, r.join("good"));
    let inner = load_page(&r.join("inner"), &r).unwrap();
    assert_eq!(inner.qml_path, r.join("inner/Real.qml"));
    for (dir, needle) in [
        ("other", "folder name"),
        ("noqml", "qml"),
        ("evil", "outside"),
        ("qmllink", "outside"),
        ("big", "64 KiB"),
        ("missing", "No such file"),
    ] {
        let e = load_page(&r.join(dir), &r).unwrap_err().to_string();
        assert!(e.contains(needle), "{dir}: {e}");
    }

    folder(
        &r,
        "w",
        "widget.toml",
        "Widget.qml",
        "uq_api = 1\nid = \"w\"\nname = \"W\"\nslot = \"bar\"\nqml = \"Widget.qml\"\n",
    );
    assert_eq!(load_widget(&r.join("w"), &r).unwrap().manifest.id, "w");
    assert!(load_widget(&r.join("good"), &r).is_err(), "no widget.toml");
}

#[test]
fn requires_reports_missing_bins() {
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("here"), "").unwrap();
    let r = Requires {
        bus_name: None,
        bins: vec!["here".into(), "gone".into()],
    };
    assert_eq!(r.missing_bins(d.path()), ["gone"]);
}

#[test]
fn small_validators() {
    assert!(valid_id("a") && valid_id("scale-tail") && valid_id(&"a".repeat(40)));
    assert!(!valid_id("") && !valid_id("A") && !valid_id("a_b") && !valid_id(&"a".repeat(41)));
    assert!(valid_bus_name("org.a.B") && !valid_bus_name(":1.5") && !valid_bus_name("org"));
    assert!(valid_bin_name("tailscale") && valid_bin_name("g++"));
    assert!(!valid_bin_name("a/b") && !valid_bin_name(".hidden") && !valid_bin_name(""));
}
