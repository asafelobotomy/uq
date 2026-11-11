//! `Launch`: run a desktop entry or an argv in its own systemd scope via Hyprland (spec §2.1, §2.8).
//!
//! Nothing here runs a process itself: the command string is handed to Hyprland's `exec_cmd`, so
//! the app is never a child of the shell or the bridge. Every argument is single-quoted for the
//! shell that `exec_cmd` uses, and the whole string is then Lua-escaped.

use super::desktop_entry::{
    Dirs, FieldCtx, LaunchEntry, binary_exists, exec_argv, normalise_id, parse_entry, parse_exec,
    unit_id, valid_desktop_id,
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

/// How long a launch waits for uqd's fixes before going ahead without one (F2).
pub const FIX_WAIT: Duration = Duration::from_millis(500);

/// The source a marked override names, when it may be launched with the fix instead (R19, F20):
/// exactly `<d>/applications/<id>` for a data dir `d` other than the data home, and a regular
/// file. Anything else is `None`, and the override runs as written.
pub fn valid_source(dirs: &Dirs, id: &str, source: &str) -> Option<PathBuf> {
    if !valid_desktop_id(id) {
        return None;
    }
    let path = PathBuf::from(source);
    let file = normalise_id(id);
    let ok = dirs
        .system
        .iter()
        .filter(|d| dirs.home.as_ref() != Some(*d))
        .any(|d| d.join(&file) == path);
    let regular = std::fs::metadata(&path).is_ok_and(|m| m.is_file());
    (ok && regular).then_some(path)
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
    Ok(Value::Null)
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "launch_fix_tests.rs"]
mod fix_tests;
