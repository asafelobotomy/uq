//! Override files and races with files swapped by hand.

use super::*;

#[test]
fn override_written_with_marker_and_source() {
    let d = dirs();
    put(&source(&d), SOURCE);
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::Written);
    let t = read(&target(&d));
    assert!(t.starts_with(OVERRIDE_HEADER), "{t}");
    assert!(t.contains("X-uQ-Override=true"), "{t}");
    assert!(
        t.contains(&format!("X-uQ-Source={}", source(&d).display())),
        "{t}"
    );
    assert!(t.contains("Exec=env -- "), "{t}");
    assert!(is_ours(&target(&d), MarkerKind::Override));
    no_temps(&target(&d));
}

#[test]
fn override_regenerated_when_source_changes() {
    let d = dirs();
    put(&source(&d), SOURCE);
    sync_overrides(&d.p, &fix(true), false);
    put(
        &source(&d),
        &SOURCE.replace("/usr/bin/code", "/opt/code/bin/code"),
    );
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::Written);
    let t = read(&target(&d));
    assert!(
        t.contains("/opt/code/bin/code") && !t.contains("/usr/bin/code"),
        "{t}"
    );
}

#[test]
fn override_deleted_when_fix_removed() {
    let d = dirs();
    put(&source(&d), SOURCE);
    sync_overrides(&d.p, &fix(true), false);
    assert!(present(&target(&d)));
    let r = sync_overrides(&d.p, &fix(false), false);
    assert_eq!(state_of(&r), OverrideState::Off);
    assert!(!present(&target(&d)), "elsewhere turned off");

    sync_overrides(&d.p, &fix(true), false);
    assert!(present(&target(&d)));
    assert!(remove_overrides(&d.p, [CODE], false).is_empty());
    assert!(!present(&target(&d)), "the fix removed");

    sync_overrides(&d.p, &fix(true), false);
    assert_eq!(marked_overrides(&d.p), vec![CODE.to_string()]);
}

#[test]
fn override_lossy_source_reported() {
    let d = dirs();
    put(&source(&d), &SOURCE.replace("%F", "--file=%f"));
    let r = sync_overrides(&d.p, &fix(true), false);
    assert!(matches!(state_of(&r), OverrideState::Error(_)), "{r:?}");
    assert!(!present(&target(&d)));
}

#[test]
fn override_without_source_reported() {
    let d = dirs();
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::NoSource);
    assert!(!present(&target(&d)));
}

#[test]
fn never_touches_unmarked_override() {
    let d = dirs();
    put(&source(&d), SOURCE);
    put(&target(&d), MINE);
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::OwnFile);
    assert_eq!(read(&target(&d)), MINE);
    sync_overrides(&d.p, &fix(false), false);
    remove_overrides(&d.p, [CODE], false);
    assert_eq!(read(&target(&d)), MINE);
}

#[test]
fn never_follows_override_symlink() {
    let d = dirs();
    put(&source(&d), SOURCE);
    // A marked file outside the folder, linked in: a symlink is never uQ's.
    let outside = d.root.join("elsewhere.desktop");
    let marked = format!("{OVERRIDE_HEADER}\n{SOURCE}X-uQ-Override=true\n");
    put(&outside, &marked);
    std::fs::create_dir_all(target(&d).parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&outside, target(&d)).unwrap();
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::OwnFile);
    sync_overrides(&d.p, &fix(false), false);
    remove_overrides(&d.p, [CODE], false);
    let meta = std::fs::symlink_metadata(target(&d)).unwrap();
    assert!(meta.file_type().is_symlink());
    assert_eq!(read(&outside), marked);
}

#[test]
fn is_ours_never_reads_a_symlink_swapped_in_after_the_check() {
    let d = dirs();
    let marked = format!("{OVERRIDE_HEADER}\n{SOURCE}X-uQ-Override=true\n");
    let outside = d.root.join("elsewhere.desktop");
    put(&outside, &marked);
    put(&target(&d), &marked);
    assert!(is_ours(&target(&d), MarkerKind::Override));
    // Between the no-follow check and the read, the file becomes a symlink to a marked file.
    let to = outside.clone();
    hook::on_read(move |p| {
        std::fs::remove_file(p).unwrap();
        std::os::unix::fs::symlink(&to, p).unwrap();
    });
    assert!(!is_ours(&target(&d), MarkerKind::Override));
    let meta = std::fs::symlink_metadata(target(&d)).unwrap();
    assert!(meta.file_type().is_symlink());
}

#[test]
fn never_deletes_file_swapped_after_check() {
    let d = dirs();
    put(&source(&d), SOURCE);
    sync_overrides(&d.p, &fix(true), false);
    hook::once(|p| std::fs::write(p, MINE).unwrap());
    let r = sync_overrides(&d.p, &fix(false), false);
    assert_eq!(state_of(&r), OverrideState::Off);
    assert_eq!(read(&target(&d)), MINE);
}

#[test]
fn never_replaces_file_swapped_after_check() {
    // A marked file, swapped for the user's own between the check and the write.
    let d = dirs();
    put(&source(&d), SOURCE);
    sync_overrides(&d.p, &fix(true), false);
    put(&source(&d), &SOURCE.replace("/usr/bin/code", "/opt/code"));
    hook::once(|p| {
        std::fs::remove_file(p).unwrap();
        std::fs::write(p, MINE).unwrap();
    });
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::OwnFile);
    assert_eq!(read(&target(&d)), MINE);
    no_temps(&target(&d));

    // A file that appeared between the check and the write.
    let d = dirs();
    put(&source(&d), SOURCE);
    hook::once(|p| std::fs::write(p, MINE).unwrap());
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::OwnFile);
    assert_eq!(read(&target(&d)), MINE);
    no_temps(&target(&d));
}

#[test]
fn override_path_stays_in_folder() {
    let d = dirs();
    let dir = d.p.data_home.join("applications");
    assert_eq!(safe_join(&dir, CODE), Some(dir.join(CODE)));
    for bad in [
        "../x.desktop",
        "a/b.desktop",
        "/etc/x.desktop",
        "..desktop",
        ".x.desktop",
        "",
    ] {
        assert_eq!(safe_join(&dir, bad), None, "{bad}");
    }
    assert_eq!(override_target(&d.p, "../../evil.desktop"), None);
}

#[test]
fn dev_session_override_sync_touches_nothing() {
    let d = dirs();
    put(&source(&d), SOURCE);
    sync_overrides(&d.p, &fix(true), false);
    let before = read(&target(&d));
    let r = sync_overrides(&d.p, &fix(false), true);
    assert_eq!(state_of(&r), OverrideState::Off);
    assert!(remove_overrides(&d.p, [CODE], true).is_empty());
    assert_eq!(read(&target(&d)), before, "a marked override stays");

    let d = dirs();
    put(&source(&d), SOURCE);
    let r = sync_overrides(&d.p, &fix(true), true);
    assert!(matches!(state_of(&r), OverrideState::Error(_)), "{r:?}");
    assert!(!present(&target(&d)), "none is written");
}
