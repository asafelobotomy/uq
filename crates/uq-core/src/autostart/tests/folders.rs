//! The folder read, on real temp dirs.

use super::super::scan::{MAX_PER_FOLDER, read_capped, read_folders};
use std::fs;
use std::time::{Duration, Instant};

const GOOD: &str = "[Desktop Entry]\nType=Application\nName=Good\nExec=good\n";

#[test]
fn read_folders_caps_and_filters() {
    let start = Instant::now();
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("user");
    let system = dir.path().join("system");
    let missing = dir.path().join("missing");
    fs::create_dir_all(&user).unwrap();
    fs::create_dir_all(&system).unwrap();
    for i in 0..205 {
        fs::write(user.join(format!("good-{i:03}.desktop")), GOOD).unwrap();
    }
    fs::write(user.join("bad name.desktop"), GOOD).unwrap();
    fs::write(user.join("notes.txt"), GOOD).unwrap();
    fs::write(
        user.join("big.desktop"),
        format!("{GOOD}{}", "#".repeat(70 * 1024)),
    )
    .unwrap();
    fs::create_dir(user.join("dir.desktop")).unwrap();
    std::os::unix::fs::symlink(user.join("good-000.desktop"), user.join("a-link.desktop")).unwrap();
    let made = std::process::Command::new("mkfifo")
        .arg(user.join("fifo.desktop"))
        .status()
        .unwrap();
    assert!(made.success());
    fs::write(
        system.join("good-000.desktop"),
        "[Desktop Entry]\nExec=sys\n",
    )
    .unwrap();
    fs::write(system.join("sys-only.desktop"), GOOD).unwrap();

    let found = read_folders(&user, &[missing, system.clone()]);
    let users: Vec<_> = found.iter().filter(|f| f.user.is_some()).collect();
    assert_eq!(users.len(), MAX_PER_FOLDER);
    assert_eq!(MAX_PER_FOLDER, 200);
    for skipped in [
        "bad name.desktop",
        "big.desktop",
        "fifo.desktop",
        "dir.desktop",
    ] {
        assert!(
            found.iter().all(|f| f.id != skipped || f.user.is_none()),
            "{skipped} was read"
        );
    }
    assert!(found.iter().all(|f| f.id != "bad name.desktop"));
    let link = found.iter().find(|f| f.id == "a-link.desktop").unwrap();
    assert_eq!(link.user.as_ref().unwrap().1, GOOD, "a symlink is followed");
    let shadowed = found.iter().find(|f| f.id == "good-000.desktop").unwrap();
    assert!(shadowed.user.is_some());
    assert_eq!(
        shadowed.system.as_ref().unwrap().0,
        system.join("good-000.desktop")
    );
    let sys_only = found.iter().find(|f| f.id == "sys-only.desktop").unwrap();
    assert!(sys_only.user.is_none() && sys_only.system.is_some());
    // Sorted by id, each id once.
    assert!(found.windows(2).all(|w| w[0].id < w[1].id));

    assert_eq!(read_capped(&user.join("fifo.desktop")), None);
    assert_eq!(read_capped(&user.join("big.desktop")), None);
    assert_eq!(read_capped(&user.join("dir.desktop")), None);
    assert_eq!(
        read_capped(&user.join("good-001.desktop")).as_deref(),
        Some(GOOD)
    );
    let exact = user.join("exact.desktop");
    fs::write(&exact, vec![b'#'; 64 * 1024]).unwrap();
    assert_eq!(read_capped(&exact).map(|t| t.len()), Some(64 * 1024));
    fs::write(&exact, b"Name=\xff\n").unwrap();
    assert_eq!(read_capped(&exact).as_deref(), Some("Name=\u{fffd}\n"));
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "{:?}",
        start.elapsed()
    );
}
