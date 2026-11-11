//! A debounced inotify watch on integration directories (Phase 4 spec §2.2, §2.3): a package
//! install or removal shows up without a relog.
//!
//! Each directory is watched with its immediate sub-directories (so a manifest written into a
//! new folder is seen). A directory that doesn't exist yet is replaced by its nearest existing
//! ancestor until it appears. The watches are re-armed after every burst of events.

use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use inotify::{Inotify, WatchMask};
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
    let mut out: Vec<PathBuf> = Vec::new();
    let mut add = |p: PathBuf| {
        if !out.contains(&p) {
            out.push(p);
        }
    };
    for dir in dirs {
        if dir.is_dir() {
            add(dir.clone());
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

fn arm(watches: &mut inotify::Watches, dirs: &[PathBuf]) {
    for p in watch_paths(dirs) {
        // A path that vanished in between is caught by the next burst.
        let _ = watches.add(&p, mask());
    }
}

/// Watch `dirs`; a `()` arrives on the receiver after each burst of changes has been quiet for
/// `debounce`. The task runs on the current tokio runtime and ends when the receiver is dropped.
pub fn watch_dirs(dirs: Vec<PathBuf>, debounce: Duration) -> std::io::Result<mpsc::Receiver<()>> {
    let inotify = Inotify::init()?;
    let mut watches = inotify.watches();
    arm(&mut watches, &dirs);
    let mut stream = inotify.into_event_stream(vec![0u8; 16 * 1024])?;
    let (tx, rx) = mpsc::channel(1);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                ev = stream.next() => match ev {
                    Some(Ok(_)) => {}
                    _ => return,
                },
                () = tx.closed() => return,
            }
            // Wait for quiet, re-arming as folders come and go.
            loop {
                arm(&mut stream.watches(), &dirs);
                match tokio::time::timeout(debounce, stream.next()).await {
                    Ok(Some(Ok(_))) => continue,
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
