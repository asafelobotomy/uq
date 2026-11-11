//! The bridge's view of desktop entries: the shared parser from `uq_core::desktop_entry`, where
//! entries live, and what a launch needs from one. Pure parsing; nothing here runs a process.

pub use uq_core::desktop_entry::*;

use std::path::{Path, PathBuf};

/// The parts of a desktop entry that launching needs.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct LaunchEntry {
    pub(super) exec: String,
    pub(super) terminal: bool,
    pub(super) try_exec: Option<String>,
    /// `X-TerminalArgExec`: the argument a terminal takes before the command to run.
    pub(super) terminal_arg: Option<String>,
    /// For the `%i` and `%c` field codes.
    pub(super) icon: Option<String>,
    pub(super) name: Option<String>,
    /// A launcher-file override uQ wrote (`X-uQ-Override=true`), and the file it was made from.
    pub(super) uq_override: bool,
    pub(super) uq_source: Option<String>,
}

/// Read a launchable entry: not hidden, an Application (or untyped) and with an `Exec`.
pub(super) fn parse_entry(text: &str) -> Result<LaunchEntry, String> {
    let e = read_entry(text);
    if e.hidden {
        return Err("desktop entry is hidden".into());
    }
    if e.kind.as_deref().is_some_and(|k| k != "Application") {
        return Err("desktop entry is not an application".into());
    }
    Ok(LaunchEntry {
        exec: e.exec.ok_or("desktop entry has no Exec")?,
        terminal: e.terminal,
        try_exec: e.try_exec,
        terminal_arg: e.terminal_arg,
        icon: e.icon,
        name: e.name,
        uq_override: e.uq_override,
        uq_source: e.uq_source,
    })
}

/// Where desktop entries live: the data home's `applications` folder, then each data dir's, in
/// lookup order.
#[derive(Debug, Clone)]
pub struct Dirs {
    pub(super) home: Option<PathBuf>,
    pub(super) system: Vec<PathBuf>,
}

impl Dirs {
    /// `home` and `system` are `applications` folders.
    pub fn new(home: Option<PathBuf>, system: Vec<PathBuf>) -> Dirs {
        Dirs { home, system }
    }

    pub fn from_env() -> Dirs {
        let get = |k| std::env::var(k).ok().filter(|v: &String| !v.is_empty());
        let home = get("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| get("HOME").map(|h| PathBuf::from(h).join(".local/share")));
        let dirs = get("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        Dirs::new(
            home.map(|h| h.join("applications")),
            dirs.split(':')
                .filter(|d| !d.is_empty())
                .map(|d| PathBuf::from(d).join("applications"))
                .collect(),
        )
    }

    fn all(&self) -> impl Iterator<Item = &PathBuf> {
        self.home.iter().chain(&self.system)
    }

    pub(super) fn find(&self, id: &str) -> Option<PathBuf> {
        let file = normalise_id(id);
        self.all().map(|d| d.join(&file)).find(|p| p.is_file())
    }
}

pub(super) fn binary_exists(name: &str) -> bool {
    if name.contains('/') {
        return Path::new(name).is_file();
    }
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_exec_strips_field_codes() {
        assert_eq!(parse_exec("firefox %u").unwrap(), v(&["firefox"]));
        assert_eq!(
            parse_exec("app --f=%F x %%").unwrap(),
            v(&["app", "--f=", "x", "%"])
        );
    }

    #[test]
    fn parse_exec_quotes() {
        assert_eq!(
            parse_exec(r#"sh -c "echo \"hi\" \$HOME""#).unwrap(),
            v(&["sh", "-c", r#"echo "hi" $HOME"#])
        );
    }

    #[test]
    fn parse_exec_rejects_deprecated() {
        for code in ["%d", "%D", "%n", "%N", "%v", "%m", "%z"] {
            assert!(parse_exec(&format!("app {code}")).is_err(), "{code}");
        }
        assert!(parse_exec("").is_err());
        assert!(parse_exec(r#"app "unterminated"#).is_err());
    }
}
