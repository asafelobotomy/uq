//! Planning a launch: the desktop entry or argv, the terminal it needs, and its systemd scope.

use super::*;

/// A launch ready to send: the shell command and the id to record, if it was a desktop entry.
#[derive(Debug, PartialEq, Eq)]
pub struct Plan {
    pub command: String,
    pub record: Option<String>,
    pub place: Option<Place>,
    /// For a terminal docked under look=see>do_: the handle that opens a new tab in it later
    /// (`Launch {run_id, terminal: true, tab}`); None when the terminal can't take tabs.
    pub tabs: Option<String>,
}

/// The file to launch and whether the fix still applies to it. A marked override already
/// carries a fix, so it is never given one again (R19): with a fix, its own valid source is
/// launched instead; otherwise the override runs as written.
pub(super) fn resolve<'f>(
    dirs: &Dirs,
    id: &str,
    fix: Option<&'f AppFix>,
) -> Result<(PathBuf, LaunchEntry, Option<&'f AppFix>), String> {
    let path = dirs.find(id).ok_or_else(|| format!("no app called {id}"))?;
    let entry = read_launch_entry(id, &path)?;
    if !entry.uq_override {
        return Ok((path, entry, fix));
    }
    let source = fix.and(entry.uq_source.as_deref()).and_then(|s| {
        let p = valid_source(dirs, id, s)?;
        let e = read_launch_entry(id, &p).ok().filter(|e| !e.uq_override)?;
        Some((p, e))
    });
    Ok(match source {
        Some((p, e)) => (p, e, fix),
        None => (path, entry, None),
    })
}

pub fn plan(
    arg: Option<&Value>,
    dirs: &Dirs,
    rand: u32,
    runs: &super::super::run::RunState,
    terminal: Option<&str>,
    fix: Option<&AppFix>,
) -> Result<Plan, String> {
    let obj = arg
        .and_then(Value::as_object)
        .ok_or("Launch takes an object argument")?;
    if let Some(run_id) = obj.get("run_id") {
        let cmd = super::super::run::take_command(
            runs,
            run_id.as_str().ok_or("run_id must be a string")?,
        )?;
        let in_terminal = match obj.get("terminal") {
            None => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => return Err("terminal must be true or false".into()),
        };
        let flag = |k: &str| match obj.get(k) {
            None | Some(Value::Null) => Ok(false),
            Some(Value::Bool(b)) => Ok(*b),
            Some(_) => Err(format!("{k} must be true or false")),
        };
        let dock = flag("dock")?;
        let tab = match obj.get("tab") {
            None | Some(Value::Null) => None,
            Some(Value::String(h)) if valid_tabs_handle(h) => Some(h.as_str()),
            Some(_) => return Err("tab must be a docked terminal's handle".into()),
        };
        if (dock || tab.is_some()) && !in_terminal {
            return Err("dock and tab need terminal: true".into());
        }
        // In a terminal the window stays until Enter, so the output can be read. The command is
        // an argument of the script ($1), never part of its text.
        let script = "eval \"$1\"; s=$?; printf '\\n[exited with %s: press Enter to close] ' \"$s\"; read _; exit \"$s\"";
        let run_args = || {
            let mut v: Vec<String> = ["sh", "-c", script, "sh"].map(String::from).to_vec();
            v.push(cmd.clone());
            v
        };
        if let Some(h) = tab {
            // A new tab in the docked terminal: the terminal's own client (`kitty @` or
            // `kyko @`) asks the window over its socket, and the command runs as that window's
            // child.
            let sock = dirs
                .runtime
                .as_deref()
                .map(|r| tabs_socket(r, h))
                .filter(|p| p.exists())
                .ok_or("the docked terminal is closed")?;
            let kind = tabs_handle_terminal(h).ok_or("tab must be a docked terminal's handle")?;
            let prefix = terminal_prefix(dirs, terminal);
            let client = if tab_terminal(&prefix[0]) == Some(kind) {
                prefix[0].clone()
            } else {
                kind.to_string()
            };
            let mut argv = vec![
                client,
                "@".into(),
                "--to".into(),
                format!("unix:{}", sock.display()),
                "launch".into(),
            ];
            // kyko takes `--type tab`; kitty takes either form.
            argv.extend(match kind {
                "kitty" => vec!["--type=tab".to_string()],
                _ => vec!["--type".to_string(), "tab".into()],
            });
            argv.extend(run_args());
            let quoted: Vec<String> = argv.iter().map(|a| shell_quote(a)).collect();
            return Ok(Plan {
                command: quoted.join(" "),
                record: None,
                place: None,
                tabs: None,
            });
        }
        let mut tabs = None;
        let argv: Vec<String> = if in_terminal {
            let mut v = terminal_prefix(dirs, terminal);
            if dock
                && let Some(kind) = tab_terminal(&v[0])
                && let Some(runtime) = dirs.runtime.as_deref()
            {
                let h = format!("{kind}-{rand:08x}");
                let listen = format!("unix:{}", tabs_socket(runtime, &h).display());
                // kitty needs remote control turned on for the socket; kyko's `--listen-on`
                // socket only ever answers its own user (0600 in the runtime folder).
                let mut extra = Vec::new();
                if kind == "kitty" {
                    extra.extend(["-o".to_string(), "allow_remote_control=socket-only".into()]);
                }
                extra.extend(["--listen-on".to_string(), listen]);
                v.splice(1..1, extra);
                tabs = Some(h);
            }
            v.extend(run_args());
            v
        } else {
            vec!["sh".to_string(), "-c".to_string(), cmd]
        };
        let place = match obj.get("place") {
            None | Some(Value::Null) => None,
            Some(_) if !in_terminal => return Err("place needs terminal: true".into()),
            Some(p) => Some(parse_place(p)?),
        };
        return Ok(Plan {
            command: scope_command("run", &argv, rand),
            record: None,
            place,
            tabs,
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
            place: None,
            tabs: None,
        });
    }
    let id = obj
        .get("desktop_id")
        .and_then(Value::as_str)
        .ok_or("Launch needs desktop_id, argv or run_id")?;
    if !valid_pinned_id(id) {
        return Err(format!("invalid desktop id {id:?}"));
    }
    // The order is fixed (F2): the file (R19), TryExec, Exec, the terminal, the fix, the scope.
    let (path, entry, fix) = resolve(dirs, id, fix)?;
    if let Some(t) = &entry.try_exec
        && !binary_exists(t)
    {
        return Err(format!("{id}: TryExec {t} not found"));
    }
    let path = path.to_string_lossy();
    let ctx = FieldCtx {
        icon: entry.icon.as_deref().filter(|i| !i.is_empty()),
        name: entry.name.as_deref(),
        path: Some(&path),
    };
    let mut argv = exec_argv(&entry.exec, &ctx).map_err(|e| format!("{id}: {e}"))?;
    if entry.terminal {
        argv.splice(0..0, terminal_prefix(dirs, terminal));
    }
    // A fix that can't be applied safely is left out: the app still starts (Review Focus 5).
    let argv = uq_core::compat::launch::argv(fix, &normalise_id(id), argv.clone()).unwrap_or(argv);
    Ok(Plan {
        command: scope_command(id.strip_suffix(".desktop").unwrap_or(id), &argv, rand),
        record: Some(id.to_string()),
        place: None,
        tabs: None,
    })
}
