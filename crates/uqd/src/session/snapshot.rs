//! What "restore last session" remembers: the apps that have windows open, each with the
//! workspace of its first window. Windows are matched to installed apps by their class, so only
//! apps that can be started again are kept. Everything read from Hyprland is untrusted text.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use uq_core::desktop_entry::{normalise_id, read_entry, valid_desktop_id};

/// The most apps reopened.
pub const MAX_APPS: usize = 30;
/// The largest session file read.
const MAX_FILE: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionApp {
    /// The app's launcher file, `firefox.desktop`.
    pub id: String,
    /// The workspace its first window was on (a normal, numbered one).
    pub workspace: i64,
}

/// One installed app, with what its windows can be known by.
#[derive(Debug, Clone)]
pub struct Known {
    pub id: String,
    /// Lower-case names a window class may have: the id without `.desktop`, its last
    /// reverse-DNS part, `StartupWMClass` and the program's file name.
    pub names: Vec<String>,
}

/// The apps in `dirs` that can be started (Application entries with an `Exec`, not hidden).
pub fn known_apps(dirs: &[PathBuf]) -> Vec<Known> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        for p in files {
            let Some(id) = p.file_name().and_then(|n| n.to_str()).map(normalise_id) else {
                continue;
            };
            if !id.ends_with(".desktop") || !valid_desktop_id(&id) || !seen.insert(id.clone()) {
                continue;
            }
            if let Some(k) = read_known(&p, &id) {
                out.push(k);
            }
        }
    }
    out
}

fn read_known(path: &Path, id: &str) -> Option<Known> {
    if std::fs::metadata(path).ok()?.len() > MAX_FILE {
        return None;
    }
    let e = read_entry(&std::fs::read_to_string(path).ok()?);
    if e.hidden || e.kind.as_deref() != Some("Application") || e.exec.is_none() {
        return None;
    }
    let stem = id.strip_suffix(".desktop").unwrap_or(id).to_lowercase();
    let mut names = vec![stem.clone()];
    if let Some(last) = stem.rsplit('.').next().filter(|l| *l != stem) {
        names.push(last.to_string());
    }
    names.extend(e.wm_class.map(|c| c.to_lowercase()));
    let program = e
        .exec
        .as_deref()
        .and_then(|x| uq_core::desktop_entry::parse_exec(x).ok())
        .and_then(|argv| argv.into_iter().next())
        .map(|p| p.rsplit('/').next().unwrap_or(&p).to_lowercase());
    names.extend(program.filter(|p| p != "env" && p != "flatpak"));
    Some(Known {
        id: id.to_string(),
        names,
    })
}

/// The app a window class belongs to: an exact `StartupWMClass` or id match first, then the
/// other names.
fn app_for<'a>(class: &str, apps: &'a [Known]) -> Option<&'a Known> {
    let c = class.to_lowercase();
    apps.iter()
        .find(|k| k.names.first() == Some(&c))
        .or_else(|| apps.iter().find(|k| k.names.contains(&c)))
}

/// The session from a `j/clients` reply: each app once, at its first window's workspace, in the
/// order Hyprland lists them. Windows on special workspaces (a scratchpad, minimised ones) and
/// unknown classes are left out; `skip` holds ids not to reopen (uQ's own windows).
pub fn from_clients(reply: &str, apps: &[Known], skip: &[&str]) -> Vec<SessionApp> {
    let Ok(Value::Array(list)) = serde_json::from_str::<Value>(reply) else {
        return Vec::new();
    };
    let mut out: Vec<SessionApp> = Vec::new();
    for c in &list {
        let class = c["initialClass"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or(c["class"].as_str())
            .unwrap_or("");
        let ws = c["workspace"]["id"].as_i64().unwrap_or(-1);
        if class.is_empty() || class.len() > 200 || ws <= 0 || c["mapped"] == false {
            continue;
        }
        let Some(app) = app_for(class, apps) else {
            continue;
        };
        if skip.contains(&app.id.as_str()) || out.iter().any(|s| s.id == app.id) {
            continue;
        }
        out.push(SessionApp {
            id: app.id.clone(),
            workspace: ws,
        });
        if out.len() >= MAX_APPS {
            break;
        }
    }
    out
}

pub fn load(path: &Path) -> Vec<SessionApp> {
    if std::fs::metadata(path).map_or(true, |m| m.len() > MAX_FILE) {
        return Vec::new();
    }
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let list: Vec<SessionApp> = serde_json::from_str(&text).unwrap_or_default();
    list.into_iter()
        .filter(|a| valid_desktop_id(&a.id) && (1..=1000).contains(&a.workspace))
        .take(MAX_APPS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(id: &str, wm: Option<&str>, exec: &str) -> Known {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(id);
        let wm = wm
            .map(|w| format!("StartupWMClass={w}\n"))
            .unwrap_or_default();
        std::fs::write(
            &p,
            format!("[Desktop Entry]\nType=Application\nName=x\nExec={exec}\n{wm}"),
        )
        .unwrap();
        read_known(&p, id).unwrap()
    }

    #[test]
    fn windows_become_apps_on_their_workspaces() {
        let apps = vec![
            known("org.kde.dolphin.desktop", None, "dolphin %u"),
            known("firefox.desktop", None, "/usr/lib/firefox/firefox %u"),
            known("code.desktop", Some("Code"), "/usr/bin/code %F"),
            known("io.github.asafelobotomy.uqview.desktop", None, "uqview %U"),
        ];
        let reply = r#"[
            {"class":"firefox","initialClass":"firefox","workspace":{"id":2},"mapped":true},
            {"class":"org.kde.dolphin","initialClass":"org.kde.dolphin","workspace":{"id":1},"mapped":true},
            {"class":"firefox","initialClass":"firefox","workspace":{"id":3},"mapped":true},
            {"class":"Code","initialClass":"Code","workspace":{"id":4},"mapped":true},
            {"class":"kitty","initialClass":"kitty","workspace":{"id":1},"mapped":true},
            {"class":"uqview","initialClass":"uqview","workspace":{"id":-98},"mapped":true},
            {"class":"io.github.asafelobotomy.uqview","workspace":{"id":5},"mapped":true}
        ]"#;
        let s = from_clients(reply, &apps, &["io.github.asafelobotomy.uqview.desktop"]);
        assert_eq!(
            s,
            [
                SessionApp {
                    id: "firefox.desktop".into(),
                    workspace: 2
                },
                SessionApp {
                    id: "org.kde.dolphin.desktop".into(),
                    workspace: 1
                },
                SessionApp {
                    id: "code.desktop".into(),
                    workspace: 4
                },
            ]
        );
        assert!(from_clients("not json", &apps, &[]).is_empty());
    }

    #[test]
    fn a_saved_session_is_checked_when_loaded() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.json");
        std::fs::write(
            &p,
            r#"[{"id":"a.desktop","workspace":2},{"id":"../b","workspace":1},{"id":"c.desktop","workspace":0}]"#,
        )
        .unwrap();
        assert_eq!(
            load(&p),
            [SessionApp {
                id: "a.desktop".into(),
                workspace: 2
            }]
        );
        assert!(load(&d.path().join("none")).is_empty());
    }
}
