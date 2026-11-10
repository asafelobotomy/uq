//! `uqctl profile`: list, switch, export and import settings profiles.
//!
//! Every message here is for a person, so none names the background service (K6).

use clap::Subcommand;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use uq_core::profile::{self, Kept, next_after};

const NAME: &str = "org.uq.Desktop1";
const PATH: &str = "/org/uq/Desktop1";
const CONFLICT: &str = "org.uq.Desktop1.Error.Conflict";

#[derive(Subcommand, Debug, PartialEq)]
pub enum ProfileCmd {
    /// List your profiles; the one that's on is marked with `*`.
    List,
    /// Turn a profile on.
    Apply { id: String },
    /// Go back to normal: turn the profile off.
    Off,
    /// Turn on the next profile; after the last one, go back to normal.
    Next,
    /// Write a profile's file to FILE, or to the screen without one.
    Export { id: String, file: Option<PathBuf> },
    /// Add a profile from a file.
    Import {
        file: PathBuf,
        /// Replace a profile that has the same id.
        #[arg(long)]
        replace: bool,
    },
    /// Delete a profile that isn't on.
    Delete { id: String },
}

#[derive(Deserialize)]
struct Row {
    id: String,
    name: String,
    keys: usize,
    #[serde(default)]
    active: bool,
}

#[derive(Deserialize)]
struct Report {
    active: String,
    #[serde(default)]
    kept: Vec<Kept>,
    #[serde(default)]
    notices: Vec<String>,
}

const NOT_RUNNING: &str = "Can't reach uQ's settings service. Is it running?";

fn rows(json: &str) -> Result<Vec<Row>, String> {
    serde_json::from_str(json).map_err(|_| "Got an answer uqctl doesn't understand.".to_string())
}

/// What a failed call says, in plain words.
fn describe(err: zbus::Error) -> String {
    match err {
        zbus::Error::MethodError(_, Some(msg), _) => msg,
        e if uq_core::client::no_owner(&e) => NOT_RUNNING.into(),
        zbus::Error::MethodError(name, None, _) => name.to_string(),
        e => format!("{NOT_RUNNING} ({e})"),
    }
}

/// The lines `profile list` prints.
pub fn profile_lines(json: &str) -> Result<Vec<String>, String> {
    let rows = rows(json)?;
    if rows.is_empty() {
        return Ok(vec![
            "No profiles yet. Make one on the Profiles page of uQ Settings.".into(),
        ]);
    }
    Ok(rows
        .iter()
        .map(|r| {
            let mark = if r.active { '*' } else { ' ' };
            let noun = if r.keys == 1 { "setting" } else { "settings" };
            format!("{mark} {}  {}  ({} {noun})", r.id, r.name, r.keys)
        })
        .collect())
}

/// The lines `profile apply`, `off` and `next` print.
pub fn report_lines(json: &str) -> Result<Vec<String>, String> {
    let r: Report = serde_json::from_str(json)
        .map_err(|_| "Got an answer uqctl doesn't understand.".to_string())?;
    let mut lines = vec![if r.active.is_empty() {
        "Back to normal".to_string()
    } else {
        format!("Profile on: {}", r.active)
    }];
    lines.extend(
        r.kept
            .iter()
            .map(|k| format!("Kept your own value: {}", k.label)),
    );
    lines.extend(r.notices);
    Ok(lines)
}

/// The profile `next` turns on; `None` means "back to normal" (or there are none).
pub fn next_target(list_json: &str) -> Result<Option<String>, String> {
    let rows = rows(list_json)?;
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let active = rows.iter().find(|r| r.active).map(|r| r.id.as_str());
    Ok(next_after(&ids, active))
}

/// The text of a file to import, refused when it is big or isn't text.
pub fn read_import(path: &Path) -> Result<String, String> {
    profile::read_file(path).map_err(|e| e.to_string())
}

/// Connect to the settings service, with a plain message when it isn't there.
pub async fn connect() -> Result<zbus::Proxy<'static>, String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|_| NOT_RUNNING.to_string())?;
    zbus::Proxy::new(&conn, NAME, PATH, NAME)
        .await
        .map_err(describe)
}

fn print_lines(lines: Vec<String>) {
    lines.iter().for_each(|l| println!("{l}"));
}

async fn list_json(proxy: &zbus::Proxy<'_>) -> Result<String, String> {
    proxy.call("ListProfiles", &()).await.map_err(describe)
}

pub async fn run(proxy: &zbus::Proxy<'_>, cmd: ProfileCmd) -> Result<(), String> {
    match cmd {
        ProfileCmd::List => print_lines(profile_lines(&list_json(proxy).await?)?),
        ProfileCmd::Apply { id } => apply(proxy, &id).await?,
        ProfileCmd::Off => off(proxy).await?,
        ProfileCmd::Next => {
            let json = list_json(proxy).await?;
            if rows(&json)?.is_empty() {
                return Err(profile_lines(&json)?.remove(0));
            }
            match next_target(&json)? {
                Some(id) => apply(proxy, &id).await?,
                None => off(proxy).await?,
            }
        }
        ProfileCmd::Export { id, file } => {
            let text: String = proxy.call("GetProfile", &(id,)).await.map_err(describe)?;
            match file {
                Some(path) => uq_core::atomic::atomic_write(&path, &text)
                    .map_err(|e| format!("Can't write {}: {e}", path.display()))?,
                None => print!("{text}"),
            }
        }
        ProfileCmd::Import { file, replace } => {
            let text = read_import(&file)?;
            let id: String = proxy
                .call("SetProfile", &(text, replace))
                .await
                .map_err(|e| {
                    let conflict = matches!(&e, zbus::Error::MethodError(n, _, _)
                        if n.as_str() == CONFLICT);
                    let msg = describe(e);
                    if conflict {
                        format!("{msg} (run again with --replace)")
                    } else {
                        msg
                    }
                })?;
            println!("Added profile: {id}");
        }
        ProfileCmd::Delete { id } => proxy
            .call::<_, _, ()>("DeleteProfile", &(id,))
            .await
            .map_err(describe)?,
    }
    Ok(())
}

async fn apply(proxy: &zbus::Proxy<'_>, id: &str) -> Result<(), String> {
    let json: String = proxy.call("ApplyProfile", &(id,)).await.map_err(describe)?;
    print_lines(report_lines(&json)?);
    Ok(())
}

async fn off(proxy: &zbus::Proxy<'_>) -> Result<(), String> {
    let json: String = proxy.call("ProfileOff", &()).await.map_err(describe)?;
    print_lines(report_lines(&json)?);
    Ok(())
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn errors_never_name_the_service() {
        let gone = zbus::Error::MethodError(
            "org.freedesktop.DBus.Error.ServiceUnknown"
                .try_into()
                .unwrap(),
            None,
            zbus::message::Message::method_call("/", "X")
                .unwrap()
                .build(&())
                .unwrap(),
        );
        assert!(!describe(gone).to_lowercase().contains("uqd"));
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
}
