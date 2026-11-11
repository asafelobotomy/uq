//! `uqctl keys`: list uQ's shortcuts and put them back from a terminal, a way back when the
//! desktop is broken (R14). It resets overrides only: your own shortcuts (`custom`) pass through
//! untouched, and it never adds or edits one.
//!
//! Every message here is for a person, so none names the background service (K6).

use crate::profile_cmd::describe;
use clap::Subcommand;
use serde_json::Value;
use uq_core::keybinds::defaults::default_of;
use uq_core::settings::clean_text_spaced;

/// The origin uqd's history records for uqctl's writes.
const ORIGIN: &str = "uqctl";
const UNDERSTAND: &str = "Got an answer uqctl doesn't understand.";
const MAX_TEXT: usize = 200;

#[derive(Subcommand, Debug, PartialEq)]
pub enum KeysCmd {
    /// List uQ's shortcuts and your own: id, keys and what each does.
    List,
    /// Put one shortcut (or every shortcut) of uQ's back to its default keys and turn it on.
    /// Your own shortcuts are kept.
    Reset {
        /// A shortcut id from `uqctl keys list`, such as uq.spotlight.
        #[arg(required_unless_present = "all", conflicts_with = "all")]
        id: Option<String>,
        /// Put every one of uQ's shortcuts back.
        #[arg(long)]
        all: bool,
    },
}

#[derive(Debug, PartialEq)]
pub enum ResetTarget {
    One(String),
    All,
}

/// The `keybinds` section with the chosen override(s) dropped. Only `overrides` changes; every
/// other key, `custom` included, is passed through as it came. An override that isn't there
/// leaves the section equal to what came in; an id that isn't one of uQ's shortcuts is an error.
pub fn reset_section(section: &Value, target: ResetTarget) -> Result<Value, String> {
    let mut out = section.clone();
    let Some(obj) = out.as_object_mut() else {
        return Err(UNDERSTAND.into());
    };
    match target {
        ResetTarget::All => {
            if obj.contains_key("overrides") {
                obj.insert("overrides".into(), Value::Array(Vec::new()));
            }
        }
        ResetTarget::One(id) => {
            if default_of(&id).is_none() {
                return Err(format!(
                    "{} isn't one of uQ's shortcuts. `uqctl keys list` shows them.",
                    clean_text_spaced(&id, 60)
                ));
            }
            if let Some(Value::Array(list)) = obj.get_mut("overrides") {
                list.retain(|o| o.get("id").and_then(Value::as_str) != Some(id.as_str()));
            }
        }
    }
    Ok(out)
}

fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// The lines `keys list` prints: `<id>\t<keys or ->\t<label>` for every shortcut, then
/// `user.lua:<line>\t<keys or ->\t<text>` for the binds marked `-- uq:bind`. Text from outside
/// has its control characters turned into spaces, so a row can't break the columns or the
/// terminal. A shortcut that is off says so after its label.
pub fn list_lines(json: &str) -> Result<Vec<String>, String> {
    let v: Value = serde_json::from_str(json).map_err(|_| UNDERSTAND.to_string())?;
    let clean = |s: &str| clean_text_spaced(s, MAX_TEXT);
    let combo = |s: &str| if s.is_empty() { "-".into() } else { clean(s) };
    let mut out = Vec::new();
    for b in v["binds"].as_array().ok_or(UNDERSTAND)? {
        let off = b.get("enabled").and_then(Value::as_bool) == Some(false);
        out.push(format!(
            "{}\t{}\t{}{}",
            clean(text(b, "id")),
            combo(text(b, "combo")),
            clean(text(b, "label")),
            if off { " (off)" } else { "" }
        ));
    }
    for u in v["user_lua"].as_array().map_or(&[][..], Vec::as_slice) {
        let line = u.get("line").and_then(Value::as_u64).unwrap_or(0);
        out.push(format!(
            "user.lua:{line}\t{}\t{}",
            combo(text(u, "combo")),
            clean(text(u, "text"))
        ));
    }
    Ok(out)
}

/// Runs `cmd` against uqd; returns the lines to print.
pub async fn run(proxy: &zbus::Proxy<'_>, cmd: KeysCmd) -> Result<Vec<String>, String> {
    match cmd {
        KeysCmd::List => {
            let json: String = proxy.call("ListBinds", &()).await.map_err(describe)?;
            list_lines(&json)
        }
        KeysCmd::Reset { id, all } => {
            let (target, what) = match (id, all) {
                (_, true) => (ResetTarget::All, "uQ's shortcuts".to_string()),
                (Some(id), false) => {
                    let what = id.clone();
                    (ResetTarget::One(id), what)
                }
                (None, false) => return Err("Say which shortcut, or use --all.".into()),
            };
            let got: String = proxy
                .call("GetSection", &("keybinds",))
                .await
                .map_err(describe)?;
            let section: Value = serde_json::from_str(&got).map_err(|_| UNDERSTAND.to_string())?;
            let next = reset_section(&section, target)?;
            if next == section {
                let line = if all {
                    "uQ's shortcuts already use their defaults.".to_string()
                } else {
                    format!(
                        "{} already uses uQ's default.",
                        clean_text_spaced(&what, 60)
                    )
                };
                return Ok(vec![line]);
            }
            proxy
                .call::<_, _, ()>("SetSection", &("keybinds", next.to_string(), ORIGIN))
                .await
                .map_err(describe)?;
            Ok(vec![if all {
                "uQ's shortcuts are back to how they came.".to_string()
            } else {
                format!("{} is back to its default.", clean_text_spaced(&what, 60))
            }])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cli, Cmd};
    use clap::Parser;
    use serde_json::json;

    fn keys(args: &[&str]) -> Result<KeysCmd, clap::Error> {
        let mut v = vec!["uqctl", "keys"];
        v.extend_from_slice(args);
        Cli::try_parse_from(v).map(|c| match c.cmd {
            Cmd::Keys { cmd } => cmd,
            _ => panic!("not a keys command"),
        })
    }

    #[test]
    fn parses() {
        assert_eq!(keys(&["list"]).unwrap(), KeysCmd::List);
        assert_eq!(
            keys(&["reset", "uq.close"]).unwrap(),
            KeysCmd::Reset {
                id: Some("uq.close".into()),
                all: false
            }
        );
        assert_eq!(
            keys(&["reset", "--all"]).unwrap(),
            KeysCmd::Reset {
                id: None,
                all: true
            }
        );
        // Neither, or both, is a usage error.
        assert!(keys(&["reset"]).is_err());
        assert!(keys(&["reset", "uq.close", "--all"]).is_err());
        assert!(keys(&[]).is_err());
    }

    fn section() -> Value {
        json!({
            "overrides": [{"id": "uq.spotlight", "combo": "SUPER + K"},
                          {"id": "uq.close", "enabled": false}],
            "custom": [{"id": "k-0001", "name": "Shot", "combo": "SUPER + F1", "enabled": true,
                        "locked": false, "repeating": false,
                        "action": {"kind": "command", "command": "grim x.png"}}],
        })
    }

    #[test]
    fn reset_one_drops_only_that_override() {
        let s = section();
        let out = reset_section(&s, ResetTarget::One("uq.spotlight".into())).unwrap();
        assert_eq!(
            out["overrides"],
            json!([{"id": "uq.close", "enabled": false}])
        );
        assert_eq!(out["custom"], s["custom"]);
        // No override: nothing changes, and the caller says so.
        let out = reset_section(&s, ResetTarget::One("uq.float".into())).unwrap();
        assert_eq!(out, s);
        // An id that isn't a shortcut is refused plainly, as is one of your own.
        let e = reset_section(&s, ResetTarget::One("uq.nope".into())).unwrap_err();
        assert!(e.contains("isn't one of uQ's shortcuts"), "{e}");
        assert!(reset_section(&s, ResetTarget::One("k-0001".into())).is_err());
    }

    #[test]
    fn reset_all_keeps_custom() {
        let s = section();
        let out = reset_section(&s, ResetTarget::All).unwrap();
        assert_eq!(out["overrides"], json!([]));
        assert_eq!(out["custom"], s["custom"]);
        assert!(reset_section(&json!([]), ResetTarget::All).is_err());
    }

    #[test]
    fn list_prints_plain_rows() {
        let json = json!({
            "binds": [
                {"id": "uq.spotlight", "combo": "SUPER + K", "label": "Open Spotlight",
                 "enabled": true},
                {"id": "uq.profile.next", "combo": null, "label": "Next profile",
                 "enabled": true},
                {"id": "k-0001", "combo": "SUPER + F1", "enabled": false,
                 "label": "Bad\u{1b}[31m name\tx\nz"},
            ],
            "user_lua": [
                {"line": 12, "combo": "SUPER + T", "text": "Terminal\u{7}two"},
                {"line": 14, "combo": null, "text": "built key"},
            ],
        })
        .to_string();
        let lines = list_lines(&json).unwrap();
        assert_eq!(
            lines,
            [
                "uq.spotlight\tSUPER + K\tOpen Spotlight",
                "uq.profile.next\t-\tNext profile",
                "k-0001\tSUPER + F1\tBad [31m name x z (off)",
                "user.lua:12\tSUPER + T\tTerminal two",
                "user.lua:14\t-\tbuilt key",
            ]
        );
        for l in &lines {
            assert!(!l.chars().any(|c| c.is_control() && c != '\t'), "{l:?}");
            assert_eq!(l.matches('\t').count(), 2, "{l:?}");
        }
        assert!(list_lines("nope").is_err());
        assert!(list_lines("{}").is_err());
    }

    #[tokio::test]
    async fn reset_journals_with_origin_uqctl() {
        use std::path::PathBuf;
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

        // Two overrides and a shortcut of your own, written the way the page writes them.
        let start = r#"{"overrides":[{"id":"uq.spotlight","combo":"SUPER + K"},
            {"id":"uq.close","enabled":false}],
            "custom":[{"name":"Shot","combo":"SUPER + F1",
              "action":{"kind":"command","command":"grim x.png"}}]}"#;
        proxy
            .call::<_, _, ()>("SetSection", &("keybinds", start, "settings"))
            .await
            .unwrap();

        let lines = run(&proxy, KeysCmd::List).await.unwrap();
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("uq.spotlight\tSUPER + K\t")),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("uq.close\t") && l.ends_with("(off)")),
            "{lines:?}"
        );

        let one = KeysCmd::Reset {
            id: Some("uq.spotlight".into()),
            all: false,
        };
        assert_eq!(
            run(&proxy, one).await.unwrap(),
            ["uq.spotlight is back to its default."]
        );
        let again = KeysCmd::Reset {
            id: Some("uq.spotlight".into()),
            all: false,
        };
        assert_eq!(
            run(&proxy, again).await.unwrap(),
            ["uq.spotlight already uses uQ's default."]
        );
        let bad = KeysCmd::Reset {
            id: Some("uq.nope".into()),
            all: false,
        };
        assert!(run(&proxy, bad).await.unwrap_err().contains("isn't one of"));

        let history: String = proxy.call("ListHistory", &(10u32,)).await.unwrap();
        let history: Vec<Value> = serde_json::from_str(&history).unwrap();
        assert_eq!(history[0]["domain"], "keybinds");
        assert_eq!(history[0]["origin"], "uqctl");
        assert_eq!(
            history[0]["summary"],
            "“Open Spotlight” back to its default"
        );

        let all = KeysCmd::Reset {
            id: None,
            all: true,
        };
        assert_eq!(
            run(&proxy, all).await.unwrap(),
            ["uQ's shortcuts are back to how they came."]
        );
        let got: String = proxy.call("GetSection", &("keybinds",)).await.unwrap();
        let got: Value = serde_json::from_str(&got).unwrap();
        assert_eq!(got["overrides"], json!([]));
        assert_eq!(got["custom"][0]["name"], "Shot", "your own shortcut stays");
        let all = KeysCmd::Reset {
            id: None,
            all: true,
        };
        assert_eq!(
            run(&proxy, all).await.unwrap(),
            ["uQ's shortcuts already use their defaults."]
        );
    }
}
