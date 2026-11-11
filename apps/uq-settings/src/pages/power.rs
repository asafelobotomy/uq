//! The Power page (spec §2.6): the power mode (power-profiles-daemon), the battery (UPower), and
//! what closing the lid or pressing the power key does (logind, written through
//! `uq-settings-helper`'s `SetLogindPowerActions`). logind is never restarted, so a changed
//! action applies at its next reload; the page says so while the running value differs.
//!
//! While the page is open it follows UPower's and power-profiles-daemon's signals, so the
//! battery and the power mode update as they change. Only when the system bus can't be
//! followed does it fall back to reading everything again every [`FALLBACK_POLL`].

pub(crate) mod logind;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use futures_util::StreamExt;

use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify};
use zbus::proxy::CacheProperties;
use zbus::zvariant::OwnedValue;

use super::proxies::{
    HELPER_NAME, HelperProxy, Login1Proxy, POWER_PROFILES_NAMES, UPowerDeviceProxy, UPowerProxy,
    fresh, fresh_at,
};
use super::runner::PageLogic;
use super::sysbus::{self, HELPER, POWER_PROFILES, SystemBus, UPOWER, failure, settle};
use logind::{Actions, CHOICES};

pub const PENDING_NOTE: &str = "Saved. It takes effect the next time the computer starts.";
/// How often the page reads everything again when it can't follow the services' signals.
pub const FALLBACK_POLL: Duration = Duration::from_secs(60);
/// UPower's bus name; its objects (and the current power-profiles-daemon's) live under
/// `/org/freedesktop/UPower`.
const UPOWER_NAME: &str = "org.freedesktop.UPower";
const UPOWER_PATHS: &str = "/org/freedesktop/UPower";

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    SetProfile {
        name: String,
    },
    SetActions {
        lid: String,
        lid_on_ac: String,
        power_key: String,
    },
}

pub struct PowerState {
    system: SystemBus,
    root: PathBuf,
    write: Mutex<()>,
    notice: StdMutex<(u64, String)>,
    poll: Duration,
}

/// "3 h 20 min" for a number of seconds; "" for none.
pub fn duration_text(secs: i64) -> String {
    if secs <= 0 {
        return String::new();
    }
    let mins = (secs + 59) / 60;
    match (mins / 60, mins % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// UPower's battery state as words.
pub fn state_text(state: u32) -> &'static str {
    match state {
        1 => "Charging",
        2 => "On battery",
        3 => "Empty",
        4 => "Fully charged",
        5 => "Plugged in, not charging",
        6 => "Waiting to discharge",
        _ => "Unknown",
    }
}

impl PowerState {
    pub fn new(_session: zbus::Connection) -> Self {
        Self::with(SystemBus::system(), PathBuf::from("/"))
    }

    pub fn with(system: SystemBus, root: PathBuf) -> Self {
        Self {
            system,
            root,
            write: Mutex::new(()),
            notice: StdMutex::new((0, String::new())),
            poll: FALLBACK_POLL,
        }
    }

    /// The same page with another fallback poll interval (tests).
    #[cfg(test)]
    pub fn with_poll(mut self, poll: Duration) -> Self {
        self.poll = poll;
        self
    }

    /// Notify `changed` whenever UPower or power-profiles-daemon says something changed, or
    /// one of them starts or stops. Returns only when the bus stops delivering signals.
    async fn follow(&self, changed: &Notify) -> zbus::Result<()> {
        let conn = self.system.conn().await?;
        let mut rules = vec![
            zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .path_namespace(UPOWER_PATHS)?
                .build(),
        ];
        for (_, path, _) in POWER_PROFILES_NAMES {
            if !path.starts_with(UPOWER_PATHS) {
                rules.push(
                    zbus::MatchRule::builder()
                        .msg_type(zbus::message::Type::Signal)
                        .path(path)?
                        .build(),
                );
            }
        }
        let mut streams = Vec::new();
        for rule in rules {
            streams.push(
                zbus::MessageStream::for_match_rule(rule, &conn, Some(64))
                    .await?
                    .map(|_| ())
                    .boxed(),
            );
        }
        let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
        let names = std::iter::once(UPOWER_NAME).chain(POWER_PROFILES_NAMES.map(|(n, _, _)| n));
        for name in names {
            streams.push(
                dbus.receive_name_owner_changed_with_args(&[(0, name)])
                    .await?
                    .map(|_| ())
                    .boxed(),
            );
        }
        let mut all = futures_util::stream::select_all(streams);
        // Anything that changed while subscribing.
        changed.notify_one();
        while all.next().await.is_some() {
            changed.notify_one();
        }
        Ok(())
    }

    fn notice(&self, text: &str) {
        let mut n = self.notice.lock().unwrap();
        *n = (n.0 + 1, text.to_string());
    }

    /// power-profiles-daemon under whichever name this machine's release uses.
    async fn profiles_proxy(&self) -> zbus::Result<zbus::Proxy<'static>> {
        let conn = self.system.conn().await?;
        for (name, path, iface) in POWER_PROFILES_NAMES {
            if sysbus::has_service(&conn, name).await {
                return zbus::proxy::Builder::<zbus::Proxy<'static>>::new(&conn)
                    .destination(name)?
                    .path(path)?
                    .interface(iface)?
                    .cache_properties(CacheProperties::No)
                    .build()
                    .await;
            }
        }
        Err(failure(POWER_PROFILES.missing_text()))
    }

    async fn profiles(&self) -> Value {
        let read = async {
            let p = self.profiles_proxy().await?;
            let active: String = p.get_property("ActiveProfile").await?;
            let list: Vec<HashMap<String, OwnedValue>> = p.get_property("Profiles").await?;
            let degraded: String = p
                .get_property("PerformanceDegraded")
                .await
                .unwrap_or_default();
            let names: Vec<String> = list
                .iter()
                .filter_map(|d| String::try_from(d.get("Profile")?.try_clone().ok()?).ok())
                .collect();
            zbus::Result::Ok(json!({
                "available": true, "active": active, "list": names, "degraded": degraded,
            }))
        };
        match sysbus::within(read).await {
            Ok(v) => v,
            Err(zbus::Error::Failure(t)) if t == POWER_PROFILES.missing_text() => {
                POWER_PROFILES.missing_json()
            }
            Err(e) => sysbus::read_error(&e, &POWER_PROFILES),
        }
    }

    async fn battery(&self) -> Value {
        let read = async {
            let conn = self.system.conn().await?;
            let upower: UPowerProxy = fresh(&conn).await?;
            let on_battery = upower.on_battery().await.unwrap_or(false);
            let lid = upower.lid_is_present().await.unwrap_or(false);
            let mut battery = Value::Null;
            for path in upower.enumerate_devices().await? {
                let d: UPowerDeviceProxy = fresh_at(&conn, path).await?;
                if d.kind().await.unwrap_or(0) != 2
                    || !d.power_supply().await.unwrap_or(false)
                    || !d.is_present().await.unwrap_or(false)
                {
                    continue;
                }
                let state = d.state().await.unwrap_or(0);
                let time = match state {
                    1 => {
                        let t = duration_text(d.time_to_full().await.unwrap_or(0));
                        if t.is_empty() {
                            t
                        } else {
                            format!("Full in {t}")
                        }
                    }
                    2 => {
                        let t = duration_text(d.time_to_empty().await.unwrap_or(0));
                        if t.is_empty() {
                            t
                        } else {
                            format!("About {t} left")
                        }
                    }
                    _ => String::new(),
                };
                let health = d.capacity().await.unwrap_or(0.0);
                battery = json!({
                    "percentage": d.percentage().await.unwrap_or(0.0).round(),
                    "state": state_text(state),
                    "time": time,
                    "health": if health > 0.0 { json!(health.round()) } else { Value::Null },
                    "model": format!("{} {}", d.vendor().await.unwrap_or_default(),
                        d.model().await.unwrap_or_default()).trim().to_string(),
                });
                break;
            }
            zbus::Result::Ok(json!({
                "available": true, "on_battery": on_battery, "lid": lid, "battery": battery,
            }))
        };
        match sysbus::within(read).await {
            Ok(v) => v,
            Err(e) => sysbus::read_error(&e, &UPOWER),
        }
    }

    /// What logind is running with now, when it answers.
    async fn running(&self) -> Option<Actions> {
        let read = async {
            let conn = self.system.conn().await?;
            let l: Login1Proxy = fresh(&conn).await?;
            zbus::Result::Ok(Actions {
                lid: l.handle_lid_switch().await?,
                lid_on_ac: l.handle_lid_switch_external_power().await?,
                power_key: l.handle_power_key().await?,
            })
        };
        sysbus::within(read).await.ok()
    }

    async fn helper_up(&self) -> bool {
        match self.system.conn().await {
            Ok(c) => sysbus::has_service(&c, HELPER_NAME).await,
            Err(_) => false,
        }
    }
}

fn actions_json(a: &Actions) -> Value {
    json!({"lid": a.lid, "lid_on_ac": a.lid_on_ac, "power_key": a.power_key})
}

impl PageLogic for PowerState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        match op {
            Op::Refresh => Ok(()),
            Op::SetProfile { name } => {
                let p = self.profiles_proxy().await?;
                let r = p
                    .set_property("ActiveProfile", name.as_str())
                    .await
                    .map_err(|e| zbus::Error::FDO(Box::new(e)));
                settle(r, &POWER_PROFILES)
            }
            Op::SetActions {
                lid,
                lid_on_ac,
                power_key,
            } => {
                if [&lid, &lid_on_ac, &power_key]
                    .iter()
                    .any(|v| !CHOICES.contains(&v.as_str()))
                {
                    return Err(failure(
                        "One of the actions is set to something this page doesn't offer. Choose an action for each first.",
                    ));
                }
                let conn = self.system.conn().await?;
                let helper = HelperProxy::new(&conn).await?;
                let r = helper
                    .set_logind_power_actions(&lid, &lid_on_ac, &power_key)
                    .await;
                let ok = r.is_ok();
                settle(r, &HELPER)?;
                if ok {
                    self.notice(PENDING_NOTE);
                }
                Ok(())
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let saved = logind::configured(&self.root);
        let running = self.running().await;
        let pending = running.as_ref().is_some_and(|r| *r != saved);
        let (seq, text) = self.notice.lock().unwrap().clone();
        Ok(json!({
            "profiles": self.profiles().await,
            "power": self.battery().await,
            "actions": {
                "helper": self.helper_up().await,
                "helper_note": HELPER.missing_text(),
                "saved": actions_json(&saved),
                "running": running.as_ref().map(actions_json),
                "pending": pending,
                "choices": CHOICES,
            },
            "notice": {"seq": seq, "text": text},
        }))
    }

    async fn watch(&self, changed: Arc<Notify>) {
        match self.follow(&changed).await {
            Ok(()) => eprintln!("uq-settings: the system bus stopped sending power changes"),
            Err(e) => eprintln!(
                "uq-settings: can't follow power changes: {}",
                sysbus::error_text(&e)
            ),
        }
        // No signals: read the battery and power mode again now and then instead.
        let mut every = tokio::time::interval(self.poll);
        every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        every.tick().await;
        loop {
            every.tick().await;
            changed.notify_one();
        }
    }
}

#[cfg(test)]
mod tests;
