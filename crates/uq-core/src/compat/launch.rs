//! How a fix reaches an app: the launch command uQ's launch path and the autostart runner run,
//! the launcher-file override for other launchers, and the session-wide switch variables.
//! Pure: nothing here reads the disk or runs a process.

use super::browsers::{PLATFORM_FLAG, SCALE_FLAG, browser_for};
use super::hypr::{PLATFORM_ENV, SCALE_ENV, STEAM_ID, SWITCH_ENV, SwitchKey, Value, switch_var};
use super::{AppFix, Compat, Platform};
use crate::desktop_entry::{
    Token, escape_value, exec_tokens, get_key, groups_with_exec, normalise_id, read_entry, set_key,
    write_exec,
};
use crate::settings::LiveOutput;
use crate::settings::plan::fallback_main;

/// The first line of a launcher-file override.
pub const OVERRIDE_HEADER: &str = "# Written by uQ Settings (Apps and scaling). Remove the fix \
                                   there; uQ replaces this file when the app updates.";

const UNSAFE: &str = "This app's command starts with a name and =, so uQ can't put its \
                      settings in front of it.";
const BAD_EXEC: &str = "uQ can't safely change this app's launcher file.";
const NO_COMMAND: &str = "This app has no command to run.";
const NO_LAUNCHER_COMMAND: &str =
    "This app's launcher file has no command, so uQ can't add its fix to it.";

/// Why a fix can't be applied. The launch then goes ahead without the fix.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompatError {
    /// The command can't be wrapped safely: GNU `env` would read its program as a setting.
    #[error("{0}")]
    Unsafe(String),
    /// The command can't be read, or rewriting it would change what it runs.
    #[error("{0}")]
    BadExec(String),
}

/// The main screen's scale (R13): the enabled output at 0,0 (the first by name if several sit
/// there), else the first enabled output by name. `None` with no enabled output.
pub fn main_scale(live: &[LiveOutput]) -> Option<f64> {
    fallback_main(live).map(|o| o.scale)
}

/// GTK's whole-number scale (R14): nearest, with .5 rounding down, at least 1.
pub fn gdk_scale(s: f64) -> u32 {
    (s - 0.5).ceil().max(1.0) as u32
}

/// A scale with at most two places and no trailing zeros (`1.25`, `1.5`, `2`).
pub fn fmt_scale(s: f64) -> String {
    let t = format!("{s:.2}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn resolve(v: Value, scale: Option<f64>) -> Option<String> {
    match v {
        Value::Fixed(s) => Some(s.to_string()),
        Value::MainScaleInt => scale.map(|s| gdk_scale(s).to_string()),
        Value::MainScale => scale.map(fmt_scale),
    }
}

/// The variables the switches that are on set, in [`SWITCH_ENV`] order. A line whose value
/// needs a scale is left out when `main` is `None`.
pub fn switch_env(c: &Compat, main: Option<f64>) -> Vec<(String, String)> {
    SWITCH_ENV
        .iter()
        .filter(|(key, _, _)| c.switch(*key))
        .filter_map(|(_, name, v)| Some((name.to_string(), resolve(*v, main)?)))
        .collect()
}

/// A fix's variables: the platform's, then the size's, then the fix's own (so the user's
/// entries win).
fn fix_env(fix: &AppFix, id: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    if let Some((_, vars)) = PLATFORM_ENV
        .iter()
        .find(|(p, _)| fix.platform != Platform::Auto && *p == fix.platform.as_str())
    {
        out.extend(vars.iter().map(|(n, v)| (n.to_string(), v.to_string())));
    }
    if let Some(s) = fix.scale {
        out.extend(
            SCALE_ENV
                .iter()
                .filter_map(|(n, v)| Some((n.to_string(), resolve(*v, Some(s))?))),
        );
        if normalise_id(id) == STEAM_ID
            && let Some((name, v)) = switch_var(SwitchKey::SteamScale)
            && let Some(value) = resolve(v, Some(s))
        {
            out.push((name.to_string(), value));
        }
    }
    out.extend(fix.env.iter().map(|v| (v.name.clone(), v.value.clone())));
    out
}

/// The arguments a fix adds after the program's own: a browser's platform and size flags, then
/// the fix's extra options.
fn extra_args(fix: &AppFix, id: &str) -> Vec<String> {
    let mut out = Vec::new();
    if browser_for(id).is_some() {
        if fix.platform != Platform::Auto {
            out.push(format!("{PLATFORM_FLAG}{}", fix.platform.as_str()));
        }
        if let Some(s) = fix.scale {
            out.push(format!("{SCALE_FLAG}{}", fmt_scale(s)));
        }
    }
    out.extend(fix.args.iter().cloned());
    out
}

fn assignments(env: &[(String, String)]) -> impl Iterator<Item = String> + '_ {
    ["env".to_string(), "--".to_string()]
        .into_iter()
        .chain(env.iter().map(|(n, v)| format!("{n}={v}")))
}

/// The command for launching `id` with its fix:
/// `["env", "--", "K=V", …, <exec…>, <browser flags…>, <extra options…>]`. With no variables
/// there is no `env` wrapper, but the flags and options are still added (F25). Without a fix
/// the command is unchanged.
pub fn argv(fix: Option<&AppFix>, id: &str, exec: Vec<String>) -> Result<Vec<String>, CompatError> {
    let Some(fix) = fix else {
        return Ok(exec);
    };
    if exec.is_empty() {
        return Err(CompatError::BadExec(NO_COMMAND.into()));
    }
    let env = fix_env(fix, id);
    let extra = extra_args(fix, id);
    if env.is_empty() {
        return Ok(exec.into_iter().chain(extra).collect());
    }
    if exec[0].contains('=') {
        return Err(CompatError::Unsafe(UNSAFE.into()));
    }
    Ok(assignments(&env).chain(exec).chain(extra).collect())
}

/// One `Exec` value with the fix applied: the env wrapper in front, the flags and options before
/// the first file or URL field code (or Flatpak's `@@` marker) (or at the end). Field codes are kept.
fn wrap_exec(raw: &str, env: &[(String, String)], extra: &[String]) -> Result<String, CompatError> {
    let bad = || CompatError::BadExec(BAD_EXEC.into());
    let lexed = exec_tokens(raw).map_err(|_| bad())?;
    if lexed.lossy {
        return Err(bad());
    }
    let mut tokens = lexed.tokens;
    match tokens.first() {
        Some(Token::Arg(program)) if !env.is_empty() && program.contains('=') => {
            return Err(CompatError::Unsafe(UNSAFE.into()));
        }
        Some(Token::Arg(_)) => {}
        _ => return Err(bad()),
    }
    // Flatpak wraps the file or URL fields as `@@u %U @@`: options go before the opening marker,
    // or Flatpak would treat them as files.
    let at = tokens
        .iter()
        .position(|t| match t {
            Token::Field('f' | 'F' | 'u' | 'U') => true,
            Token::Arg(a) => matches!(a.as_str(), "@@" | "@@u" | "@@f"),
            _ => false,
        })
        .unwrap_or(tokens.len());
    tokens.splice(at..at, extra.iter().cloned().map(Token::Arg));
    if !env.is_empty() {
        tokens.splice(0..0, assignments(env).map(Token::Arg));
    }
    Ok(write_exec(&tokens))
}

/// The launcher-file override for `id`: the source copied line for line, every `Exec` in
/// `[Desktop Entry]` and each `[Desktop Action *]` wrapped, `DBusActivatable` turned off, and
/// the marker and the source path added, under a header. `None` when the fix adds nothing to
/// the command line. A source `Exec` that can't be rewritten exactly is refused (F12), and so is
/// a source whose `[Desktop Entry]` has no `Exec`.
pub fn override_text(
    source: &str,
    source_path: &str,
    fix: &AppFix,
    id: &str,
) -> Result<Option<String>, CompatError> {
    let env = fix_env(fix, id);
    let extra = extra_args(fix, id);
    if env.is_empty() && extra.is_empty() {
        return Ok(None);
    }
    // Without its own command (a D-Bus-only entry, or commands only in actions), the override
    // would turn D-Bus off and leave other launchers nothing to run.
    if get_key(source, "Desktop Entry", "Exec").is_none() {
        return Err(CompatError::BadExec(NO_LAUNCHER_COMMAND.into()));
    }
    let mut text = source.to_string();
    for group in groups_with_exec(source) {
        let raw = get_key(source, &group, "Exec").unwrap_or_default();
        text = set_key(&text, &group, "Exec", &wrap_exec(&raw, &env, &extra)?);
    }
    const ENTRY: &str = "Desktop Entry";
    if read_entry(source).dbus_activatable {
        text = set_key(&text, ENTRY, "DBusActivatable", "false");
    }
    text = set_key(&text, ENTRY, "X-uQ-Override", "true");
    text = set_key(&text, ENTRY, "X-uQ-Source", &escape_value(source_path));
    Ok(Some(format!("{OVERRIDE_HEADER}\n{text}")))
}
