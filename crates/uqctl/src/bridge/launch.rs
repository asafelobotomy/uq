//! `Launch`: run a desktop entry or an argv in its own systemd scope via Hyprland (spec §2.1, §2.8).
//!
//! Nothing here runs a process itself: the command string is handed to Hyprland's `exec_cmd`, so
//! the app is never a child of the shell or the bridge. Every argument is single-quoted for the
//! shell that `exec_cmd` uses, and the whole string is then Lua-escaped.

use super::uqd::Link;
use serde_json::Value;
use std::path::{Path, PathBuf};
use uq_core::lua::lua_string;
use uq_core::settings::valid_pinned_id;
use uqd::compositor::CompositorError;
use uqd::hypr::HyprSocket;

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

/// POSIX single-quote: safe for any bytes.
pub fn shell_quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', r"'\''"))
}

/// The command Hyprland's `exec_cmd` runs: a transient user scope named after the app.
pub fn scope_command(app_id: &str, argv: &[String], rand: u32) -> String {
    let id: String = app_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    let quoted: Vec<String> = argv.iter().map(|a| shell_quote(a)).collect();
    format!(
        "systemd-run --user --scope --collect --unit=app-uq-{id}-{rand:08x} -- {}",
        quoted.join(" ")
    )
}

/// The Lua `hyprctl eval` runs for a command string.
pub fn exec_lua(cmd: &str) -> String {
    format!("return hl.dispatch(hl.dsp.exec_cmd({}))", lua_string(cmd))
}

/// The parts of a desktop entry that launching needs.
#[derive(Debug, PartialEq, Eq)]
struct Entry {
    exec: String,
    terminal: bool,
    try_exec: Option<String>,
    /// `X-TerminalArgExec`: the argument a terminal takes before the command to run.
    terminal_arg: Option<String>,
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
fn parse_entry(text: &str) -> Result<Entry, String> {
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
pub struct Dirs(Vec<PathBuf>);

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

    fn find(&self, id: &str) -> Option<PathBuf> {
        let file = if id.ends_with(".desktop") {
            id.to_string()
        } else {
            format!("{id}.desktop")
        };
        self.0.iter().map(|d| d.join(&file)).find(|p| p.is_file())
    }
}

fn binary_exists(name: &str) -> bool {
    if name.contains('/') {
        return Path::new(name).is_file();
    }
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

/// A launch ready to send: the shell command and the id to record, if it was a desktop entry.
#[derive(Debug, PartialEq, Eq)]
pub struct Plan {
    pub command: String,
    pub record: Option<String>,
}

const FALLBACK_TERMINAL: [&str; 2] = ["kitty", "-e"];

/// The argv that runs a command in a terminal: the chosen terminal's own `Exec` (field codes
/// stripped) and then its `X-TerminalArgExec`, else `-e`. Anything wrong with the terminal
/// (none chosen, not installed, hidden, `TryExec` missing, bad `Exec`, or itself a terminal
/// app) gives `kitty -e`: a launch is never refused over the terminal. Nothing here goes
/// through a shell.
fn terminal_prefix(dirs: &Dirs, terminal: Option<&str>) -> Vec<String> {
    let fallback = || FALLBACK_TERMINAL.map(String::from).to_vec();
    let Some(id) = terminal.filter(|t| valid_pinned_id(t)) else {
        return fallback();
    };
    let Some(path) = dirs.find(id) else {
        return fallback();
    };
    let Ok(entry) = std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|t| parse_entry(&t))
    else {
        return fallback();
    };
    if entry.terminal || entry.try_exec.as_deref().is_some_and(|t| !binary_exists(t)) {
        return fallback();
    }
    let Ok(mut argv) = parse_exec(&entry.exec) else {
        return fallback();
    };
    argv.push(
        entry
            .terminal_arg
            .filter(|a| !a.is_empty())
            .unwrap_or_else(|| "-e".into()),
    );
    argv
}

pub fn plan(
    arg: Option<&Value>,
    dirs: &Dirs,
    rand: u32,
    runs: &super::run::RunState,
    terminal: Option<&str>,
) -> Result<Plan, String> {
    let obj = arg
        .and_then(Value::as_object)
        .ok_or("Launch takes an object argument")?;
    if let Some(run_id) = obj.get("run_id") {
        let cmd =
            super::run::take_command(runs, run_id.as_str().ok_or("run_id must be a string")?)?;
        let argv = ["sh".to_string(), "-c".to_string(), cmd];
        return Ok(Plan {
            command: scope_command("run", &argv, rand),
            record: None,
        });
    }
    if let Some(argv) = obj.get("argv") {
        let argv: Vec<String> = argv
            .as_array()
            .ok_or("argv must be an array of strings")?
            .iter()
            .map(|a| a.as_str().map(str::to_string))
            .collect::<Option<_>>()
            .ok_or("argv must be an array of strings")?;
        let first = argv
            .first()
            .filter(|a| !a.is_empty())
            .ok_or("argv is empty")?;
        let app = Path::new(first)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("app");
        return Ok(Plan {
            command: scope_command(app, &argv, rand),
            record: None,
        });
    }
    let id = obj
        .get("desktop_id")
        .and_then(Value::as_str)
        .ok_or("Launch needs desktop_id, argv or run_id")?;
    if !valid_pinned_id(id) {
        return Err(format!("invalid desktop id {id:?}"));
    }
    let path = dirs
        .find(id)
        .ok_or_else(|| format!("no desktop entry {id}"))?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let entry = parse_entry(&text).map_err(|e| format!("{id}: {e}"))?;
    if let Some(t) = &entry.try_exec
        && !binary_exists(t)
    {
        return Err(format!("{id}: TryExec {t} not found"));
    }
    let mut argv = parse_exec(&entry.exec).map_err(|e| format!("{id}: {e}"))?;
    if entry.terminal {
        argv.splice(0..0, terminal_prefix(dirs, terminal));
    }
    Ok(Plan {
        command: scope_command(id.strip_suffix(".desktop").unwrap_or(id), &argv, rand),
        record: Some(id.to_string()),
    })
}

/// A small non-cryptographic value: it only keeps scope names apart.
pub fn random_suffix() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let mut x =
        nanos ^ std::process::id().rotate_left(16) ^ N.fetch_add(0x9e37_79b9, Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 17;
    x ^ (x << 5)
}

/// Send a plan to Hyprland; Ok means Hyprland accepted the eval.
pub async fn dispatch(hypr: &HyprSocket, plan: &Plan) -> Result<(), String> {
    match hypr.eval(&exec_lua(&plan.command)).await {
        Ok(()) => Ok(()),
        Err(CompositorError::NotRunning) => Err("Hyprland is not running".into()),
        Err(CompositorError::Rejected(msg)) => Err(msg),
        Err(e) => Err(e.to_string()),
    }
}

/// The bridge's `Launch` call.
pub async fn launch(
    link: &Link,
    args: &[Value],
    runs: &std::sync::Mutex<super::run::RunState>,
) -> Result<Value, String> {
    let terminal = link
        .settings_rx()
        .borrow()
        .as_ref()
        .and_then(|s| s.shell.terminal.clone());
    let plan = {
        let runs = runs.lock().unwrap_or_else(|e| e.into_inner());
        plan(
            args.first(),
            &Dirs::from_env(),
            random_suffix(),
            &runs,
            terminal.as_deref(),
        )?
    };
    dispatch(&HyprSocket::from_env(), &plan).await?;
    if let Some(id) = &plan.record {
        // Counting is best effort: a launch that worked must not fail because uqd is down.
        let _ = link
            .call("RecordLaunch", &[Value::String(id.clone())])
            .await;
    }
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::run::{RunState, run_item};

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

    #[test]
    fn shell_quote_single_quotes() {
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(shell_quote("a b"), "'a b'");
    }

    #[test]
    fn launch_wraps_in_systemd_run_scope() {
        assert_eq!(
            scope_command("org.mozilla.firefox", &v(&["firefox"]), 0xabcd),
            "systemd-run --user --scope --collect --unit=app-uq-org_mozilla_firefox-0000abcd -- 'firefox'"
        );
        let long = "x".repeat(100);
        let cmd = scope_command(&format!("{long}/../a b"), &v(&["a"]), 1);
        assert!(
            cmd.contains(&format!("--unit=app-uq-{}-00000001 ", "x".repeat(64))),
            "{cmd}"
        );
    }

    #[test]
    fn exec_lua_escapes() {
        assert_eq!(
            exec_lua("echo \"a\"\nb"),
            r#"return hl.dispatch(hl.dsp.exec_cmd("echo \"a\"\nb"))"#
        );
    }

    #[test]
    fn hostile_desktop_entry_stays_quoted() {
        let argv = parse_exec(r#"evil "a; rm -rf ~""#).unwrap();
        assert_eq!(argv, v(&["evil", "a; rm -rf ~"]));
        let cmd = scope_command("evil", &argv, 0);
        assert!(cmd.ends_with("-- 'evil' 'a; rm -rf ~'"), "{cmd}");
    }

    use serde_json::json;

    fn dirs_with(files: &[(&str, &str)]) -> (tempfile::TempDir, Dirs) {
        let t = tempfile::tempdir().unwrap();
        let apps = t.path().join("applications");
        std::fs::create_dir_all(&apps).unwrap();
        for (n, c) in files {
            std::fs::write(apps.join(n), c).unwrap();
        }
        let d = Dirs(vec![t.path().join("none/applications"), apps]);
        (t, d)
    }

    #[test]
    fn desktop_id_plans_scope_and_records() {
        let (_t, d) = dirs_with(&[(
            "org.a.B.desktop",
            "[Desktop Entry]\nType=Application\nExec=b --x %U\nName=B\n[Desktop Action n]\nExec=zzz\n",
        )]);
        let p = plan(
            Some(&json!({"desktop_id":"org.a.B"})),
            &d,
            1,
            &RunState::default(),
            None,
        )
        .unwrap();
        assert_eq!(
            p.command,
            "systemd-run --user --scope --collect --unit=app-uq-org_a_B-00000001 -- 'b' '--x'"
        );
        assert_eq!(p.record.as_deref(), Some("org.a.B"));
    }

    fn htop_plan(d: &Dirs, terminal: Option<&str>) -> Plan {
        plan(
            Some(&json!({"desktop_id":"t.desktop"})),
            d,
            0,
            &RunState::default(),
            terminal,
        )
        .unwrap()
    }

    const HTOP: (&str, &str) = ("t.desktop", "[Desktop Entry]\nExec=htop\nTerminal=true\n");

    #[test]
    fn terminal_apps_get_kitty_by_default_and_bad_entries_refuse() {
        let (_t, d) = dirs_with(&[
            HTOP,
            ("h.desktop", "[Desktop Entry]\nExec=h\nHidden=true\n"),
            (
                "m.desktop",
                "[Desktop Entry]\nExec=m\nTryExec=/no/such/bin\n",
            ),
            ("d.desktop", "[Desktop Entry]\nExec=d %d\n"),
        ]);
        let p = htop_plan(&d, None);
        assert!(
            p.command.ends_with("-- 'kitty' '-e' 'htop'"),
            "{}",
            p.command
        );
        for id in ["h", "m", "d", "absent"] {
            assert!(
                plan(
                    Some(&json!({"desktop_id":id})),
                    &d,
                    0,
                    &RunState::default(),
                    None
                )
                .is_err(),
                "{id}"
            );
        }
        // Ids that could escape the applications folder never reach the filesystem.
        for id in ["../x", "a/b", "", "a b"] {
            let e = plan(
                Some(&json!({"desktop_id":id})),
                &d,
                0,
                &RunState::default(),
                None,
            )
            .unwrap_err();
            assert!(e.starts_with("invalid desktop id"), "{e}");
        }
    }

    #[test]
    fn terminal_uses_chosen_entry() {
        let (_t, d) = dirs_with(&[HTOP, ("foot.desktop", "[Desktop Entry]\nExec=foot\n")]);
        for id in ["foot", "foot.desktop"] {
            let p = htop_plan(&d, Some(id));
            assert!(
                p.command.ends_with("-- 'foot' '-e' 'htop'"),
                "{}",
                p.command
            );
        }
    }

    #[test]
    fn terminal_arg_from_x_terminal_arg_exec() {
        let (_t, d) = dirs_with(&[
            HTOP,
            (
                "gnome-terminal.desktop",
                "[Desktop Entry]\nExec=gnome-terminal %U\nX-TerminalArgExec=--\n",
            ),
        ]);
        let p = htop_plan(&d, Some("gnome-terminal"));
        assert!(
            p.command.ends_with("-- 'gnome-terminal' '--' 'htop'"),
            "{}",
            p.command
        );
    }

    #[test]
    fn terminal_falls_back_to_kitty() {
        let (_t, d) = dirs_with(&[
            HTOP,
            ("bad.desktop", "[Desktop Entry]\nExec=bad %d\n"),
            ("hid.desktop", "[Desktop Entry]\nExec=hid\nHidden=true\n"),
            (
                "gone.desktop",
                "[Desktop Entry]\nExec=gone\nTryExec=/no/such/bin\n",
            ),
            ("tty.desktop", "[Desktop Entry]\nExec=tty\nTerminal=true\n"),
            ("noexec.desktop", "[Desktop Entry]\nName=x\n"),
        ]);
        for t in [
            "missing", "bad", "hid", "gone", "tty", "noexec", "../x", "a/b", "",
        ] {
            let p = htop_plan(&d, Some(t));
            assert!(
                p.command.ends_with("-- 'kitty' '-e' 'htop'"),
                "{t}: {}",
                p.command
            );
        }
    }

    #[test]
    fn hostile_terminal_entry_stays_argv() {
        let (_t, d) = dirs_with(&[
            HTOP,
            (
                "evil.desktop",
                "[Desktop Entry]\nExec=evil \"a; rm -rf ~\" %f\nX-TerminalArgExec=$(id)\n",
            ),
        ]);
        let p = htop_plan(&d, Some("evil"));
        assert!(
            p.command
                .ends_with("-- 'evil' 'a; rm -rf ~' '$(id)' 'htop'"),
            "{}",
            p.command
        );
    }

    #[test]
    fn argv_and_run_id_and_junk() {
        let d = Dirs(vec![]);
        let p = plan(
            Some(&json!({"argv":["/usr/bin/foot","-e","a b"]})),
            &d,
            2,
            &RunState::default(),
            None,
        )
        .unwrap();
        assert_eq!(
            p.command,
            "systemd-run --user --scope --collect --unit=app-uq-foot-00000002 -- '/usr/bin/foot' '-e' 'a b'"
        );
        assert_eq!(p.record, None);
        let mut runs = RunState::default();
        run_item("> echo hi", 7, &mut runs).unwrap();
        let p = plan(Some(&json!({"run_id":"run-7"})), &d, 3, &runs, None).unwrap();
        assert_eq!(
            p.command,
            "systemd-run --user --scope --collect --unit=app-uq-run-00000003 -- 'sh' '-c' 'echo hi'"
        );
        assert_eq!(p.record, None);
        assert_eq!(
            plan(Some(&json!({"run_id":"run-8"})), &d, 0, &runs, None).unwrap_err(),
            "unknown run id"
        );
        assert!(plan(Some(&json!({"run_id":5})), &d, 0, &runs, None).is_err());
        for bad in [
            json!({"argv":[]}),
            json!({"argv":[1]}),
            json!({}),
            json!("x"),
        ] {
            assert!(
                plan(Some(&bad), &d, 0, &RunState::default(), None).is_err(),
                "{bad}"
            );
        }
        assert!(plan(None, &d, 0, &RunState::default(), None).is_err());
    }

    #[tokio::test]
    async fn dispatch_sends_eval_and_maps_errors() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("hypr/SIG");
        std::fs::create_dir_all(&sock).unwrap();
        let l = tokio::net::UnixListener::bind(sock.join(".socket.sock")).unwrap();
        let server = tokio::spawn(async move {
            let mut seen = Vec::new();
            for reply in ["ok", "nope"] {
                let (mut s, _) = l.accept().await.unwrap();
                let mut buf = vec![0u8; 4096];
                let n = s.read(&mut buf).await.unwrap();
                seen.push(String::from_utf8_lossy(&buf[..n]).into_owned());
                s.write_all(reply.as_bytes()).await.unwrap();
            }
            seen
        });
        let h = HyprSocket::new(Some(dir.path().into()), Some("SIG".into()));
        let p = Plan {
            command: "c".into(),
            record: None,
        };
        dispatch(&h, &p).await.unwrap();
        assert_eq!(dispatch(&h, &p).await.unwrap_err(), "nope");
        assert_eq!(
            server.await.unwrap()[0],
            "eval return hl.dispatch(hl.dsp.exec_cmd(\"c\"))"
        );
        let off = HyprSocket::new(None, None);
        assert_eq!(
            dispatch(&off, &p).await.unwrap_err(),
            "Hyprland is not running"
        );
    }
}
