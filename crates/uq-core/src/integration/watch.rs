//! A debounced inotify watch on integration directories (Phase 4 spec §2.2, §2.3): a package
//! install or removal shows up without a relog.
//!
//! Each directory is watched with its immediate sub-directories (so a manifest written into a
//! new folder is seen). A directory that doesn't exist yet is replaced by its nearest existing
//! ancestor until it appears. The watches are re-armed only when an event can change which
//! paths need watching (a directory created, moved in or removed, a watched path gone, or a
//! queue overflow); file writes in an install burst leave them alone.

use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use inotify::{EventMask, Inotify, WatchMask};
use tokio::sync::mpsc;

/// The quiet time after the last event before a change is reported (§2.2).
pub const DEBOUNCE: Duration = Duration::from_secs(1);

fn mask() -> WatchMask {
    WatchMask::CREATE
        | WatchMask::DELETE
        | WatchMask::MOVED_FROM
        | WatchMask::MOVED_TO
        | WatchMask::CLOSE_WRITE
        | WatchMask::ATTRIB
        | WatchMask::DELETE_SELF
        | WatchMask::MOVE_SELF
}

/// The paths to watch for `dirs` now: each existing directory and its sub-directories, or the
/// nearest existing ancestor of a missing one.
pub fn watch_paths(dirs: &[PathBuf]) -> Vec<PathBuf> {
    watch_paths_with(dirs, true)
}

/// As `watch_paths`; without `subdirs` only the directories themselves.
fn watch_paths_with(dirs: &[PathBuf], subdirs: bool) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut add = |p: PathBuf| {
        if !out.contains(&p) {
            out.push(p);
        }
    };
    for dir in dirs {
        if dir.is_dir() {
            add(dir.clone());
            if !subdirs {
                continue;
            }
            if let Ok(rd) = std::fs::read_dir(dir) {
                let mut subs: Vec<PathBuf> = rd
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect();
                subs.sort();
                subs.into_iter().for_each(&mut add);
            }
        } else if let Some(anc) = dir.ancestors().skip(1).find(|a| a.is_dir()) {
            add(anc.to_path_buf());
        }
    }
    out
}

fn arm(watches: &mut inotify::Watches, dirs: &[PathBuf], subdirs: bool) {
    for p in watch_paths_with(dirs, subdirs) {
        // A path that vanished in between is caught by the next burst.
        let _ = watches.add(&p, mask());
    }
}

/// Whether an event with `mask` can change `watch_paths`: a sub-directory (or a missing
/// directory, or one of its ancestors) appeared or went, a watched path itself went, or events
/// were lost. A file written, created, removed or touched never does.
fn reshapes(mask: EventMask) -> bool {
    mask.intersects(
        EventMask::DELETE_SELF | EventMask::MOVE_SELF | EventMask::IGNORED | EventMask::Q_OVERFLOW,
    ) || (mask.contains(EventMask::ISDIR)
        && mask.intersects(
            EventMask::CREATE | EventMask::MOVED_TO | EventMask::DELETE | EventMask::MOVED_FROM,
        ))
}

/// Watch `dirs`; a `()` arrives on the receiver after each burst of changes has been quiet for
/// `debounce`. The task runs on the current tokio runtime and ends when the receiver is dropped.
pub fn watch_dirs(dirs: Vec<PathBuf>, debounce: Duration) -> std::io::Result<mpsc::Receiver<()>> {
    watch(dirs, debounce, true)
}

/// As `watch_dirs`, but only the directories themselves, not their sub-directories: for a busy
/// folder such as `~/.config` where only its own entries matter.
pub fn watch_dirs_flat(
    dirs: Vec<PathBuf>,
    debounce: Duration,
) -> std::io::Result<mpsc::Receiver<()>> {
    watch(dirs, debounce, false)
}

fn watch(
    dirs: Vec<PathBuf>,
    debounce: Duration,
    subdirs: bool,
) -> std::io::Result<mpsc::Receiver<()>> {
    let inotify = Inotify::init()?;
    let mut watches = inotify.watches();
    arm(&mut watches, &dirs, subdirs);
    let mut stream = inotify.into_event_stream(vec![0u8; 16 * 1024])?;
    let (tx, rx) = mpsc::channel(1);
    tokio::spawn(async move {
        loop {
            let mut ev = tokio::select! {
                ev = stream.next() => match ev {
                    Some(Ok(e)) => e.mask,
                    _ => return,
                },
                () = tx.closed() => return,
            };
            // Wait for quiet, re-arming as folders come and go (so writes into a new folder
            // still extend the burst).
            loop {
                if reshapes(ev) {
                    arm(&mut stream.watches(), &dirs, subdirs);
                }
                match tokio::time::timeout(debounce, stream.next()).await {
                    Ok(Some(Ok(e))) => ev = e.mask,
                    Ok(_) => return,
                    Err(_) => break,
                }
            }
            if tx.send(()).await.is_err() {
                return;
            }
        }
    });
    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn changed(rx: &mut mpsc::Receiver<()>, within: Duration) -> bool {
        tokio::time::timeout(within, rx.recv()).await.is_ok()
    }

    #[test]
    fn only_folder_changes_re_arm() {
        for m in [
            EventMask::CREATE | EventMask::ISDIR,
            EventMask::MOVED_TO | EventMask::ISDIR,
            EventMask::DELETE | EventMask::ISDIR,
            EventMask::MOVED_FROM | EventMask::ISDIR,
            EventMask::DELETE_SELF,
            EventMask::MOVE_SELF,
            EventMask::IGNORED,
            EventMask::Q_OVERFLOW,
        ] {
            assert!(reshapes(m), "{m:?}");
        }
        for m in [
            EventMask::CLOSE_WRITE,
            EventMask::CREATE,
            EventMask::DELETE,
            EventMask::MOVED_TO,
            EventMask::ATTRIB,
            EventMask::ATTRIB | EventMask::ISDIR,
        ] {
            assert!(!reshapes(m), "{m:?}");
        }
    }

    #[tokio::test]
    async fn a_manifest_in_a_new_nested_folder_is_seen() {
        // Watching an ancestor of a missing directory: the folders appear one by one, then
        // the manifest is written much later than the debounce.
        let d = tempfile::tempdir().unwrap();
        let pages = d.path().join("a/b/pages");
        let debounce = Duration::from_millis(150);
        let mut rx = watch_dirs(vec![pages.clone()], debounce).unwrap();
        std::fs::create_dir_all(pages.join("x")).unwrap();
        assert!(changed(&mut rx, Duration::from_secs(3)).await);
        while changed(&mut rx, Duration::from_millis(400)).await {}
        std::fs::write(pages.join("x/page.toml"), "id = \"x\"\n").unwrap();
        assert!(
            changed(&mut rx, Duration::from_secs(3)).await,
            "the new folder is watched"
        );
    }

    #[tokio::test]
    async fn flat_ignores_sub_directories() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("app")).unwrap();
        let debounce = Duration::from_millis(150);
        let mut rx = watch_dirs_flat(vec![d.path().to_path_buf()], debounce).unwrap();
        std::fs::write(d.path().join("app/state"), "x").unwrap();
        assert!(!changed(&mut rx, Duration::from_millis(500)).await, "inside a sub-folder");
        std::fs::write(d.path().join("x-flags.conf"), "x").unwrap();
        assert!(changed(&mut rx, Duration::from_secs(3)).await, "its own entry");
        assert_eq!(
            watch_paths_with(&[d.path().to_path_buf()], false),
            [d.path().to_path_buf()]
        );
    }

    #[test]
    fn missing_dirs_fall_back_to_an_ancestor() {
        let d = tempfile::tempdir().unwrap();
        let pages = d.path().join("settings/pages");
        assert_eq!(watch_paths(std::slice::from_ref(&pages)), [d.path()]);
        std::fs::create_dir_all(pages.join("a")).unwrap();
        assert_eq!(
            watch_paths(std::slice::from_ref(&pages)),
            [pages.clone(), pages.join("a")]
        );
    }

    #[tokio::test]
    async fn reports_installs_and_removals_once_quiet() {
        let d = tempfile::tempdir().unwrap();
        let pages = d.path().join("share/pages");
        let debounce = Duration::from_millis(200);
        let mut rx = watch_dirs(vec![pages.clone()], debounce).unwrap();
        assert!(
            !changed(&mut rx, Duration::from_millis(400)).await,
            "quiet at start"
        );

        // The directory itself appears, then a page folder, then its manifest: one report or a
        // few, but at least one, and none once quiet.
        std::fs::create_dir_all(pages.join("x")).unwrap();
        assert!(
            changed(&mut rx, Duration::from_secs(3)).await,
            "the folder appeared"
        );
        while changed(&mut rx, Duration::from_millis(500)).await {}
        std::fs::write(pages.join("x/page.toml"), "id = \"x\"\n").unwrap();
        assert!(
            changed(&mut rx, Duration::from_secs(3)).await,
            "a manifest written"
        );
        while changed(&mut rx, Duration::from_millis(500)).await {}

        std::fs::remove_dir_all(pages.join("x")).unwrap();
        assert!(
            changed(&mut rx, Duration::from_secs(3)).await,
            "the page removed"
        );
    }

    #[tokio::test]
    async fn a_burst_is_one_report() {
        let d = tempfile::tempdir().unwrap();
        let debounce = Duration::from_millis(300);
        let mut rx = watch_dirs(vec![d.path().to_path_buf()], debounce).unwrap();
        for i in 0..10 {
            std::fs::write(d.path().join(format!("f{i}")), "x").unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(changed(&mut rx, Duration::from_secs(3)).await);
        assert!(
            !changed(&mut rx, Duration::from_millis(600)).await,
            "one report per burst"
        );
    }
}
