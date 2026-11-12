//! App actions (Phase 4 spec §2.4.2): `/usr/share/uq/launcher/actions/<id>.appaction` files in
//! desktop-entry syntax, shown in the right-click menu of matching app rows.
//!
//! ```ini
//! [App Action]
//! Name=Uninstall…
//! Icon=edit-delete
//! Match=*
//! Exec=/usr/bin/crumbs uninstall --desktop-id %d
//! Terminal=true
//! ```
//!
//! `%d` is the only placeholder: the row's desktop id, validated, always one whole argv entry.
//! `Exec` is split with the desktop-entry quoting rules; no shell is involved.

use std::path::{Path, PathBuf};

use crate::desktop_entry::{split_command, unescape_value};
use crate::integration::manifest::{valid_icon_name, valid_id, valid_text};

/// The group every `.appaction` file has, and the only one.
pub const GROUP: &str = "[App Action]";

const MAX_MATCH_CHARS: usize = 200;
const MAX_EXEC_ARGS: usize = 64;

/// One `Exec` argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecArg {
    Literal(String),
    /// `%d`: the row's desktop id.
    DesktopId,
}

/// A valid `.appaction` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppAction {
    /// The file name without `.appaction`.
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    /// A glob (`*`, `?`) on the desktop id, e.g. `appofholding-*.desktop`.
    pub match_glob: String,
    /// `argv[0]` is always a literal absolute path.
    pub exec: Vec<ExecArg>,
    /// Run in the terminal chosen in Settings; otherwise detached in its own scope.
    pub terminal: bool,
}

/// `^[A-Za-z0-9._-]{1,200}\.desktop$` (§2.4.2), not starting with `-` or `.`: `%d` is a whole
/// argument, so it must never read as an option.
pub fn valid_desktop_id(id: &str) -> bool {
    let Some(stem) = id.strip_suffix(".desktop") else {
        return false;
    };
    (1..=200).contains(&stem.len())
        && !stem.starts_with(['-', '.'])
        && stem
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// `*` matches any run of characters, `?` one character; everything else is literal.
pub fn glob_match(glob: &str, text: &str) -> bool {
    let (g, t): (Vec<char>, Vec<char>) = (glob.chars().collect(), text.chars().collect());
    let (mut gi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        match g.get(gi) {
            Some('*') => {
                star = Some((gi, ti));
                gi += 1;
            }
            Some(&c) if c == '?' || c == t[ti] => {
                gi += 1;
                ti += 1;
            }
            _ => match star {
                Some((sg, st)) => {
                    gi = sg + 1;
                    ti = st + 1;
                    star = Some((sg, st + 1));
                }
                None => return false,
            },
        }
    }
    g[gi..].iter().all(|&c| c == '*')
}

impl AppAction {
    /// The program, `argv[0]`.
    pub fn program(&self) -> &str {
        match self.exec.first() {
            Some(ExecArg::Literal(p)) => p,
            _ => "",
        }
    }

    /// Whether the action shows on the row of `desktop_id`.
    pub fn matches(&self, desktop_id: &str) -> bool {
        valid_desktop_id(desktop_id) && glob_match(&self.match_glob, desktop_id)
    }

    /// The argv for the row of `desktop_id`, which must be valid and match.
    pub fn argv(&self, desktop_id: &str) -> Result<Vec<String>, String> {
        if !valid_desktop_id(desktop_id) {
            return Err(format!("{desktop_id:?} is not a valid desktop id"));
        }
        if !glob_match(&self.match_glob, desktop_id) {
            return Err(format!("the action doesn't apply to {desktop_id:?}"));
        }
        Ok(self
            .exec
            .iter()
            .map(|a| match a {
                ExecArg::Literal(s) => s.clone(),
                ExecArg::DesktopId => desktop_id.to_string(),
            })
            .collect())
    }

    /// Whether the program exists (an action whose program is missing is hidden, §5).
    pub fn program_exists(&self) -> bool {
        Path::new(self.program()).is_file()
    }
}

fn parse_bool(key: &str, v: &str) -> Result<bool, String> {
    match v {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("{key} must be true or false, not {v:?}")),
    }
}

fn parse_exec(v: &str) -> Result<Vec<ExecArg>, String> {
    let words = split_command(v).map_err(|e| format!("Exec: {e}"))?;
    if words.is_empty() || words.len() > MAX_EXEC_ARGS {
        return Err(format!("Exec must have 1 to {MAX_EXEC_ARGS} arguments"));
    }
    let mut out = Vec::with_capacity(words.len());
    for (i, w) in words.into_iter().enumerate() {
        if w == "%d" {
            if i == 0 {
                return Err("Exec can't start with %d".into());
            }
            out.push(ExecArg::DesktopId);
        } else if w.contains('%') {
            return Err(format!(
                "Exec: {w:?}: %d is the only placeholder and must be a whole argument"
            ));
        } else {
            out.push(ExecArg::Literal(w));
        }
    }
    match out.first() {
        Some(ExecArg::Literal(p))
            if Path::new(p).is_absolute()
                && !p.ends_with('/')
                && !Path::new(p)
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir)) => {}
        _ => return Err("Exec must start with an absolute program path".into()),
    }
    Ok(out)
}

/// Parse an `.appaction` file. `id` is the file name without `.appaction`. Unknown keys, other
/// groups and repeated keys are errors; `Name[xx]` translations are accepted and ignored.
pub fn parse_app_action(id: &str, text: &str) -> Result<AppAction, String> {
    if !valid_id(id) {
        return Err(format!("file name {id:?} must match [a-z0-9-]{{1,40}}"));
    }
    let mut seen_group = false;
    let mut keys: Vec<String> = Vec::new();
    let (mut name, mut icon, mut glob, mut exec, mut terminal) = (None, None, None, None, None);
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            if line != GROUP || seen_group {
                return Err(format!("line {}: only one {GROUP} group is allowed", n + 1));
            }
            seen_group = true;
            continue;
        }
        if !seen_group {
            return Err(format!("line {}: a key before {GROUP}", n + 1));
        }
        let Some((k, v)) = line.split_once('=') else {
            return Err(format!("line {}: not a key=value line", n + 1));
        };
        let (k, v) = (k.trim(), v.trim());
        if keys.iter().any(|o| o == k) {
            return Err(format!("{k} is set twice"));
        }
        keys.push(k.to_string());
        match k {
            "Name" => name = Some(unescape_value(v)),
            "Icon" => icon = Some(unescape_value(v)),
            "Match" => glob = Some(unescape_value(v)),
            "Exec" => exec = Some(unescape_value(v)),
            "Terminal" => terminal = Some(parse_bool(k, v)?),
            k if k.starts_with("Name[") && k.ends_with(']') => {}
            k => return Err(format!("unknown key {k:?}")),
        }
    }
    if !seen_group {
        return Err(format!("no {GROUP} group"));
    }
    let name = name.ok_or("no Name")?;
    if !valid_text(&name, 100) {
        return Err("Name must be 1-100 characters without control characters".into());
    }
    if let Some(i) = &icon
        && !valid_icon_name(i)
    {
        return Err(format!(
            "Icon {i:?} must be an icon name matching [A-Za-z0-9._-]{{1,64}}"
        ));
    }
    let match_glob = glob.unwrap_or_else(|| "*".into());
    if match_glob.is_empty()
        || match_glob.chars().count() > MAX_MATCH_CHARS
        || !match_glob
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'*' | b'?'))
    {
        return Err(format!(
            "Match {match_glob:?} must be a glob of [A-Za-z0-9._-*?] on the desktop id"
        ));
    }
    let exec = parse_exec(&exec.ok_or("no Exec")?)?;
    Ok(AppAction {
        id: id.to_string(),
        name,
        icon,
        match_glob,
        exec,
        terminal: terminal.unwrap_or(false),
    })
}

/// Read and parse one `.appaction` file (at most 64 KiB, a regular file).
pub fn load_app_action(path: &Path) -> Result<AppAction, String> {
    let id = path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".appaction"))
        .ok_or("not an .appaction file")?
        .to_string();
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > super::manifest::MAX_MANIFEST_BYTES {
        return Err("not a regular file under 64 KiB".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_app_action(&id, &text)
}

/// Every `*.appaction` file in `dir`, sorted by name, each with its parse result.
pub fn scan_app_actions(dir: &Path) -> Vec<(PathBuf, Result<AppAction, String>)> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "appaction"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let r = load_app_action(&p);
            (p, r)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "# comment\n[App Action]\nName=Uninstall…\nName[de]=Entfernen…\n\
                        Icon=edit-delete\nMatch=*\n\
                        Exec=/usr/bin/crumbs uninstall --desktop-id %d\nTerminal=true\n";

    #[test]
    fn parses_the_spec_example() {
        let a = parse_app_action("crumbs-uninstall", GOOD).unwrap();
        assert_eq!(a.name, "Uninstall…");
        assert_eq!(a.icon.as_deref(), Some("edit-delete"));
        assert!(a.terminal);
        assert_eq!(a.program(), "/usr/bin/crumbs");
        assert_eq!(
            a.argv("org.gnome.Maps.desktop").unwrap(),
            [
                "/usr/bin/crumbs",
                "uninstall",
                "--desktop-id",
                "org.gnome.Maps.desktop"
            ]
        );
    }

    #[test]
    fn desktop_id_is_one_validated_argument() {
        let a = parse_app_action("x", GOOD).unwrap();
        for bad in [
            "a b.desktop",
            "--evil.desktop;rm",
            "x.desktop\n",
            "../x.desktop",
            ".desktop",
            "x",
            &format!("{}.desktop", "a".repeat(201)),
            // `%d` may follow no flag at all, so it must never look like an option.
            "-x.desktop",
            "-rf.desktop",
            ".hidden.desktop",
        ] {
            assert!(a.argv(bad).is_err(), "{bad:?}");
            assert!(!a.matches(bad), "{bad:?}");
        }
        assert_eq!(a.argv("x-.desktop").unwrap().len(), 4);
    }

    #[test]
    fn quoting_follows_desktop_entry_rules() {
        let a = parse_app_action(
            "q",
            "[App Action]\nName=Q\nExec=\"/usr/lib/my app/run\" \"two words\" %d\n",
        )
        .unwrap();
        assert_eq!(
            a.argv("a.desktop").unwrap(),
            ["/usr/lib/my app/run", "two words", "a.desktop"]
        );
        assert!(!a.terminal);
        assert_eq!(a.match_glob, "*");
    }

    #[test]
    fn match_globs() {
        let a = parse_app_action(
            "m",
            "[App Action]\nName=M\nMatch=appofholding-*.desktop\nExec=/usr/bin/aoh check %d\n",
        )
        .unwrap();
        assert!(a.matches("appofholding-firefox.desktop"));
        assert!(!a.matches("firefox.desktop"));
        assert!(a.argv("firefox.desktop").is_err());
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "ac"));
        assert!(glob_match("*.desktop", "x.y.desktop"));
        assert!(glob_match("**", ""));
        assert!(!glob_match("", "x"));
    }

    #[test]
    fn rejects_bad_files() {
        let body = |extra: &str| format!("[App Action]\nName=N\n{extra}");
        for (why, id, text) in [
            ("bad id", "Bad_Id", body("Exec=/usr/bin/x\n")),
            ("no group", "a", "Name=N\nExec=/usr/bin/x\n".to_string()),
            ("two groups", "a", body("Exec=/usr/bin/x\n[App Action]\n")),
            ("other group", "a", "[Desktop Entry]\nName=N\n".to_string()),
            ("unknown key", "a", body("Exec=/usr/bin/x\nSurprise=1\n")),
            (
                "repeated key",
                "a",
                body("Exec=/usr/bin/x\nExec=/usr/bin/y\n"),
            ),
            ("no exec", "a", body("")),
            (
                "no name",
                "a",
                "[App Action]\nExec=/usr/bin/x\n".to_string(),
            ),
            ("relative program", "a", body("Exec=crumbs %d\n")),
            ("dotdot program", "a", body("Exec=/usr/bin/../x %d\n")),
            ("placeholder program", "a", body("Exec=%d\n")),
            ("other field code", "a", body("Exec=/usr/bin/x %f\n")),
            (
                "placeholder in a word",
                "a",
                body("Exec=/usr/bin/x --id=%d\n"),
            ),
            ("literal percent", "a", body("Exec=/usr/bin/x 50%%\n")),
            ("unterminated", "a", body("Exec=/usr/bin/x \"open\n")),
            ("bad terminal", "a", body("Exec=/usr/bin/x\nTerminal=yes\n")),
            ("bad icon", "a", body("Exec=/usr/bin/x\nIcon=/tmp/x.png\n")),
            ("bad match", "a", body("Exec=/usr/bin/x\nMatch=[a]*\n")),
            ("not key value", "a", body("Exec=/usr/bin/x\njunk\n")),
            (
                "control in name",
                "a",
                "[App Action]\nName=a\u{7}\nExec=/usr/bin/x\n".into(),
            ),
        ] {
            assert!(parse_app_action(id, &text).is_err(), "{why}: {text:?}");
        }
    }

    #[test]
    fn scan_reads_only_appaction_files() {
        let d = tempfile::tempdir().unwrap();
        // The program lives in the temp dir, so the test doesn't depend on whether this machine
        // has crumbs installed (it does on a uQ desktop, where `uq-dev promote uq` runs check()).
        let program = d.path().join("crumbs");
        let good = GOOD.replace("/usr/bin/crumbs", program.to_str().unwrap());
        std::fs::write(d.path().join("a.appaction"), &good).unwrap();
        std::fs::write(d.path().join("b.appaction"), "junk").unwrap();
        std::fs::write(d.path().join("c.txt"), &good).unwrap();
        let found = scan_app_actions(d.path());
        assert_eq!(found.len(), 2);
        assert!(found[0].1.is_ok());
        assert!(found[1].1.is_err());
        let action = found[0].1.as_ref().unwrap();
        assert!(!action.program_exists(), "no program yet");
        std::fs::write(&program, "").unwrap();
        assert!(action.program_exists());
        assert!(scan_app_actions(Path::new("/nonexistent")).is_empty());
    }
}
