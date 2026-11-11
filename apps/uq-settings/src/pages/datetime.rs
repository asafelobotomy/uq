//! The Date and time page (spec §2.6): the time zone, network time and the manual date and
//! time through timedated (its own polkit prompts), and the bar's 24-hour clock, which is uQ's
//! `shell.clock_24h` in uqd. Without uqd the clock switch is unavailable; the rest still works.

use std::sync::Mutex as StdMutex;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::client::DesktopProxy;

use super::proxies::{TimedateProxy, fresh};
use super::runner::{PageLogic, parse_json, patch_section};
use super::sysbus::{self, SystemBus, TIMEDATE, failure, settle};

/// 2000-01-01 and 2200-01-01 in microseconds: a manual time outside is a typing slip.
const EARLIEST: i64 = 946_684_800_000_000;
const LATEST: i64 = 7_258_118_400_000_000;

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    SetTimezone {
        zone: String,
    },
    SetNtp {
        on: bool,
    },
    /// Microseconds since 1970 (UTC).
    SetTime {
        usec: i64,
    },
    SetClock24h {
        on: bool,
    },
}

pub struct DateTimeState {
    session: Option<zbus::Connection>,
    system: SystemBus,
    write: Mutex<()>,
    notice: StdMutex<(u64, String)>,
}

impl DateTimeState {
    pub fn new(session: zbus::Connection) -> Self {
        Self::with(Some(session), SystemBus::system())
    }

    pub fn with(session: Option<zbus::Connection>, system: SystemBus) -> Self {
        Self {
            session,
            system,
            write: Mutex::new(()),
            notice: StdMutex::new((0, String::new())),
        }
    }

    async fn timedate(&self) -> zbus::Result<TimedateProxy<'static>> {
        let conn = self.system.conn().await?;
        fresh(&conn).await
    }

    async fn uqd(&self) -> zbus::Result<DesktopProxy<'static>> {
        let conn = self
            .session
            .as_ref()
            .ok_or_else(|| failure("uQ's settings service isn't running."))?;
        DesktopProxy::new(conn).await
    }

    async fn read_timedate(&self) -> Value {
        let read = async {
            let t = self.timedate().await?;
            let zone = t.timezone().await?;
            let zones = t.list_timezones().await.unwrap_or_default();
            zbus::Result::Ok(json!({
                "available": true,
                "timezone": zone,
                "ntp": t.ntp().await.unwrap_or(false),
                "can_ntp": t.can_ntp().await.unwrap_or(false),
                "synced": t.ntp_synchronized().await.unwrap_or(false),
                "zones": zones,
            }))
        };
        match sysbus::within(read).await {
            Ok(v) => v,
            Err(e) => sysbus::read_error(&e, &TIMEDATE),
        }
    }

    /// `shell.clock_24h`, or `None` when uqd can't be read.
    async fn clock_24h(&self) -> Option<bool> {
        let proxy = self.uqd().await.ok()?;
        let section = parse_json(&sysbus::within(proxy.get_section("shell")).await.ok()?).ok()?;
        Some(section["clock_24h"].as_bool().unwrap_or(true))
    }
}

impl PageLogic for DateTimeState {
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
            Op::SetTimezone { zone } => {
                let t = self.timedate().await?;
                let zones = t.list_timezones().await.unwrap_or_default();
                if !zones.is_empty() && !zones.contains(&zone) {
                    return Err(failure("That time zone isn't known on this computer."));
                }
                settle(t.set_timezone(&zone, true).await, &TIMEDATE)
            }
            Op::SetNtp { on } => {
                let t = self.timedate().await?;
                settle(t.set_ntp(on, true).await, &TIMEDATE)
            }
            Op::SetTime { usec } => {
                if !(EARLIEST..LATEST).contains(&usec) {
                    return Err(failure("That date isn't between 2000 and 2200."));
                }
                let t = self.timedate().await?;
                if t.ntp().await.unwrap_or(false) {
                    return Err(failure(
                        "The time is set from the network. Turn that off to set it by hand.",
                    ));
                }
                let r = t.set_time(usec, false, true).await;
                let ok = r.is_ok();
                settle(r, &TIMEDATE)?;
                if ok {
                    let mut n = self.notice.lock().unwrap();
                    *n = (n.0 + 1, "The date and time were set.".into());
                }
                Ok(())
            }
            Op::SetClock24h { on } => {
                let proxy = self.uqd().await?;
                patch_section(&proxy, "shell", |o| {
                    o.insert("clock_24h".into(), json!(on));
                    Ok(())
                })
                .await
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let (seq, text) = self.notice.lock().unwrap().clone();
        Ok(json!({
            "timedate": self.read_timedate().await,
            "clock_24h": self.clock_24h().await,
            "notice": {"seq": seq, "text": text},
        }))
    }
}

#[cfg(test)]
mod tests;
