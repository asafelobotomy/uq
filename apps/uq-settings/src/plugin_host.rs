//! The `PluginHost` singleton: what a plugin page's `ctx.command` and `ctx.dbus` reach (Phase 4
//! spec §2.3, Phase 3 spec §2.5). QML side: `qml/components/PluginContext.qml`.
//!
//! Every request returns an id at once; its result arrives later as `finished(id, json)`, always
//! queued (never during the call), so the page has registered its callback by then.
//!
//! - **Commands** run only as declared in the page's `[[command]]` tables, through
//!   `uq_core::integration::command`. A command with `confirm` first emits
//!   `confirmRequested(id, page, text, plan)` and runs only after `answer(id, true)`; the text is
//!   the manifest's, with `%s` filled from the first argument. With `preview` (`uq_api` 3), the
//!   preview command runs first and its answer, cleaned by `uq_core::integration::plan`, is the
//!   `plan` (JSON; "" without a preview). A plan that says `would_refuse` can't be confirmed.
//!   One run per command at a time.
//! - **Streams** (`ctx.command.stream`, `uq_api` 3): a command with `output = "ndjson"` run with
//!   `stream_command` emits `progress(id, event)` for each line it prints, then `finished` as
//!   usual (its `output` is `null`: the events were not kept).
//! - **D-Bus** reaches only the page's own name. With a `[dbus]` table, only its path, interface
//!   and listed methods and properties (no target is passed). With only Phase 3's `dbus_name`,
//!   the page names the object path and interface, and any method or property on that name is
//!   allowed. Types come from introspection (`uq_core::integration::dbus_json`), with a 2 s
//!   deadline (or the method's `[dbus] timeout_ms`, for a write that may ask for a password),
//!   at most 8 requests in flight per page. A page that waits for its name
//!   (`requires.bus_name`) never starts the service.
//!
//! - **Watches** (`ctx.command.watch`): a command with a `watch` table is followed (files with
//!   inotify, D-Bus signals by match rule, `uq_core::integration::command_watch`); each quiet
//!   burst of changes emits `commandChanged(watch)` and the page's context re-runs it. At most
//!   [`MAX_WATCHES`] per page; `unwatch` (or the page closing) stops it.
//! - **Opening an app** (`ctx.openApp`): a plain `name.desktop` id, started by `uqctl launch`,
//!   the shell's own launch path (the app's fix, its own scope, counted), detached from Settings.
//!
//! This limits mistakes, not attacks: a page is trusted package code.

use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::{Value, json};
use uq_core::integration::command::{self, BusySet, CommandError, Runner};
use uq_core::integration::command_watch::{self, WATCH_DEBOUNCE, WatchSpec};
use uq_core::integration::dbus_json::{self, Target};
use uq_core::integration::manifest::{Bus, CommandSpec, Output, PageManifest};
use uq_core::integration::plan;

use crate::plugins;
mod bus;
mod commands;
pub use bus::*;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type PluginHost = super::PluginHostRust;

        /// Run `command` of page `page` with `args_json` (a JSON array of strings). Returns the
        /// request id.
        #[qinvokable]
        fn run_command(
            self: Pin<&mut PluginHost>,
            page: &QString,
            command: &QString,
            args_json: &QString,
        ) -> i32;

        /// As `run_command` for a command with `output = "ndjson"`: `progress(request, event)` for
        /// each line, then `finished` with `{code, output: null}` or `{error, …}`.
        #[qinvokable]
        fn stream_command(
            self: Pin<&mut PluginHost>,
            page: &QString,
            command: &QString,
            args_json: &QString,
        ) -> i32;

        /// The answer to `confirmRequested(request, …)`: run it, or report it cancelled.
        #[qinvokable]
        fn answer(self: Pin<&mut PluginHost>, request: i32, confirmed: bool);

        /// Call `method` on the page's D-Bus name with `args_json` (a JSON array). `target_json`
        /// is `{path, interface}` for a page with only `dbus_name`, else "" or "{}".
        #[qinvokable]
        fn dbus_call(
            self: Pin<&mut PluginHost>,
            page: &QString,
            method: &QString,
            args_json: &QString,
            target_json: &QString,
        ) -> i32;

        /// Read `property` (as `dbus_call` for the target).
        #[qinvokable]
        fn dbus_get(
            self: Pin<&mut PluginHost>,
            page: &QString,
            property: &QString,
            target_json: &QString,
        ) -> i32;

        /// Follow `command` of page `page` (it must have a `watch` table). Returns the watch id
        /// (`commandChanged(id)` after each change), or 0 with a reason on stderr.
        #[qinvokable]
        fn watch_command(self: Pin<&mut PluginHost>, page: &QString, command: &QString) -> i32;

        /// Stop the watch `watch`.
        #[qinvokable]
        fn unwatch(self: Pin<&mut PluginHost>, watch: i32);

        /// A watched command's files or signals changed: run it again.
        #[qsignal]
        fn command_changed(self: Pin<&mut PluginHost>, watch: i32);

        /// Open the app `desktop_id` (`name.desktop`) as the launcher does. Returns the request
        /// id; `finished` gets `{ok: true}` or `{error}`.
        #[qinvokable]
        fn open_app(self: Pin<&mut PluginHost>, page: &QString, desktop_id: &QString) -> i32;

        /// A request ended: `{code, output}` or `{error, …}` for commands, `{value}` or
        /// `{error}` for D-Bus, `{error, cancelled: true}` when a confirmation was declined.
        #[qsignal]
        fn finished(self: Pin<&mut PluginHost>, request: i32, result_json: QString);

        /// A streamed command printed one JSON line.
        #[qsignal]
        fn progress(self: Pin<&mut PluginHost>, request: i32, event_json: QString);

        /// A command needs the person's confirmation: show `text` (plain text) with Cancel and
        /// Confirm, then call `answer`. `plan_json` is the previewed plan, or "" for none.
        #[qsignal]
        fn confirm_requested(
            self: Pin<&mut PluginHost>,
            request: i32,
            page: QString,
            text: QString,
            plan_json: QString,
        );
    }

    impl cxx_qt::Threading for PluginHost {}
}

use qobject::PluginHost;

/// At most this many watches per page at once.
const MAX_WATCHES: usize = 8;

/// The `watch` table of `command` on `page`, or why it can't be followed.
pub fn watch_spec(manifest: Option<PageManifest>, command: &str) -> Result<WatchSpec, String> {
    let m = manifest.ok_or("this page is not installed any more")?;
    let spec = command::find(&m.commands, command).map_err(|e| e.to_string())?;
    spec.watch
        .clone()
        .ok_or_else(|| format!("command {command:?} has no watch in page.toml"))
}

/// How long `uqctl launch` may take to hand the app to Hyprland.
const OPEN_APP_WAIT: Duration = Duration::from_secs(10);

/// The command that opens `desktop_id` for a page: `uqctl launch <id>`, the launcher's path.
pub fn open_app_argv(desktop_id: &str) -> Result<Vec<String>, String> {
    if !uq_core::integration::appaction::valid_desktop_id(desktop_id) {
        return Err(format!(
            "{} is not an app id (expected a name like org.example.App.desktop)",
            uq_core::desktop_entry::clean_display(desktop_id, 80)
        ));
    }
    Ok(vec!["uqctl".into(), "launch".into(), desktop_id.into()])
}

/// Run `argv` detached from Settings (own process group, no stdio) and wait for it, so it is
/// reaped: `{ok: true}` on exit 0, else `{error}`.
pub async fn launch(argv: &[String], within: Duration) -> Value {
    let child = tokio::process::Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return error(format!("the app couldn't be opened: {e}")),
    };
    match tokio::time::timeout(within, child.wait()).await {
        Ok(Ok(status)) if status.success() => json!({"ok": true}),
        Ok(Ok(_)) => error("the app couldn't be opened (is it installed?)"),
        Ok(Err(e)) => error(format!("the app couldn't be opened: {e}")),
        // Still going: leave it running and let tokio reap it later.
        Err(_) => json!({"ok": true}),
    }
}

/// At most this many D-Bus requests in flight per page (Phase 4 spec §2.2).
const MAX_IN_FLIGHT: usize = 8;

/// The runtime the host's work runs on; set by `main` before the window exists.
static RUNTIME: OnceLock<tokio::runtime::Handle> = OnceLock::new();

pub fn set_runtime(handle: tokio::runtime::Handle) {
    let _ = RUNTIME.set(handle);
}

fn busy() -> &'static BusySet {
    static BUSY: OnceLock<BusySet> = OnceLock::new();
    BUSY.get_or_init(BusySet::new)
}

fn runner() -> &'static Runner {
    static RUNNER: OnceLock<Runner> = OnceLock::new();
    RUNNER.get_or_init(Runner::system)
}

fn in_flight() -> &'static Mutex<HashMap<String, usize>> {
    static IN_FLIGHT: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();
    IN_FLIGHT.get_or_init(Mutex::default)
}

/// A slot of a page's D-Bus budget, freed on drop.
struct Slot(String);

impl Slot {
    fn take(page: &str) -> Option<Slot> {
        let mut map = in_flight().lock().unwrap_or_else(|e| e.into_inner());
        let n = map.entry(page.to_string()).or_insert(0);
        if *n >= MAX_IN_FLIGHT {
            return None;
        }
        *n += 1;
        Some(Slot(page.to_string()))
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut map = in_flight().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(n) = map.get_mut(&self.0) {
            *n = n.saturating_sub(1);
        }
    }
}

struct Pending {
    page: String,
    spec: CommandSpec,
    args: Vec<String>,
    /// Run as a stream (`stream_command`).
    stream: bool,
    /// The plan said `would_refuse`: Confirm can't run it, whatever the page sends.
    refused: bool,
}

#[derive(Default)]
pub struct PluginHostRust {
    next: i32,
    pending: HashMap<i32, Pending>,
    /// watch id -> (page, the task following it; aborting it stops the watch).
    watches: HashMap<i32, (String, tokio::task::JoinHandle<()>)>,
}

fn error(msg: impl std::fmt::Display) -> Value {
    json!({"error": msg.to_string()})
}

/// A declared command of `page` and the caller's checked arguments.
pub fn prepare(
    manifest: Option<PageManifest>,
    command: &str,
    args_json: &str,
) -> Result<(CommandSpec, Vec<String>), String> {
    let m = manifest.ok_or("this page is not installed any more")?;
    let spec = command::find(&m.commands, command)
        .map_err(|e| e.to_string())?
        .clone();
    let args: Vec<String> = if args_json.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(args_json).map_err(|_| "arguments must be a list of text values")?
    };
    runner().argv(&spec, &args).map_err(|e| e.to_string())?;
    Ok((spec, args))
}

impl PluginHost {
    fn next_id(self: Pin<&mut Self>) -> i32 {
        let mut rust = self.rust_mut();
        rust.next = rust.next.wrapping_add(1).max(1);
        rust.next
    }

    /// Emit `finished(request, value)` from the Qt event loop, later.
    fn deliver(&self, request: i32, value: Value) {
        let json = QString::from(&value.to_string());
        let _ = self
            .qt_thread()
            .queue(move |host| host.finished(request, json));
    }
}

#[cfg(test)]
mod tests;
