//! "Start minimised": once a startup app's window opens, hide it. An app that has an icon in
//! the system tray goes to the tray (its window is closed, which is what a tray app does
//! with its window); one without a tray icon is minimised, which is the move the bar's tabs
//! make (`special:minimized`, tagged with the workspace it came from, so it can be restored).
//!
//! The watch is bounded: it ends [`WINDOW_WAIT`] after the app started, or [`GRACE`] after
//! the app's first window opened, whichever is later, so no timer outlives a login. Windows are
//! matched to the app by its systemd scope (`/proc/<pid>/cgroup`), so a window the user
//! opened later of the same program is never touched, and everything read from Hyprland or the
//! bus is untrusted text that is checked before it is used.

use super::spawn::cgroup_has_unit;
use crate::compositor::CompositorError;
use crate::hypr::HyprSocket;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

/// How long after an app starts its first window is waited for.
pub const WINDOW_WAIT: Duration = Duration::from_secs(60);
/// How long after the first window other windows of the app (a splash screen, then the main
/// window) are still hidden.
pub const GRACE: Duration = Duration::from_secs(15);
/// How long a window waits for its app's tray icon before it is minimised instead.
pub const TRAY_WAIT: Duration = Duration::from_secs(4);
const POLL: Duration = Duration::from_millis(300);

/// What happened to a startup app that was to start minimised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hidden {
    /// Its window was closed to the tray icon it has.
    Tray,
    /// Its window was minimised (the app has no tray icon).
    Minimised,
    /// No window opened in time.
    NoWindow,
}

impl Hidden {
    pub fn key(self) -> &'static str {
        match self {
            Hidden::Tray => "tray",
            Hidden::Minimised => "minimised",
            Hidden::NoWindow => "no_window",
        }
    }
}

/// Hides the windows of one started scope.
#[async_trait]
pub trait Hider: Send + Sync {
    /// Wait for the windows of the scope `unit` and hide them. `None` when the compositor
    /// couldn't be reached.
    async fn hide(&self, unit: &str) -> Option<Hidden>;
}

/// One window Hyprland lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    pub address: String,
    pub pid: u32,
    pub workspace: i64,
    pub mapped: bool,
}

/// `0x` and hex digits, as Hyprland writes a window's address.
fn valid_address(a: &str) -> bool {
    a.len() <= 18
        && a.strip_prefix("0x")
            .is_some_and(|h| !h.is_empty() && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// The windows in a `j/clients` reply; an entry that doesn't look right is skipped.
pub fn parse_clients(reply: &str) -> Vec<Client> {
    let Ok(Value::Array(list)) = serde_json::from_str::<Value>(reply) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|c| {
            let address = c["address"].as_str().filter(|a| valid_address(a))?;
            Some(Client {
                address: address.to_string(),
                pid: u32::try_from(c["pid"].as_u64()?).ok()?,
                workspace: c["workspace"]["id"].as_i64()?,
                mapped: c["mapped"].as_bool().unwrap_or(true),
            })
        })
        .collect()
}

fn lua_string(s: &str) -> String {
    format!("\"{s}\"")
}

/// The Lua that closes a window (an address that passed `valid_address`).
pub fn close_lua(address: &str) -> String {
    format!(
        "return hl.dispatch(hl.dsp.window.close({{ window = {} }}))",
        lua_string(&format!("address:{address}"))
    )
}

/// The two steps of a minimise, as the shell's tabs make it: remember the workspace, then move
/// the window to `special:minimized`.
pub fn minimise_lua(address: &str, from_ws: i64) -> [String; 2] {
    let win = lua_string(&format!("address:{address}"));
    [
        format!(
            "return hl.dispatch(hl.dsp.window.tag({{ tag = \"+uq_ws_{from_ws}\", window = {win} }}))"
        ),
        format!(
            "return hl.dispatch(hl.dsp.window.move({{ workspace = \"special:minimized\", follow = false, window = {win} }}))"
        ),
    ]
}

/// The tray items' bus names out of `RegisteredStatusNotifierItems`: an item is
/// `<bus name>/<object path>` (some apps register only a path or only a name).
pub fn item_bus_name(item: &str) -> Option<&str> {
    let name = item.split('/').next().filter(|n| !n.is_empty())?;
    (name.len() <= 255 && !name.contains(char::is_whitespace)).then_some(name)
}

/// Which processes own an icon in the system tray right now.
#[async_trait]
pub trait Tray: Send + Sync {
    async fn pids(&self) -> Vec<u32>;
}

/// The real tray: the pids behind the session bus's registered status notifier items.
#[derive(Default)]
pub struct BusTray {
    bus: tokio::sync::OnceCell<Option<zbus::Connection>>,
}

impl BusTray {
    async fn bus(&self) -> Option<&zbus::Connection> {
        self.bus
            .get_or_init(|| async { zbus::Connection::session().await.ok() })
            .await
            .as_ref()
    }
}

#[async_trait]
impl Tray for BusTray {
    async fn pids(&self) -> Vec<u32> {
        let Some(bus) = self.bus().await else {
            return Vec::new();
        };
        let items = bus
            .call_method(
                Some("org.kde.StatusNotifierWatcher"),
                "/StatusNotifierWatcher",
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &(
                    "org.kde.StatusNotifierWatcher",
                    "RegisteredStatusNotifierItems",
                ),
            )
            .await
            .ok()
            .and_then(|m| m.body().deserialize::<zbus::OwnedVariant>().ok())
            .and_then(|v| Vec::<String>::try_from(v).ok())
            .unwrap_or_default();
        let mut pids = Vec::new();
        for item in items.iter().take(64) {
            let Some(name) = item_bus_name(item) else {
                continue;
            };
            let pid = bus
                .call_method(
                    Some("org.freedesktop.DBus"),
                    "/org/freedesktop/DBus",
                    Some("org.freedesktop.DBus"),
                    "GetConnectionUnixProcessID",
                    &(name,),
                )
                .await
                .ok()
                .and_then(|m| m.body().deserialize::<u32>().ok());
            pids.extend(pid);
        }
        pids
    }
}

/// The real hider: Hyprland's request socket, the tray, and `/proc` for the scopes.
pub struct SystemHider {
    hypr: HyprSocket,
    tray: Box<dyn Tray>,
    proc: PathBuf,
    tray_wait: Duration,
    grace: Duration,
}

impl SystemHider {
    pub fn new(hypr: HyprSocket) -> SystemHider {
        SystemHider::with(hypr, Box::new(BusTray::default()), PathBuf::from("/proc"))
    }

    /// Any tray and any `/proc` (tests give a folder of `<pid>/cgroup` files).
    pub fn with(hypr: HyprSocket, tray: Box<dyn Tray>, proc: PathBuf) -> SystemHider {
        SystemHider {
            hypr,
            tray,
            proc,
            tray_wait: TRAY_WAIT,
            grace: GRACE,
        }
    }

    /// How long a window waits for its app's tray icon, and for more windows after the first
    /// (tests shorten both).
    pub fn with_waits(mut self, tray: Duration, grace: Duration) -> SystemHider {
        self.tray_wait = tray;
        self.grace = grace;
        self
    }

    async fn in_unit(&self, pid: u32, unit: &str) -> bool {
        tokio::fs::read_to_string(self.proc.join(pid.to_string()).join("cgroup"))
            .await
            .is_ok_and(|t| cgroup_has_unit(&t, unit))
    }

    async fn has_tray(&self, unit: &str) -> bool {
        for pid in self.tray.pids().await {
            if self.in_unit(pid, unit).await {
                return true;
            }
        }
        false
    }

    /// The windows of `unit` that are open and not handled yet.
    async fn fresh(
        &self,
        unit: &str,
        seen: &BTreeSet<String>,
    ) -> Result<Vec<Client>, CompositorError> {
        let clients = parse_clients(&self.hypr.request("j/clients").await?);
        let mut out = Vec::new();
        for c in clients {
            if c.mapped && !seen.contains(&c.address) && self.in_unit(c.pid, unit).await {
                out.push(c);
            }
        }
        Ok(out)
    }

    async fn hide_one(&self, unit: &str, c: &Client) -> Result<Hidden, CompositorError> {
        let waited = tokio::time::Instant::now();
        let mut tray = false;
        loop {
            tray = tray || self.has_tray(unit).await;
            if tray || waited.elapsed() >= self.tray_wait {
                break;
            }
            tokio::time::sleep(POLL).await;
        }
        if tray {
            self.hypr.eval(&close_lua(&c.address)).await?;
            return Ok(Hidden::Tray);
        }
        for step in minimise_lua(&c.address, c.workspace) {
            self.hypr.eval(&step).await?;
        }
        Ok(Hidden::Minimised)
    }
}

#[async_trait]
impl Hider for SystemHider {
    async fn hide(&self, unit: &str) -> Option<Hidden> {
        let start = tokio::time::Instant::now();
        let mut seen = BTreeSet::new();
        let mut first: Option<tokio::time::Instant> = None;
        let mut result = Hidden::NoWindow;
        loop {
            let now = tokio::time::Instant::now();
            let over = match first {
                Some(f) => now.duration_since(f) >= self.grace,
                None => now.duration_since(start) >= WINDOW_WAIT,
            };
            if over {
                return Some(result);
            }
            let fresh = match self.fresh(unit, &seen).await {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!("[START-003] start minimised: reading the windows: {e}");
                    return None;
                }
            };
            for c in fresh {
                seen.insert(c.address.clone());
                first.get_or_insert(tokio::time::Instant::now());
                match self.hide_one(unit, &c).await {
                    // A window sent to the tray stays the answer; a later one doesn't undo it.
                    Ok(h) if result != Hidden::Tray => result = h,
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("[START-003] start minimised: hiding a window: {e}");
                        return None;
                    }
                }
            }
            tokio::time::sleep(POLL).await;
        }
    }
}

#[cfg(test)]
mod tests;
