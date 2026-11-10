//! Desktop-entry parsing: the `Exec` line, the `[Desktop Entry]` keys launching needs, and where
//! entries live. Pure parsing; nothing here runs a process.

use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExecError {
    #[error("empty Exec line")]
    Empty,
    #[error("unterminated quote in Exec")]
    Unterminated,
    #[error("deprecated field code %{0} in Exec")]
    Deprecated(char),
    #[error("unknown field code %{0} in Exec")]
    Unknown(char),
    #[error("dangling % at end of Exec")]
    DanglingPercent,
}

/// Split a desktop-entry `Exec` value into argv. Quoting uses `"`; inside quotes a backslash
/// escapes one of `" ` $ \`. `%%` is a literal `%`, the plain field codes are dropped (this
/// launcher passes no files or URLs), and deprecated or unknown codes are errors.
pub fn parse_exec(exec: &str) -> Result<Vec<String>, ExecError> {
    let mut args: Vec<String> = Vec::new();
    let mut cur = String::new();
    // An argument exists once it has literal text or was quoted (so "" is an empty argument).
    let mut live = false;
    let mut quoted = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                live = true;
            }
            '\\' if quoted => match chars.next() {
                Some(n @ ('"' | '`' | '$' | '\\')) => cur.push(n),
                Some(n) => {
                    cur.push('\\');
                    cur.push(n);
                }
                None => return Err(ExecError::Unterminated),
            },
            '%' => match chars.next() {
                Some('%') => {
                    cur.push('%');
                    live = true;
                }
                Some('f' | 'F' | 'u' | 'U' | 'i' | 'c' | 'k') => {}
                Some(d @ ('d' | 'D' | 'n' | 'N' | 'v' | 'm')) => {
                    return Err(ExecError::Deprecated(d));
                }
                Some(o) => return Err(ExecError::Unknown(o)),
                None => return Err(ExecError::DanglingPercent),
            },
            c if c.is_ascii_whitespace() && !quoted => {
                if live || !cur.is_empty() {
                    args.push(std::mem::take(&mut cur));
                    live = false;
                }
            }
            c => {
                cur.push(c);
                live = true;
            }
        }
    }
    if quoted {
        return Err(ExecError::Unterminated);
    }
    if live || !cur.is_empty() {
        args.push(cur);
    }
    if args.is_empty() {
        return Err(ExecError::Empty);
    }
    Ok(args)
}

/// The parts of a desktop entry that launching needs.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Entry {
    pub(super) exec: String,
    pub(super) terminal: bool,
    pub(super) try_exec: Option<String>,
    /// `X-TerminalArgExec`: the argument a terminal takes before the command to run.
    pub(super) terminal_arg: Option<String>,
}

/// Undo the key-value escapes of the desktop-entry format (`\s \n \t \r \\`).
fn unescape_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut it = v.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Read the `[Desktop Entry]` group. Only `Exec`, `Terminal`, `TryExec`, `X-TerminalArgExec`, `Type` and `Hidden` count.
pub(super) fn parse_entry(text: &str) -> Result<Entry, String> {
    let (mut in_group, mut exec, mut terminal, mut try_exec) = (false, None, false, None);
    let (mut hidden, mut kind, mut terminal_arg) = (false, None, None);
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_group = line == "[Desktop Entry]";
            continue;
        }
        if !in_group || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim();
        match k.trim() {
            "Exec" => exec = Some(unescape_value(v)),
            "TryExec" => try_exec = Some(unescape_value(v)),
            "Terminal" => terminal = v == "true",
            "X-TerminalArgExec" => terminal_arg = Some(unescape_value(v)),
            "Hidden" => hidden = v == "true",
            "Type" => kind = Some(v.to_string()),
            _ => {}
        }
    }
    if hidden {
        return Err("desktop entry is hidden".into());
    }
    if kind.as_deref().is_some_and(|k| k != "Application") {
        return Err("desktop entry is not an application".into());
    }
    Ok(Entry {
        exec: exec.ok_or("desktop entry has no Exec")?,
        terminal,
        try_exec,
        terminal_arg,
    })
}

/// Where desktop entries live, in lookup order.
#[derive(Debug, Clone)]
pub struct Dirs(pub(super) Vec<PathBuf>);

impl Dirs {
    pub fn from_env() -> Dirs {
        let get = |k| std::env::var(k).ok().filter(|v: &String| !v.is_empty());
        let home = get("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| get("HOME").map(|h| PathBuf::from(h).join(".local/share")));
        let dirs = get("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        Dirs(
            home.into_iter()
                .chain(dirs.split(':').filter(|d| !d.is_empty()).map(PathBuf::from))
                .map(|d| d.join("applications"))
                .collect(),
        )
    }

    pub(super) fn find(&self, id: &str) -> Option<PathBuf> {
        let file = if id.ends_with(".desktop") {
            id.to_string()
        } else {
            format!("{id}.desktop")
        };
        self.0.iter().map(|d| d.join(&file)).find(|p| p.is_file())
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
