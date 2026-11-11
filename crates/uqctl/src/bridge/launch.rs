//! `Launch`: run a desktop entry or an argv in its own systemd scope via Hyprland (spec §2.1, §2.8).
//!
//! Nothing here runs a process itself: the command string is handed to Hyprland's `exec_cmd`, so
//! the app is never a child of the shell or the bridge. Every argument is single-quoted for the
//! shell that `exec_cmd` uses, and the whole string is then Lua-escaped.

use super::desktop_entry::{Dirs, binary_exists, parse_entry, parse_exec, unit_id};
use super::uqd::Link;
use serde_json::Value;
use std::path::Path;
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
#[path = "launch_tests.rs"]
mod tests;
