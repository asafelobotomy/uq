//! Scanning the user theme folder.

use super::*;

#[test]
fn scan_reads_good_and_skips_bad() {
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    good(r, "user-good");
    fs::create_dir(r.join("user-onlydark")).unwrap();
    fs::write(r.join("user-onlydark/dark.toml"), dark_text()).unwrap();
    good(r, "ocean");
    put(r, "user-big", &" ".repeat(65_537), &light_text());
    put(
        r,
        "user-dim",
        &dark_text().replace("text_muted = \"#bdae93\"", "text_muted = \"#32302f\""),
        &light_text(),
    );
    fs::write(r.join("notes.txt"), "hi").unwrap();
    let s = scan(r);
    assert!(!s.root_is_link);
    assert_eq!(s.themes.len(), 1);
    assert_eq!(s.themes[0].dark.id, "user-good");
    assert_eq!(s.themes[0].light.id, "user-good");
    let got: Vec<_> = s
        .skipped
        .iter()
        .map(|k| (k.folder.as_deref().unwrap(), &k.problem))
        .collect();
    assert_eq!(got.len(), 4, "{got:?}");
    assert_eq!(got[0], ("ocean", &Problem::FolderName));
    assert_eq!(got[1], ("user-big", &Problem::Unreadable(Variant::Dark)));
    assert!(matches!(got[2], ("user-dim", Problem::HardToRead { .. })));
    assert_eq!(
        got[3],
        ("user-onlydark", &Problem::MissingVariant(Variant::Light))
    );
}

#[test]
fn scan_reads_the_author() {
    let root = tempfile::tempdir().unwrap();
    let with = |t: String| format!("author = \"Sam\"\n{t}");
    put(
        root.path(),
        "user-a",
        &with(dark_text()),
        &with(light_text()),
    );
    put(
        root.path(),
        "user-b",
        &with(dark_text()).replace("Sam", "Sam\\n"),
        &light_text(),
    );
    let s = scan(root.path());
    assert_eq!(s.themes.len(), 1);
    assert_eq!(s.themes[0].author.as_deref(), Some("Sam"));
    assert_eq!(s.skipped.len(), 1);
    assert_eq!(s.skipped[0].problem, Problem::Author);
}

#[test]
fn scan_skips_links_and_bad_names() {
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    good(r, "user-real");
    std::os::unix::fs::symlink(r.join("user-real"), r.join("user-folderlink")).unwrap();
    good(r, "user-filelink");
    fs::remove_file(r.join("user-filelink/light.toml")).unwrap();
    std::os::unix::fs::symlink(
        r.join("user-real/light.toml"),
        r.join("user-filelink/light.toml"),
    )
    .unwrap();
    // A pipe named dark.toml must be skipped, not waited on.
    fs::create_dir(r.join("user-pipe")).unwrap();
    let ok = std::process::Command::new("mkfifo")
        .arg(r.join("user-pipe/dark.toml"))
        .status()
        .unwrap()
        .success();
    assert!(ok, "mkfifo");
    fs::write(r.join("user-pipe/light.toml"), light_text()).unwrap();
    fs::create_dir(r.join("Bad Name!")).unwrap();

    let (tx, rx) = std::sync::mpsc::channel();
    let path = r.to_path_buf();
    std::thread::spawn(move || tx.send(scan(&path)).ok());
    let s = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the scan waited on a pipe");
    assert_eq!(s.themes.len(), 1);
    assert_eq!(s.themes[0].dark.id, "user-real");
    let by = |f: &str| {
        s.skipped
            .iter()
            .find(|k| k.folder.as_deref() == Some(f))
            .map(|k| k.problem.clone())
    };
    assert_eq!(by("user-folderlink"), Some(Problem::Link));
    assert_eq!(by("user-filelink"), Some(Problem::Link));
    assert_eq!(by("user-pipe"), Some(Problem::Unreadable(Variant::Dark)));
    let odd = s.skipped.iter().find(|k| k.folder.is_none()).unwrap();
    assert_eq!(odd.problem, Problem::FolderName);

    // A symlinked root reads nothing.
    let base = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(r, base.path().join("themes")).unwrap();
    let s = scan(&base.path().join("themes"));
    assert!(s.root_is_link);
    assert!(s.themes.is_empty() && s.skipped.is_empty());
    assert!(fingerprint(&base.path().join("themes")).is_empty());
    // An absent root is just empty.
    let s = scan(&base.path().join("nope"));
    assert!(!s.root_is_link && s.themes.is_empty());
}

#[test]
fn scan_caps_themes() {
    let root = tempfile::tempdir().unwrap();
    for i in 0..51 {
        good(root.path(), &format!("user-t{i:02}"));
    }
    let s = scan(root.path());
    assert_eq!(s.themes.len(), MAX_USER_THEMES);
    assert_eq!(s.skipped.len(), 1);
    assert_eq!(s.skipped[0].problem, Problem::TooMany);
}

#[test]
fn scan_looks_at_a_bounded_number_of_entries() {
    let root = tempfile::tempdir().unwrap();
    for i in 0..(MAX_SCANNED + 20) {
        fs::create_dir(root.path().join(format!("junk{i:03}"))).unwrap();
    }
    let s = scan(root.path());
    assert_eq!(s.skipped.len(), MAX_SCANNED);
}

#[test]
fn fingerprint_changes_only_with_files() {
    let root = tempfile::tempdir().unwrap();
    good(root.path(), "user-a");
    let before = fingerprint(root.path());
    assert_eq!(before.len(), 3, "the folder and its two files");
    assert_eq!(fingerprint(root.path()), before);
    fs::write(root.path().join("stray.txt"), "x").unwrap();
    assert_eq!(fingerprint(root.path()), before);
    let f = fs::OpenOptions::new()
        .write(true)
        .open(root.path().join("user-a/dark.toml"))
        .unwrap();
    f.set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000))
        .unwrap();
    assert_ne!(fingerprint(root.path()), before);
}

#[test]
fn fingerprint_follows_links_without_reading_them() {
    let root = tempfile::tempdir().unwrap();
    let before = fingerprint(root.path());
    std::os::unix::fs::symlink("/etc", root.path().join("user-etc")).unwrap();
    assert_ne!(fingerprint(root.path()), before);
}
