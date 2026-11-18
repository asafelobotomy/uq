use uqlip::config::Config;
use uqlip::daemon::Daemon;
use uqlip::ipc::Request;
use uqlip::privacy::{Privacy, Skip, choose};
use uqlip::store::{Added, Kind, Limits, Store};
use uqlip::watch::Captured;

fn store(limits: Limits) -> (tempfile::TempDir, Store) {
    let d = tempfile::tempdir().unwrap();
    let s = Store::open(d.path(), limits).unwrap();
    (d, s)
}

fn text(s: &mut Store, t: &str, now: u64) -> Added {
    s.add(
        Kind::Text,
        "text/plain;charset=utf-8",
        t.as_bytes(),
        None,
        now,
    )
    .unwrap()
}

#[test]
fn newest_first_and_duplicates_move_to_the_top() {
    let (_d, mut s) = store(Limits::default());
    let Added::New(a) = text(&mut s, "one", 1) else {
        panic!()
    };
    text(&mut s, "two", 2);
    assert_eq!(text(&mut s, "one", 3), Added::Bumped(a));
    let l = s.list("", None, 10);
    assert_eq!(
        l.iter().map(|e| e.preview.as_str()).collect::<Vec<_>>(),
        ["one", "two"]
    );
    assert_eq!(l[0].created, 3);
}

#[test]
fn pinned_come_first_and_survive_pruning_and_clearing() {
    let (_d, mut s) = store(Limits {
        max_items: 2,
        max_entry_bytes: 1000,
    });
    let Added::New(p) = text(&mut s, "keep me", 1) else {
        panic!()
    };
    s.set_pinned(p, true).unwrap();
    for (i, t) in ["a", "b", "c", "d"].iter().enumerate() {
        text(&mut s, t, 10 + i as u64);
    }
    let l = s.list("", None, 10);
    assert_eq!(
        l.iter().map(|e| e.preview.as_str()).collect::<Vec<_>>(),
        ["keep me", "d", "c"]
    );
    assert_eq!(s.clear(true).unwrap(), 2);
    assert_eq!(s.len(), 1);
    assert_eq!(s.clear(false).unwrap(), 1);
    assert!(s.is_empty());
}

#[test]
fn too_big_and_blank_copies_are_not_kept() {
    let (_d, mut s) = store(Limits {
        max_items: 5,
        max_entry_bytes: 4,
    });
    assert_eq!(text(&mut s, "12345", 1), Added::TooBig);
    assert_eq!(text(&mut s, "  \n", 1), Added::Empty);
    assert!(s.is_empty());
}

#[test]
fn history_survives_a_restart_and_orphans_are_swept() {
    let d = tempfile::tempdir().unwrap();
    {
        let mut s = Store::open(d.path(), Limits::default()).unwrap();
        text(&mut s, "hello", 1);
    }
    std::fs::write(d.path().join("blobs/999"), b"stray").unwrap();
    let s = Store::open(d.path(), Limits::default()).unwrap();
    assert_eq!(s.len(), 1);
    assert!(!d.path().join("blobs/999").exists());
}

#[test]
fn a_broken_index_starts_empty_instead_of_failing() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("blobs")).unwrap();
    std::fs::write(d.path().join("index.json"), b"{not json").unwrap();
    assert!(Store::open(d.path(), Limits::default()).unwrap().is_empty());
}

#[test]
fn files_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let (d, mut s) = store(Limits::default());
    let Added::New(id) = text(&mut s, "secret-ish", 1) else {
        panic!()
    };
    for p in [
        d.path().join("index.json"),
        d.path().join("blobs").join(id.to_string()),
    ] {
        assert_eq!(
            std::fs::metadata(p).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert_eq!(
        std::fs::metadata(d.path().join("blobs"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}

#[test]
fn search_finds_text_beyond_the_preview() {
    let (_d, mut s) = store(Limits::default());
    let long = format!("{} needle", "x".repeat(400));
    text(&mut s, &long, 1);
    text(&mut s, "other", 2);
    assert_eq!(s.list("needle", None, 10).len(), 1);
    assert_eq!(s.list("NEEDLE", Some(Kind::Image), 10).len(), 0);
}

#[test]
fn password_manager_copies_are_skipped() {
    let p = Privacy::default();
    let offered = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let plain = offered(&["text/plain"]);
    assert_eq!(p.check(false, &plain, None, None), Ok(()));
    let marked = offered(&["text/plain", "x-kde-passwordManagerHint"]);
    assert_eq!(
        p.check(false, &marked, Some("secret"), None),
        Err(Skip::SecretHint)
    );
    // Offered with no readable value is still treated as secret.
    assert_eq!(p.check(false, &marked, None, None), Err(Skip::SecretHint));
    assert_eq!(p.check(false, &marked, Some("normal"), None), Ok(()));
    assert_eq!(p.check(true, &plain, None, None), Err(Skip::Paused));
    assert_eq!(
        p.check(false, &plain, None, Some("KeePassXC")),
        Err(Skip::IgnoredApp)
    );
    assert_eq!(
        p.check(false, &plain, None, Some("keepassxc")),
        Err(Skip::IgnoredApp)
    );
}

#[test]
fn images_beat_text_and_unknown_types_are_ignored() {
    let o = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        choose(&o(&["text/plain", "image/png"])).unwrap().1,
        Kind::Image
    );
    assert_eq!(choose(&o(&["UTF8_STRING"])).unwrap().1, Kind::Text);
    assert!(choose(&o(&["application/x-foo"])).is_none());
}

fn daemon() -> (tempfile::TempDir, Daemon) {
    let (d, s) = store(Limits::default());
    (d, Daemon::new(s, Config::default()))
}

fn copy(d: &mut Daemon, t: &str, app: Option<&str>) {
    d.captured(
        Captured {
            offered: vec!["text/plain".into()],
            mime: "text/plain".into(),
            data: t.as_bytes().to_vec(),
        },
        app,
    );
}

#[test]
fn paused_or_disabled_daemon_records_nothing() {
    let (_d, mut d) = daemon();
    d.handle(Request::Pause { paused: true });
    copy(&mut d, "x", None);
    assert_eq!(d.status().count, 0);
    d.handle(Request::Pause { paused: false });
    copy(&mut d, "x", Some("kitty"));
    assert_eq!(d.status().count, 1);
    d.config.enabled = false;
    copy(&mut d, "y", None);
    assert_eq!(d.status().count, 1);
}

#[test]
fn requests_list_pin_delete_and_copy() {
    let (_d, mut d) = daemon();
    copy(&mut d, "alpha", Some("kitty"));
    copy(&mut d, "beta", None);
    let v = d.handle(Request::List {
        query: "alp".into(),
        kind: None,
        limit: 10,
    });
    let id = v["entries"][0]["id"].as_u64().unwrap();
    assert_eq!(v["entries"][0]["app"], "kitty");
    assert_eq!(d.handle(Request::Pin { id, pinned: true })["ok"], true);
    assert_eq!(d.status().pinned, 1);
    assert_eq!(d.handle(Request::Get { id })["text"], "alpha");

    // No clipboard connection yet: copy says so instead of pretending.
    assert_eq!(d.handle(Request::Copy { id })["ok"], false);
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let s2 = seen.clone();
    d.set_setter(move |m, data| s2.lock().unwrap().push((m, data)));
    assert_eq!(d.handle(Request::Copy { id })["ok"], true);
    let got = seen.lock().unwrap();
    assert_eq!(got[0].1, b"alpha");
    assert!(got[0].0.iter().any(|m| m == "UTF8_STRING"));
    drop(got);

    assert_eq!(d.handle(Request::Delete { id })["ok"], true);
    assert_eq!(d.handle(Request::Delete { id })["ok"], false);
}

#[test]
fn watchers_hear_about_changes() {
    let (_d, mut d) = daemon();
    let (tx, rx) = std::sync::mpsc::channel();
    d.add_watcher(tx);
    assert!(rx.recv().unwrap().contains("\"count\":0"));
    copy(&mut d, "one", None);
    assert!(rx.recv().unwrap().contains("\"count\":1"));
}

#[test]
fn expire_forgets_old_unpinned_entries_only() {
    let (_d, mut s) = store(Limits::default());
    text(&mut s, "old", 100);
    text(&mut s, "old pinned", 100);
    text(&mut s, "fresh", 10_000);
    let id = s.list("old pinned", None, 1)[0].id;
    s.set_pinned(id, true).unwrap();
    assert_eq!(s.expire(3600, 10_000).unwrap(), 1);
    let left: Vec<String> = s.list("", None, 10).into_iter().map(|e| e.preview).collect();
    assert_eq!(left, vec!["old pinned", "fresh"]);
    assert_eq!(s.expire(3600, 10_000).unwrap(), 0);
}
