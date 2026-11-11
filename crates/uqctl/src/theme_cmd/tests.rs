use super::*;
use crate::{Cli, Cmd};
use clap::Parser;
use serde_json::json;

fn theme(args: &[&str]) -> ThemeCmd {
    let mut v = vec!["uqctl", "theme"];
    v.extend_from_slice(args);
    match Cli::try_parse_from(v).unwrap().cmd {
        Cmd::Theme { cmd } => cmd,
        _ => panic!("not a theme command"),
    }
}

#[test]
fn cli_parses_theme_commands() {
    assert_eq!(theme(&["get"]), ThemeCmd::Get);
    assert_eq!(theme(&["list"]), ThemeCmd::List { long: false });
    assert_eq!(theme(&["list", "--long"]), ThemeCmd::List { long: true });
    assert_eq!(
        theme(&["set", "rosewood", "light"]),
        ThemeCmd::Set {
            id: "rosewood".into(),
            variant: "light".into()
        }
    );
    assert_eq!(
        theme(&["import", "o.uqtheme"]),
        ThemeCmd::Import {
            file: "o.uqtheme".into(),
            replace: false,
            with_look: false
        }
    );
    assert_eq!(
        theme(&["import", "o.uqtheme", "--replace", "--with-look"]),
        ThemeCmd::Import {
            file: "o.uqtheme".into(),
            replace: true,
            with_look: true
        }
    );
    assert_eq!(
        theme(&["export", "user-ocean"]),
        ThemeCmd::Export {
            id: "user-ocean".into(),
            file: None,
            with_look: false
        }
    );
    assert_eq!(
        theme(&["export", "user-ocean", "o.uqtheme", "--with-look"]),
        ThemeCmd::Export {
            id: "user-ocean".into(),
            file: Some("o.uqtheme".into()),
            with_look: true
        }
    );
    assert_eq!(
        theme(&["delete", "user-ocean"]),
        ThemeCmd::Delete {
            id: "user-ocean".into()
        }
    );
    assert!(Cli::try_parse_from(["uqctl", "theme"]).is_err());
}

fn info(rows: &[(&str, &str, bool, bool)]) -> String {
    json!({
        "themes": rows.iter().map(|(id, name, user, active)| json!({
            "id": id, "name": name, "author": null, "user": user, "missing": false,
            "active": active,
        })).collect::<Vec<_>>(),
        "skipped": [],
        "root_is_link": false,
    })
    .to_string()
}

#[test]
fn info_lines_short_and_long() {
    let json = info(&[
        ("gruvbox", "Gruvbox", false, false),
        ("user-ocean", "Ocean", true, true),
    ]);
    assert_eq!(info_lines(&json, false).unwrap(), ["gruvbox", "user-ocean"]);
    assert_eq!(
        info_lines(&json, true).unwrap(),
        ["  gruvbox  Gruvbox", "* user-ocean  Ocean  (yours)"]
    );
    assert!(info_lines("nope", true).is_err());
}

#[test]
fn info_lines_strip_control_chars() {
    let json = info(&[("user-x", "Evil\x1b[2J", true, false)]);
    let lines = info_lines(&json, true).unwrap();
    assert!(lines.iter().all(|l| !l.contains('\x1b')), "{lines:?}");
    assert!(lines[0].contains("Evil[2J"));
}

#[test]
fn delete_lines_say_what_happened() {
    let j = json!({"switched": true, "profiles": ["Gam\x1bing"]}).to_string();
    assert_eq!(
        delete_lines(&j).unwrap(),
        [
            "Switched to the default theme first.",
            "The Gaming profile uses this theme. Pick another theme for it."
        ]
    );
    assert!(
        delete_lines(&json!({"switched": false, "profiles": []}).to_string())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn import_reads_capped_file() {
    let dir = tempfile::tempdir().unwrap();
    let big = dir.path().join("big.uqtheme");
    std::fs::write(&big, vec![b' '; 64 * 1024 + 1]).unwrap();
    let err = read_import(&big).unwrap_err();
    assert!(err.contains("too big"), "{err}");
    assert!(read_import(&dir.path().join("none")).is_err());
    let ok = dir.path().join("ok.uqtheme");
    std::fs::write(&ok, "format = 1\n").unwrap();
    assert_eq!(read_import(&ok).unwrap(), "format = 1\n");
}

#[test]
fn errors_never_name_the_service() {
    for msg in [
        describe(zbus::Error::Failure("boom".into())),
        crate::profile_cmd::NOT_RUNNING.to_string(),
    ] {
        let lower = msg.to_lowercase();
        assert!(
            !lower.contains("uqd") && !lower.contains("d-bus") && !lower.contains("dbus"),
            "{msg}"
        );
    }
}

#[tokio::test]
async fn run_imports_exports_deletes() {
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
        zbus::connection::Builder::address(bus.address.as_str()).unwrap(),
        desktop,
    )
    .await
    .unwrap();
    let client = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    let proxy = zbus::Proxy::new(
        &client,
        "org.uq.Desktop1",
        "/org/uq/Desktop1",
        "org.uq.Desktop1",
    )
    .await
    .unwrap();

    // Export a system theme, rename it, import it.
    let sys = dir.path().join("gruvbox.uqtheme");
    run(
        &proxy,
        ThemeCmd::Export {
            id: "gruvbox".into(),
            file: Some(sys.clone()),
            with_look: false,
        },
    )
    .await
    .unwrap();
    let text = std::fs::read_to_string(&sys)
        .unwrap()
        .replacen("id = \"gruvbox\"", "id = \"ocean\"", 1)
        .replacen("name = \"Gruvbox\"", "name = \"Ocean\"", 1);
    let file = dir.path().join("ocean.uqtheme");
    std::fs::write(&file, text).unwrap();
    let import = |replace| ThemeCmd::Import {
        file: file.clone(),
        replace,
        with_look: false,
    };
    run(&proxy, import(false)).await.unwrap();
    let err = run(&proxy, import(false)).await.unwrap_err();
    assert!(err.ends_with("(run again with --replace)"), "{err}");
    run(&proxy, import(true)).await.unwrap();

    let json: String = proxy.call("ListThemeInfo", &()).await.unwrap();
    assert!(
        info_lines(&json, true)
            .unwrap()
            .contains(&"  user-ocean  Ocean  (yours)".to_string())
    );
    let out = dir.path().join("out/ocean.uqtheme");
    run(
        &proxy,
        ThemeCmd::Export {
            id: "user-ocean".into(),
            file: Some(out.clone()),
            with_look: false,
        },
    )
    .await
    .unwrap();
    assert!(
        std::fs::read_to_string(&out)
            .unwrap()
            .contains("name = \"Ocean\"")
    );

    run(
        &proxy,
        ThemeCmd::Set {
            id: "user-ocean".into(),
            variant: "light".into(),
        },
    )
    .await
    .unwrap();
    run(&proxy, ThemeCmd::List { long: true }).await.unwrap();
    run(
        &proxy,
        ThemeCmd::Delete {
            id: "user-ocean".into(),
        },
    )
    .await
    .unwrap();
    let err = run(
        &proxy,
        ThemeCmd::Delete {
            id: "user-ocean".into(),
        },
    )
    .await
    .unwrap_err();
    assert!(!err.to_lowercase().contains("uqd"), "{err}");
}
