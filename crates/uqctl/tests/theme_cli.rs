//! `uqctl theme` end to end: the real binary against a real uqd on a private bus, with every home
//! under a temporary directory. Never touches the user's session.

mod common;

use common::{Bus, connect, private_bus, serve_uqd};
use std::path::{Path, PathBuf};
use uq_core::client::DesktopProxy;
use uq_core::paths::Paths;
use uq_core::theme::uqtheme;

/// One "home": its own bus and uqd.
struct Home {
    bus: Bus,
    root: tempfile::TempDir,
    _uqd: zbus::Connection,
}

async fn home() -> Home {
    let bus = private_bus();
    let root = tempfile::tempdir().unwrap();
    let uqd = serve_uqd(&bus, root.path()).await;
    Home {
        bus,
        root,
        _uqd: uqd,
    }
}

impl Home {
    fn uqctl(&self, args: &[&str]) -> std::process::Output {
        std::process::Command::new(env!("CARGO_BIN_EXE_uqctl"))
            .args(args)
            .env("DBUS_SESSION_BUS_ADDRESS", &self.bus.address)
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .unwrap()
    }

    fn user_themes(&self) -> PathBuf {
        Paths::under(self.root.path()).user_themes_dir
    }

    /// Every file under the user themes folder with its bytes, sorted.
    fn folder(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut out = Vec::new();
        let root = self.user_themes();
        if let Ok(rd) = std::fs::read_dir(&root) {
            for dir in rd.flatten() {
                for f in std::fs::read_dir(dir.path())
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    let rel = f.path().strip_prefix(&root).unwrap().to_path_buf();
                    out.push((rel, std::fs::read(f.path()).unwrap()));
                }
            }
        }
        out.sort();
        out
    }

    async fn restore_points(&self) -> usize {
        let conn = connect(&self.bus).await;
        let proxy = DesktopProxy::new(&conn).await.unwrap();
        let text = proxy.list_restore_points().await.unwrap();
        serde_json::from_str::<Vec<serde_json::Value>>(&text)
            .unwrap()
            .len()
    }
}

fn stdout(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn bad_corpus() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../uq-core/tests/fixtures/uqtheme/bad");
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "uqtheme"))
        .collect();
    v.sort();
    v
}

const SYSTEM: [&str; 7] = [
    "dracula", "forest", "graphite", "gruvbox", "matcha", "rosewood", "velvet",
];

/// Review Focus 6: every system theme exports from home A and imports into a fresh home B, where
/// it is a user theme whose files are what the export says.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_import_on_a_second_home() {
    let a = home().await;
    let b = home().await;
    let out = tempfile::tempdir().unwrap();
    let mut texts = Vec::new();
    for id in SYSTEM {
        let file = out.path().join(format!("{id}.uqtheme"));
        let o = a.uqctl(&["theme", "export", id, file.to_str().unwrap()]);
        assert!(o.status.success(), "{id}: {}", stderr(&o));
        let o = b.uqctl(&["theme", "import", file.to_str().unwrap()]);
        assert!(o.status.success(), "{id}: {}", stderr(&o));
        assert_eq!(stdout(&o).trim(), format!("Added theme: user-{id}"));
        texts.push((id, std::fs::read_to_string(&file).unwrap()));
    }
    let list = b.uqctl(&["theme", "list", "--long"]);
    let list = stdout(&list);
    for id in SYSTEM {
        let row = list
            .lines()
            .find(|l| l.contains(&format!("user-{id} ")))
            .unwrap_or_else(|| panic!("no user-{id} in\n{list}"));
        assert!(row.ends_with("(yours)"), "{row}");
    }
    assert_eq!(list.matches("(yours)").count(), 7, "{list}");
    for (id, text) in texts {
        let want = uqtheme::install_files(&uqtheme::parse(&text).unwrap());
        for (name, body) in want.0 {
            let path = b.user_themes().join(format!("user-{id}")).join(&name);
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                body,
                "{id}/{name:?}"
            );
        }
    }
    // A second export of an import is the same file (stable round trip).
    let again = out.path().join("again.uqtheme");
    let o = b.uqctl(&["theme", "export", "user-graphite", again.to_str().unwrap()]);
    assert!(o.status.success(), "{}", stderr(&o));
    let first = std::fs::read_to_string(out.path().join("graphite.uqtheme")).unwrap();
    assert_eq!(std::fs::read_to_string(again).unwrap(), first);
}

/// Review Focus 3: every hostile file is refused with one line and changes nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bad_corpus_through_cli_changes_nothing() {
    let b = home().await;
    let good = tempfile::tempdir().unwrap();
    let file = good.path().join("g.uqtheme");
    let a_text = b.uqctl(&["theme", "export", "gruvbox"]);
    std::fs::write(&file, stdout(&a_text)).unwrap();
    assert!(
        b.uqctl(&["theme", "import", file.to_str().unwrap()])
            .status
            .success()
    );
    let (folder, points) = (b.folder(), b.restore_points().await);
    assert_eq!(folder.len(), 2);

    let corpus = bad_corpus();
    assert!(corpus.len() >= 20, "{}", corpus.len());
    for path in corpus {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let o = b.uqctl(&["theme", "import", path.to_str().unwrap()]);
        assert!(!o.status.success(), "{name} was accepted");
        let err = stderr(&o);
        assert_eq!(err.lines().count(), 1, "{name}: {err:?}");
        assert!(err.starts_with("uqctl: "), "{name}: {err:?}");
        assert!(
            !err.chars().any(|c| c.is_control() && c != '\n'),
            "{name}: {err:?}"
        );
        assert_eq!(b.folder(), folder, "{name} changed the themes folder");
        assert_eq!(
            b.restore_points().await,
            points,
            "{name} saved a restore point"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_and_conflict_through_the_cli() {
    let a = home().await;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("g.uqtheme");
    let o = a.uqctl(&["theme", "export", "graphite", file.to_str().unwrap()]);
    assert!(o.status.success());
    assert!(
        a.uqctl(&["theme", "import", file.to_str().unwrap()])
            .status
            .success()
    );
    let o = a.uqctl(&["theme", "import", file.to_str().unwrap()]);
    assert!(!o.status.success());
    assert!(stderr(&o).contains("--replace"), "{}", stderr(&o));
    assert!(
        a.uqctl(&["theme", "import", file.to_str().unwrap(), "--replace"])
            .status
            .success()
    );
    assert!(
        a.uqctl(&["theme", "set", "user-graphite", "dark"])
            .status
            .success()
    );
    let o = a.uqctl(&["theme", "delete", "user-graphite"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(
        stdout(&o).contains("Switched to the default theme first."),
        "{}",
        stdout(&o)
    );
    assert!(a.folder().is_empty());
    let o = a.uqctl(&["theme", "delete", "graphite"]);
    assert!(!o.status.success());
}
