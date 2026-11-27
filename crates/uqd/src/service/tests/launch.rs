//! Launch counts.

use super::*;

#[tokio::test]
async fn record_launch_throttled() {
    let mut r = rig();
    let t0 = Instant::now();
    r.desktop.record_launch_at("a.desktop", t0);
    r.desktop
        .record_launch_at("a.desktop", t0 + Duration::from_secs(1));
    assert_eq!(counts(&r).get("a.desktop"), Some(&2));
    assert_eq!(r.desktop.launch_saves(), 1);
    assert_eq!(
        State::load(&r.desktop.paths().state_file)
            .launch_counts
            .get("a.desktop"),
        Some(&1),
        "second launch not yet on disk"
    );
    let evs = drain(&mut r.events);
    assert_eq!(evs.len(), 2);
    assert!(matches!(&evs[1], Event::LaunchCountsChanged(c) if c.get("a.desktop") == Some(&2)));
    // Too early: the flush does nothing. After 30 s it writes the dirty count.
    assert_eq!(
        r.desktop.launch_flush_due(t0 + Duration::from_secs(10)),
        Some(t0 + Duration::from_secs(30)),
        "due 30 s after the first save"
    );
    r.desktop
        .flush_launch_counts_at(t0 + Duration::from_secs(10));
    assert_eq!(r.desktop.launch_saves(), 1);
    r.desktop
        .flush_launch_counts_at(t0 + Duration::from_secs(31));
    assert_eq!(r.desktop.launch_saves(), 2);
    assert_eq!(
        State::load(&r.desktop.paths().state_file)
            .launch_counts
            .get("a.desktop"),
        Some(&2)
    );
    r.desktop
        .flush_launch_counts_at(t0 + Duration::from_secs(90));
    assert_eq!(r.desktop.launch_saves(), 2, "clean state is not rewritten");
    assert_eq!(
        r.desktop.launch_flush_due(t0 + Duration::from_secs(90)),
        None,
        "nothing to wait for"
    );
}

#[tokio::test]
async fn record_launch_opt_out() {
    let mut r = rig();
    r.desktop
        .set_setting(
            "privacy.record_launches",
            serde_json::json!(false),
            Origin::Settings,
        )
        .await
        .unwrap();
    drain(&mut r.events);
    r.desktop.record_launch("a.desktop");
    assert!(counts(&r).is_empty());
    assert!(drain(&mut r.events).is_empty());
}

#[tokio::test]
async fn launch_count_capped() {
    let mut r = rig();
    r.desktop
        .state
        .launch_counts
        .insert("a.desktop".into(), 10_000);
    r.desktop.record_launch("a.desktop");
    assert_eq!(counts(&r).get("a.desktop"), Some(&10_000));
}

#[tokio::test]
async fn record_launch_rejects_bad_ids() {
    let mut r = rig();
    r.desktop.record_launch("../evil");
    r.desktop.record_launch("");
    assert!(counts(&r).is_empty());
}

#[tokio::test]
async fn clear_launch_counts_saves_and_emits() {
    let mut r = rig();
    r.desktop.record_launch("a.desktop");
    drain(&mut r.events);
    r.desktop.clear_launch_counts();
    assert!(counts(&r).is_empty());
    assert!(
        State::load(&r.desktop.paths().state_file)
            .launch_counts
            .is_empty()
    );
    assert_eq!(
        drain(&mut r.events),
        vec![Event::LaunchCountsChanged(BTreeMap::new())]
    );
}

#[tokio::test]
async fn prune_drops_uninstalled() {
    let mut r = rig();
    r.desktop.record_launch("keep.desktop");
    r.desktop.record_launch("gone.desktop");
    r.desktop.prune_launch_counts(&|id| id == "keep.desktop");
    assert_eq!(counts(&r).keys().collect::<Vec<_>>(), vec!["keep.desktop"]);
    assert_eq!(
        State::load(&r.desktop.paths().state_file)
            .launch_counts
            .keys()
            .collect::<Vec<_>>(),
        vec!["keep.desktop"]
    );
}

#[test]
fn desktop_file_lookup() {
    let dir = tempfile::tempdir().unwrap();
    let apps = dir.path().join("applications");
    fs::create_dir_all(&apps).unwrap();
    fs::write(apps.join("foo.desktop"), "").unwrap();
    let dirs = [dir.path().to_path_buf()];
    assert!(desktop_file_installed("foo.desktop", &dirs));
    assert!(desktop_file_installed("foo", &dirs));
    assert!(!desktop_file_installed("bar.desktop", &dirs));
    assert!(!desktop_file_installed("../foo", &dirs));
}
