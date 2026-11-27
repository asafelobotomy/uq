use super::*;
use crate::{Cli, Cmd};
use clap::Parser;
use serde_json::json;

fn parse(args: &[&str]) -> Cmd {
    let mut v = vec!["uqctl", "profile"];
    v.extend_from_slice(args);
    Cli::try_parse_from(v).unwrap().cmd
}

fn profile(args: &[&str]) -> ProfileCmd {
    match parse(args) {
        Cmd::Profile { cmd } => cmd,
        _ => panic!("not a profile command"),
    }
}

#[test]
fn cli_parses_profile_commands() {
    assert_eq!(profile(&["list"]), ProfileCmd::List);
    assert_eq!(
        profile(&["apply", "gaming"]),
        ProfileCmd::Apply {
            id: "gaming".into()
        }
    );
    assert_eq!(profile(&["off"]), ProfileCmd::Off);
    assert_eq!(profile(&["next"]), ProfileCmd::Next);
    assert_eq!(
        profile(&["export", "work"]),
        ProfileCmd::Export {
            id: "work".into(),
            file: None
        }
    );
    assert_eq!(
        profile(&["export", "work", "/tmp/w.toml"]),
        ProfileCmd::Export {
            id: "work".into(),
            file: Some("/tmp/w.toml".into())
        }
    );
    assert_eq!(
        profile(&["import", "w.toml"]),
        ProfileCmd::Import {
            file: "w.toml".into(),
            replace: false
        }
    );
    assert_eq!(
        profile(&["import", "w.toml", "--replace"]),
        ProfileCmd::Import {
            file: "w.toml".into(),
            replace: true
        }
    );
    assert_eq!(
        profile(&["delete", "work"]),
        ProfileCmd::Delete { id: "work".into() }
    );
    assert!(Cli::try_parse_from(["uqctl", "profile"]).is_err());
}

fn list(rows: &[(&str, &str, usize, bool)]) -> String {
    json!(
        rows.iter()
            .map(|(id, name, keys, active)| json!({
                "id": id, "name": name, "icon": null, "keys": keys, "power": null,
                "active": active,
            }))
            .collect::<Vec<_>>()
    )
    .to_string()
}

#[test]
fn profile_lines_format() {
    let json = list(&[("gaming", "Gaming", 6, true), ("work", "Work", 1, false)]);
    assert_eq!(
        profile_lines(&json).unwrap(),
        [
            "* gaming  Gaming  (6 settings)",
            "  work  Work  (1 setting)"
        ]
    );
    assert_eq!(
        profile_lines("[]").unwrap(),
        ["No profiles yet. Make one on the Profiles page of uQ Settings."]
    );
    assert!(profile_lines("nope").is_err());
}

#[test]
fn report_lines_format() {
    let on = json!({
        "active": "gaming",
        "kept": [],
        "notices": ["Power mode wasn't changed: it isn't available."],
    })
    .to_string();
    assert_eq!(
        report_lines(&on).unwrap(),
        [
            "Profile on: gaming",
            "Power mode wasn't changed: it isn't available."
        ]
    );
    let off = json!({
        "active": "",
        "kept": [
            {"domain": "look", "path": ["gaps_in"], "label": "Gaps between windows"},
            {"domain": "power", "path": ["profile"], "label": "Power mode"},
        ],
        "notices": [],
    })
    .to_string();
    assert_eq!(
        report_lines(&off).unwrap(),
        [
            "Back to normal",
            "Kept your own value: Gaps between windows",
            "Kept your own value: Power mode"
        ]
    );
}

#[test]
fn next_target_cycles() {
    let none_on = list(&[("a", "A", 1, false), ("b", "B", 1, false)]);
    let a_on = list(&[("a", "A", 1, true), ("b", "B", 1, false)]);
    let b_on = list(&[("a", "A", 1, false), ("b", "B", 1, true)]);
    assert_eq!(next_target(&none_on).unwrap(), Some("a".into()));
    assert_eq!(next_target(&a_on).unwrap(), Some("b".into()));
    assert_eq!(next_target(&b_on).unwrap(), None);
    assert_eq!(next_target("[]").unwrap(), None);
}

#[test]
fn read_import_refuses_big_and_binary_files() {
    let dir = tempfile::tempdir().unwrap();
    let ok = dir.path().join("ok.toml");
    std::fs::write(&ok, "format = 1\n").unwrap();
    assert_eq!(read_import(&ok).unwrap(), "format = 1\n");

    let big = dir.path().join("big.toml");
    std::fs::write(&big, vec![b'#'; 1 << 20]).unwrap();
    assert_eq!(
        read_import(&big).unwrap_err(),
        "The file is too big to be a profile."
    );

    let bin = dir.path().join("bin.toml");
    std::fs::write(&bin, [0xff, 0xfe, 0x00, 0x80]).unwrap();
    assert_eq!(
        read_import(&bin).unwrap_err(),
        "This file isn't a uQ profile."
    );

    let err = read_import(&dir.path().join("missing.toml")).unwrap_err();
    assert!(err.starts_with("Can't read the file"), "{err}");
}

fn method_error(name: &str, msg: Option<&str>) -> zbus::Error {
    zbus::Error::MethodError(
        name.try_into().unwrap(),
        msg.map(str::to_string),
        zbus::message::Message::method_call("/", "X")
            .build(&())
            .unwrap(),
    )
}

#[test]
fn errors_never_name_the_service() {
    // What a real bus daemon says when uqd isn't running: it always carries a message.
    for name in [
        "org.freedesktop.DBus.Error.ServiceUnknown",
        "org.freedesktop.DBus.Error.NameHasNoOwner",
    ] {
        let gone = method_error(
            name,
            Some("The name org.uq.Desktop1 was not provided by any .service files"),
        );
        assert_eq!(describe(gone), NOT_RUNNING);
    }
    // uqd's own plain message still comes through.
    let own = method_error("org.uq.Desktop1.Error.NotFound", Some("no profile \"x\""));
    assert_eq!(describe(own), "no profile \"x\"");
    // An error with no message never shows its bus name.
    let bare = describe(method_error("org.uq.Desktop1.Error.Failed", None));
    assert!(!bare.contains("org."), "{bare}");
    // Anything else is the plain line, without the library's text.
    assert_eq!(describe(zbus::Error::InvalidReply), NOT_RUNNING);
    assert!(!NOT_RUNNING.to_lowercase().contains("uqd"));
}

const GAMING: &str = "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[look]\ngaps_in = 0\n";

#[tokio::test]
async fn run_imports_exports_and_switches() {
    use std::sync::Arc;
    use uqd::compositor::fake::FakeCompositor;
    use uqd::system::FakeSystem;

    let dir = tempfile::tempdir().unwrap();
    let bus = uq_core::test_bus::private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let desktop = uqd::service::Desktop::new(
        paths,
        Arc::new(FakeCompositor::running()),
        Arc::new(FakeSystem::default()),
    )
    .unwrap();
    let _server = uqd::dbus::serve(
        zbus::connection::Builder::address(bus.address.as_str()),
        desktop,
    )
    .await
    .unwrap();
    let client = zbus::connection::Builder::address(bus.address.as_str())
        .build()
        .await
        .unwrap();
    let proxy = zbus::Proxy::new(&client, NAME, PATH, NAME).await.unwrap();

    let file = dir.path().join("gaming.toml");
    std::fs::write(&file, GAMING).unwrap();
    let import = |replace| ProfileCmd::Import {
        file: file.clone(),
        replace,
    };
    run(&proxy, import(false)).await.unwrap();
    let err = run(&proxy, import(false)).await.unwrap_err();
    assert!(err.ends_with("(run again with --replace)"), "{err}");
    run(&proxy, import(true)).await.unwrap();

    let out = dir.path().join("out/exported.toml");
    let export = ProfileCmd::Export {
        id: "gaming".into(),
        file: Some(out.clone()),
    };
    run(&proxy, export).await.unwrap();
    assert!(
        std::fs::read_to_string(&out)
            .unwrap()
            .contains("gaps_in = 0")
    );

    run(&proxy, ProfileCmd::Next).await.unwrap();
    let json = list_json(&proxy).await.unwrap();
    assert_eq!(next_target(&json).unwrap(), None, "gaming is on, and last");
    run(&proxy, ProfileCmd::Next).await.unwrap();
    assert_eq!(
        next_target(&list_json(&proxy).await.unwrap()).unwrap(),
        Some("gaming".into())
    );

    let err = run(&proxy, ProfileCmd::Apply { id: "nope".into() })
        .await
        .unwrap_err();
    assert!(err.contains("no profile"), "{err}");
}
