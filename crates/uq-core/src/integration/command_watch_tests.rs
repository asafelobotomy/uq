use super::*;
use crate::integration::manifest::{parse_page, parse_widget};

const PAGE: &str = "id = \"p\"\nname = \"P\"\nqml = \"Page.qml\"\n";

fn page_with(command: &str) -> Result<crate::integration::manifest::PageManifest, String> {
    parse_page(&format!(
        "{PAGE}[[command]]\nid = \"status\"\nargv = [\"/usr/bin/x\"]\noutput = \"json\"\n{command}"
    ))
    .map_err(|e| e.to_string())
}

#[test]
fn a_page_command_takes_a_watch() {
    let p = page_with(
        "watch = { files = [\"~/.config/x/config.toml\", \"/etc/x.conf\"], signals = [\
         { bus = \"system\", interface = \"org.x.X1\", member = \"Changed\", path = \"/org/x\", \
         sender = \"org.x.X1\" }, { interface = \"org.x.X1\", member = \"Other\" }] }\n",
    )
    .unwrap();
    let w = p.commands[0].watch.clone().unwrap();
    assert_eq!(w.files, ["~/.config/x/config.toml", "/etc/x.conf"]);
    assert_eq!(w.signals[0].bus, Bus::System);
    assert_eq!(w.signals[0].path.as_deref(), Some("/org/x"));
    assert_eq!(w.signals[1].bus, Bus::Session, "session by default");

    // The table form under the command works too.
    let p = page_with("[command.watch]\nfiles = [\"/etc/x.conf\"]\n").unwrap();
    assert_eq!(p.commands[0].watch.as_ref().unwrap().files, ["/etc/x.conf"]);
    assert!(page_with("").unwrap().commands[0].watch.is_none());
}

#[test]
fn bad_watches_are_refused() {
    let many: Vec<String> = (0..=MAX_WATCH_FILES)
        .map(|i| format!("\"/f{i}\""))
        .collect();
    let many = format!("watch = {{ files = [{}] }}\n", many.join(", "));
    for (why, extra) in [
        ("empty", "watch = {}\n"),
        ("relative", "watch = { files = [\"x.conf\"] }\n"),
        ("dot dot", "watch = { files = [\"/etc/../x\"] }\n"),
        ("dot", "watch = { files = [\"/etc/./x\"] }\n"),
        ("double slash", "watch = { files = [\"/etc//x\"] }\n"),
        ("trailing slash", "watch = { files = [\"/etc/x/\"] }\n"),
        ("home itself", "watch = { files = [\"~/\"] }\n"),
        ("other user", "watch = { files = [\"~root/x\"] }\n"),
        ("root", "watch = { files = [\"/\"] }\n"),
        ("too many", many.as_str()),
        (
            "interface",
            "watch = { signals = [{ interface = \"x\", member = \"Changed\" }] }\n",
        ),
        (
            "member",
            "watch = { signals = [{ interface = \"org.x.X\", member = \"a.b\" }] }\n",
        ),
        (
            "path",
            "watch = { signals = [{ interface = \"org.x.X\", member = \"C\", path = \"x\" }] }\n",
        ),
        (
            "sender",
            "watch = { signals = [{ interface = \"org.x.X\", member = \"C\", sender = \":1.2\" }] }\n",
        ),
        (
            "unknown key",
            "watch = { files = [\"/etc/x\"], every = 5 }\n",
        ),
        (
            "args",
            "args = [{ name = \"a\", pattern = \"^a$\" }]\nwatch = { files = [\"/etc/x\"] }\n",
        ),
        (
            "confirm",
            "confirm = \"Sure?\"\nwatch = { files = [\"/etc/x\"] }\n",
        ),
    ] {
        assert!(page_with(extra).is_err(), "{why}");
    }
    let w = parse_widget(
        "uq_api = 1\nid = \"w\"\nname = \"W\"\nslot = \"bar\"\nqml = \"W.qml\"\n\
         [[command]]\nid = \"s\"\nargv = [\"/usr/bin/x\"]\noutput = \"json\"\n\
         watch = { files = [\"/etc/x\"] }\n",
    );
    assert!(w.unwrap_err().to_string().contains("pages only"));
}

#[test]
fn targets_follow_missing_folders_until_they_appear() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("a/b/config.toml");
    assert_eq!(
        file_targets(std::slice::from_ref(&f)),
        [(d.path().to_path_buf(), "a".into())]
    );
    std::fs::create_dir_all(d.path().join("a/b")).unwrap();
    assert_eq!(
        file_targets(std::slice::from_ref(&f)),
        [(d.path().join("a/b"), "config.toml".into())]
    );
    assert_eq!(
        expand("~/x/y", Path::new("/home/u")),
        Path::new("/home/u/x/y")
    );
    assert_eq!(expand("/etc/x", Path::new("/home/u")), Path::new("/etc/x"));
}

async fn changed(w: &mut Watch, within: Duration) -> bool {
    tokio::time::timeout(within, w.changes.recv()).await.is_ok()
}

#[tokio::test]
async fn file_changes_are_reported_once_quiet() {
    let d = tempfile::tempdir().unwrap();
    let spec = WatchSpec {
        files: vec!["~/conf/x.toml".into()],
        signals: Vec::new(),
    };
    let debounce = Duration::from_millis(150);
    let mut w = start(&spec, d.path(), None, None, debounce).await.unwrap();
    assert!(
        !changed(&mut w, Duration::from_millis(300)).await,
        "quiet at start"
    );

    // The folder doesn't exist yet: it appears, then the file is written.
    std::fs::create_dir(d.path().join("conf")).unwrap();
    assert!(
        changed(&mut w, Duration::from_secs(3)).await,
        "its folder appeared"
    );
    std::fs::write(d.path().join("conf/x.toml"), "a = 1").unwrap();
    assert!(changed(&mut w, Duration::from_secs(3)).await, "written");

    // A burst of writes is one report; a neighbour file is none.
    for i in 0..10 {
        std::fs::write(d.path().join("conf/x.toml"), format!("a = {i}")).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(changed(&mut w, Duration::from_secs(3)).await);
    assert!(
        !changed(&mut w, Duration::from_millis(400)).await,
        "one per burst"
    );
    std::fs::write(d.path().join("conf/other"), "x").unwrap();
    assert!(
        !changed(&mut w, Duration::from_millis(400)).await,
        "another file"
    );

    // An editor's save: write a temporary file and rename it over.
    std::fs::write(d.path().join("conf/.x.tmp"), "a = 9").unwrap();
    std::fs::rename(d.path().join("conf/.x.tmp"), d.path().join("conf/x.toml")).unwrap();
    assert!(changed(&mut w, Duration::from_secs(3)).await, "replaced");
}

#[tokio::test]
async fn a_signal_bus_that_is_not_there_is_an_error() {
    let spec = WatchSpec {
        files: Vec::new(),
        signals: vec![SignalMatch {
            bus: Bus::System,
            interface: "org.x.X".into(),
            member: "C".into(),
            path: None,
            sender: None,
        }],
    };
    let r = start(&spec, Path::new("/"), None, None, WATCH_DEBOUNCE).await;
    assert!(r.is_err());
}
