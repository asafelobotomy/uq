//! `StaleApps`: which of the shell's desktop entries the app menu and look=see>do_ leave out,
//! because launching them can't work or they repeat another entry. An entry is stale when:
//!
//! - its `TryExec` isn't installed, or the program its `Exec` runs is gone (an AppImage that was
//!   deleted, a package that was removed but left a copied launcher in `~/.local/share`), or
//! - its file is a symlink to the file of another listed entry (a package that ships
//!   `app.desktop` as an alias of `org.example.App.desktop`): the alias goes, the real one stays.
//!
//! An id that can't be found or read is never stale: the menu shows what it can't check. Nothing
//! here launches or changes anything.

use super::desktop_entry::{Dirs, binary_exists, parse_exec, read_entry};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use uq_core::settings::valid_pinned_id;

/// The most ids one call checks; a menu has a few hundred.
const MAX_IDS: usize = 4096;

pub fn stale_apps(args: &[Value]) -> Result<Value, String> {
    let [Value::Array(ids)] = args else {
        return Err("StaleApps takes one list of desktop ids".into());
    };
    if ids.len() > MAX_IDS {
        return Err(format!("StaleApps takes at most {MAX_IDS} ids"));
    }
    let ids: Vec<&str> = ids.iter().filter_map(Value::as_str).collect();
    Ok(Value::from(stale(&Dirs::from_env(), &ids, &binary_exists)))
}

/// The program an `Exec` line starts: its first word, after `env` and its `NAME=value` words.
fn program(exec: &str) -> Option<String> {
    let argv = parse_exec(exec).ok()?;
    let mut words = argv.into_iter().peekable();
    if words
        .peek()
        .is_some_and(|w| w == "env" || w == "/usr/bin/env")
    {
        words.next();
        while let Some(w) =
            words.next_if(|w| w.starts_with('-') || (w.contains('=') && !w.starts_with('=')))
        {
            // `-u NAME` and `-C DIR` take the next word too.
            if w == "-u" || w == "--unset" || w == "-C" || w == "--chdir" {
                words.next();
            }
        }
    }
    words.next().filter(|w| !w.is_empty())
}

pub(super) fn stale(dirs: &Dirs, ids: &[&str], exists: &dyn Fn(&str) -> bool) -> Vec<String> {
    struct Found<'a> {
        id: &'a str,
        link: bool,
        real: Option<PathBuf>,
    }
    let mut out = vec![];
    let mut found = vec![];
    for &id in ids {
        if !valid_pinned_id(id) {
            continue;
        }
        let Some(path) = dirs.find(id) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let e = read_entry(&text);
        let gone = e.try_exec.as_deref().is_some_and(|t| !exists(t))
            || e.exec
                .as_deref()
                .and_then(program)
                .is_some_and(|p| !exists(&p));
        if gone {
            out.push(id.to_string());
            continue;
        }
        let link = path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink());
        found.push(Found {
            id,
            link,
            real: std::fs::canonicalize(&path).ok(),
        });
    }
    // Real files by where they are; a symlink to one of them is that entry again.
    let real: HashMap<&PathBuf, &str> = found
        .iter()
        .filter(|f| !f.link)
        .filter_map(|f| f.real.as_ref().map(|r| (r, f.id)))
        .collect();
    for f in found.iter().filter(|f| f.link) {
        if f.real
            .as_ref()
            .and_then(|r| real.get(r))
            .is_some_and(|other| *other != f.id)
        {
            out.push(f.id.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("uq-staleapps-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("home")).unwrap();
        fs::create_dir_all(d.join("sys")).unwrap();
        d
    }

    fn entry(exec: &str, extra: &str) -> String {
        format!("[Desktop Entry]\nType=Application\nName=App\nExec={exec}\n{extra}")
    }

    fn installed(p: &str) -> bool {
        ["firefox", "/usr/bin/emobie", "emobie", "flatpak"].contains(&p)
    }

    #[test]
    fn program_skips_env_and_its_settings() {
        assert_eq!(program("firefox %u").as_deref(), Some("firefox"));
        assert_eq!(
            program("env GDK_BACKEND=x11 -u FOO app --x").as_deref(),
            Some("app")
        );
        assert_eq!(
            program(r#""/home/u/Apps/a b.AppImage" %U"#).as_deref(),
            Some("/home/u/Apps/a b.AppImage")
        );
        assert_eq!(program(""), None);
    }

    #[test]
    fn gone_programs_and_try_exec_are_stale() {
        let d = dir("gone");
        let home = d.join("home");
        fs::write(home.join("ok.desktop"), entry("firefox %u", "")).unwrap();
        fs::write(
            home.join("grok.desktop"),
            entry("/home/u/.local/share/grok/appimage -- %U", ""),
        )
        .unwrap();
        fs::write(
            home.join("try.desktop"),
            entry("firefox", "TryExec=missing-bin\n"),
        )
        .unwrap();
        fs::write(home.join("bad.desktop"), "not an entry").unwrap();
        let dirs = Dirs::new(Some(home), vec![d.join("sys")]);
        let got = stale(
            &dirs,
            &["ok", "grok.desktop", "try", "bad", "unknown", "../x"],
            &installed,
        );
        assert_eq!(got, vec!["grok.desktop", "try"]);
    }

    #[test]
    fn a_symlinked_alias_of_a_listed_entry_is_stale() {
        let d = dir("alias");
        let sys = d.join("sys");
        fs::write(sys.join("io.github.x.emobie.desktop"), entry("emobie", "")).unwrap();
        std::os::unix::fs::symlink("io.github.x.emobie.desktop", sys.join("emobie.desktop"))
            .unwrap();
        let dirs = Dirs::new(Some(d.join("home")), vec![sys]);
        assert_eq!(
            stale(&dirs, &["emobie", "io.github.x.emobie"], &installed),
            vec!["emobie"]
        );
        // On its own the alias is the only way to the app, so it stays.
        assert!(stale(&dirs, &["emobie"], &installed).is_empty());
    }

    #[test]
    fn args_are_checked() {
        assert!(stale_apps(&[]).is_err());
        assert!(stale_apps(&[Value::from("x")]).is_err());
        assert!(stale_apps(&[Value::from(vec![Value::from("x"); MAX_IDS + 1])]).is_err());
        assert!(
            stale_apps(&[Value::Array(vec![])])
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}
