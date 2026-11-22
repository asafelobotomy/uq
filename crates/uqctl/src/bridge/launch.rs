//! `Launch`: run a desktop entry or an argv in its own systemd scope via Hyprland (spec §2.1, §2.8).
//!
//! Nothing here runs a process itself: the command string is handed to Hyprland's `exec_cmd`, so
//! the app is never a child of the shell or the bridge. Every argument is single-quoted for the
//! shell that `exec_cmd` uses, and the whole string is then Lua-escaped.

use super::desktop_entry::{
    Dirs, FieldCtx, LaunchEntry, binary_exists, exec_argv, normalise_id, parse_entry, parse_exec,
    unit_id, valid_override_source,
};
use super::uqd::Link;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uq_core::compat::{AppFix, Compat};
pub use uq_core::desktop_entry::random_suffix;
use uq_core::lua::lua_string;
use uq_core::settings::{provider_enabled, valid_pinned_id};
use uqd::compositor::CompositorError;
use uqd::hypr::HyprSocket;

/// POSIX single-quote: safe for any bytes.
pub fn shell_quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', r"'\''"))
}

/// The command Hyprland's `exec_cmd` runs: a transient user scope named after the app.
pub fn scope_command(app_id: &str, argv: &[String], rand: u32) -> String {
    let id = unit_id(app_id);
    let quoted: Vec<String> = argv.iter().map(|a| shell_quote(a)).collect();
    format!(
        "systemd-run --user --scope --collect --slice=app.slice --unit=app-uq-{id}-{rand:08x} -- {}",
        quoted.join(" ")
    )
}

/// The Lua `hyprctl eval` runs for a command string.
pub fn exec_lua(cmd: &str) -> String {
    format!("return hl.dispatch(hl.dsp.exec_cmd({}))", lua_string(cmd))
}

/// Where a terminal opened from look=see>do_ goes: floating, at `x, y` (from the top-left of
/// its monitor) and `w` by `h`, without taking focus from look=see>do_.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Place {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    /// An output name, when the window must open on a given monitor.
    pub monitor: Option<String>,
}

/// [`exec_lua`] with the window rules for a placed window (Hyprland applies them to the first
/// window the command opens).
pub fn exec_lua_placed(cmd: &str, place: Option<&Place>) -> String {
    let Some(p) = place else {
        return exec_lua(cmd);
    };
    let monitor = p
        .monitor
        .as_deref()
        .map(|m| format!("monitor = {}, ", lua_string(m)))
        .unwrap_or_default();
    format!(
        "return hl.dispatch(hl.dsp.exec_cmd({}, {{ float = true, {monitor}size = {{ {}, {} }}, move = {{ {}, {} }}, no_initial_focus = true }}))",
        lua_string(cmd),
        p.w,
        p.h,
        p.x,
        p.y
    )
}

/// The `place` argument of a terminal run: `{x, y, w, h}` whole numbers, the size positive, and
/// an optional `monitor` that is a plain output name.
fn parse_place(v: &Value) -> Result<Place, String> {
    let n = |k: &str, lo: i64, hi: i64| match v.get(k).and_then(Value::as_i64) {
        Some(n) if (lo..=hi).contains(&n) => Ok(n),
        _ => Err(format!(
            "place.{k} must be a whole number from {lo} to {hi}"
        )),
    };
    let monitor = match v.get("monitor") {
        None | Some(Value::Null) => None,
        Some(Value::String(m)) if m.is_empty() => None,
        Some(Value::String(m))
            if m.len() <= 64
                && m.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.:".contains(c)) =>
        {
            Some(m.clone())
        }
        Some(_) => return Err("place.monitor must be an output name".into()),
    };
    Ok(Place {
        x: n("x", -65_536, 65_536)?,
        y: n("y", -65_536, 65_536)?,
        w: n("w", 1, 32_768)?,
        h: n("h", 1, 32_768)?,
        monitor,
    })
}

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

/// Whether `argv0` is kitty, the terminal that can be asked for a new tab from outside (over a
/// remote-control socket that only the docked window listens on).
fn is_kitty(argv0: &str) -> bool {
    Path::new(argv0).file_name().is_some_and(|n| n == "kitty")
}

/// A docked terminal's handle: `kitty-` and 8 lowercase hex digits. It names the socket, so
/// nothing else can be slipped into the path.
pub fn valid_tabs_handle(h: &str) -> bool {
    h.strip_prefix("kitty-").is_some_and(|x| {
        x.len() == 8
            && x.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// The socket a docked terminal with handle `h` listens on, in the runtime folder (0700).
fn tabs_socket(runtime: &Path, h: &str) -> PathBuf {
    runtime.join(format!("uq-lookseedo-{h}.sock"))
}

/// The terminal a launch falls back to: the first of uQ's terminals that is installed (kyko,
/// else kitty), and kitty when neither is found, so the launch still says what it wanted.
pub(crate) fn fallback_terminal() -> &'static str {
    uq_core::kdeglobals::TERMINAL_APPS
        .into_iter()
        .find(|t| binary_exists(t))
        .unwrap_or("kitty")
}

/// The argv that runs a command in a terminal: the chosen terminal's own `Exec` (field codes
/// stripped) and then its `X-TerminalArgExec`, else `-e`. Anything wrong with the terminal
/// (none chosen, not installed, hidden, `TryExec` missing, bad `Exec`, or itself a terminal
/// app) gives `fallback_terminal() -e`: a launch is never refused over the terminal. Nothing here goes
/// through a shell.
pub(crate) fn terminal_prefix(dirs: &Dirs, terminal: Option<&str>) -> Vec<String> {
    let fallback = || vec![fallback_terminal().to_string(), "-e".to_string()];
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

/// How long a launch waits for uqd's fixes before going ahead without one (F2).
pub const FIX_WAIT: Duration = Duration::from_millis(500);

/// The source a marked override names, when it may be launched with the fix instead (R19, F20):
/// [`uq_core::desktop_entry::valid_override_source`] (shared with uqd's autostart runner), and
/// a regular file. Anything else is `None`, and the override runs as written.
pub fn valid_source(dirs: &Dirs, id: &str, source: &str) -> Option<PathBuf> {
    valid_override_source(dirs.home.as_deref(), &dirs.system, id, source)
        .filter(|p| std::fs::metadata(p).is_ok_and(|m| m.is_file()))
}

fn read_launch_entry(id: &str, path: &Path) -> Result<LaunchEntry, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_entry(&text).map_err(|e| format!("{id}: {e}"))
}

/// The file to launch and whether the fix still applies to it. A marked override already
/// carries a fix, so it is never given one again (R19): with a fix, its own valid source is
/// launched instead; otherwise the override runs as written.
fn resolve<'f>(
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
    runs: &super::run::RunState,
    terminal: Option<&str>,
    fix: Option<&AppFix>,
) -> Result<Plan, String> {
    let obj = arg
        .and_then(Value::as_object)
        .ok_or("Launch takes an object argument")?;
    if let Some(run_id) = obj.get("run_id") {
        let cmd =
            super::run::take_command(runs, run_id.as_str().ok_or("run_id must be a string")?)?;
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
            // A new tab in the docked terminal: kitty's own client asks the window over its
            // socket, and the command runs as that window's child.
            let sock = dirs
                .runtime
                .as_deref()
                .map(|r| tabs_socket(r, h))
                .filter(|p| p.exists())
                .ok_or("the docked terminal is closed")?;
            let prefix = terminal_prefix(dirs, terminal);
            let kitty = if is_kitty(&prefix[0]) {
                prefix[0].clone()
            } else {
                "kitty".to_string()
            };
            let mut argv = vec![
                kitty,
                "@".into(),
                "--to".into(),
                format!("unix:{}", sock.display()),
                "launch".into(),
                "--type=tab".into(),
            ];
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
                && is_kitty(&v[0])
                && let Some(runtime) = dirs.runtime.as_deref()
            {
                let h = format!("kitty-{rand:08x}");
                let listen = format!("unix:{}", tabs_socket(runtime, &h).display());
                v.splice(
                    1..1,
                    [
                        "-o".to_string(),
                        "allow_remote_control=socket-only".into(),
                        "--listen-on".into(),
                        listen,
                    ],
                );
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

/// The fix uqd holds for `id`, fetched for one launch. uqd down, slow (over [`FIX_WAIT`]) or
/// answering garbage gives `None`: a launch never fails over a fix.
pub async fn fix_for(link: &Link, id: &str) -> Option<AppFix> {
    let args = [Value::String("compat".into())];
    let call = link.call("GetSection", &args);
    let v = tokio::time::timeout(FIX_WAIT, call).await.ok()?.ok()?;
    fix_in(v, id)
}

/// The fix for `id` in a `compat` section (read leniently, as uqd loads its state).
pub fn fix_in(section: Value, id: &str) -> Option<AppFix> {
    let compat: Compat = serde_json::from_value(section).ok()?;
    compat.fix_for(id).cloned()
}

/// Send a plan to Hyprland; Ok means Hyprland accepted the eval.
pub async fn dispatch(hypr: &HyprSocket, plan: &Plan) -> Result<(), String> {
    match hypr
        .eval(&exec_lua_placed(&plan.command, plan.place.as_ref()))
        .await
    {
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
    let prefs = super::providers::provider_prefs(link);
    if args.first().and_then(|a| a.get("run_id")).is_some() && !provider_enabled(&prefs, "run") {
        // A run id from before Run was switched off: forget the command and refuse.
        runs.lock().unwrap_or_else(|e| e.into_inner()).forget();
        return Err("Run is switched off".into());
    }
    let terminal = link
        .settings_rx()
        .borrow()
        .as_ref()
        .and_then(|s| s.shell.terminal.clone());
    let id = args
        .first()
        .and_then(|a| a.get("desktop_id"))
        .and_then(Value::as_str)
        .filter(|id| valid_pinned_id(id));
    let fix = match id {
        Some(id) => fix_for(link, id).await,
        None => None,
    };
    let plan = {
        let runs = runs.lock().unwrap_or_else(|e| e.into_inner());
        plan(
            args.first(),
            &Dirs::from_env(),
            random_suffix(),
            &runs,
            terminal.as_deref(),
            fix.as_ref(),
        )?
    };
    dispatch(&HyprSocket::from_env(), &plan).await?;
    if let Some(id) = &plan.record {
        // Counting is best effort: a launch that worked must not fail because uqd is down.
        let _ = link
            .call("RecordLaunch", &[Value::String(id.clone())])
            .await;
    }
    Ok(match plan.tabs {
        Some(h) => serde_json::json!({ "tabs": h }),
        None => Value::Null,
    })
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "launch_fix_tests.rs"]
mod fix_tests;
