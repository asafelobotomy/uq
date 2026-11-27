//! Network lookups (spec §2.11): Open-Meteo weather and geocoding, ipinfo.io country (and the public address, city, region and provider for the VPN details).
//! HTTPS only, 5 s timeout, nothing logged or persisted.

use super::protocol::Outbound;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify, mpsc, watch};
use tokio_util::sync::CancellationToken;
use uq_core::settings::{Location, Settings, round2};
mod client;
pub use client::*;

const MAX_NAME_CHARS: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("{0}")]
    BadInput(&'static str),
    /// Only the kind of failure; never the URL (it carries coordinates) or any body.
    #[error("request failed ({0})")]
    Request(&'static str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeoHit {
    pub name: String,
    pub country: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Weather {
    pub temp_c: f64,
    pub code: u16,
    pub is_day: bool,
    pub location_name: String,
    /// Unix seconds of the fetch.
    pub fetched_at: i64,
    /// Today's (max, min) in the location's time zone.
    pub today: Option<(f64, f64)>,
    /// The next 12 hours from the current one: (unix time, temp, code).
    pub hourly: Vec<(i64, f64, u16)>,
}

/// Hours shown in the forecast.
const HOURS: usize = 12;

impl Weather {
    pub fn to_json(&self) -> Value {
        json!({
            "temp_c": self.temp_c,
            "code": self.code,
            "is_day": self.is_day,
            "location_name": self.location_name,
            "fetched_at": self.fetched_at,
            "today": self.today.map(|(max, min)| json!({"max_c": max, "min_c": min})),
            "hourly": self.hourly.iter()
                .map(|(t, c, k)| json!({"time": t, "temp_c": c, "code": k}))
                .collect::<Vec<_>>(),
        })
    }
}

/// From parallel hourly arrays: the entries from the hour containing `now`, at most 12.
fn next_hours(
    times: &[i64],
    temps: &[Option<f64>],
    codes: &[Option<u16>],
    now: i64,
) -> Vec<(i64, f64, u16)> {
    times
        .iter()
        .zip(temps)
        .zip(codes)
        .filter(|((t, _), _)| **t + 3600 > now)
        .filter_map(|((t, c), k)| Some((*t, (*c)?, (*k)?)))
        .take(HOURS)
        .collect()
}

/// Where the internet sees this computer, for the status island's VPN details: the country, and
/// the public address, city, region and provider when ipinfo gives them. `loc` (coordinates)
/// and everything else ipinfo sends is dropped at parse time.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Country {
    pub country_code: String,
    pub country: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org: Option<String>,
}

/// A short, printable name from ipinfo ("Amsterdam", "AS9009 M247 Europe SRL"), or None.
fn clean_name(s: Option<String>) -> Option<String> {
    let t = s?.trim().to_string();
    (!t.is_empty() && t.chars().count() <= 80 && !t.chars().any(char::is_control)).then_some(t)
}

use super::feeds::kind;

/// Decides whether a public-IP lookup may run now.
#[derive(Debug, Default)]
pub struct IpGate {
    pub vpn_active: bool,
    pub last: Option<Instant>,
}

const IP_MIN_GAP: Duration = Duration::from_secs(60);

impl IpGate {
    /// True only with the setting on, a VPN active and 60 s since the last lookup that ran.
    pub fn should_lookup(&mut self, enabled: bool, now: Instant) -> bool {
        if !enabled || !self.vpn_active {
            return false;
        }
        if self
            .last
            .is_some_and(|t| now.saturating_duration_since(t) < IP_MIN_GAP)
        {
            return false;
        }
        self.last = Some(now);
        true
    }
}

/// Weather refresh period.
pub const WEATHER_EVERY: Duration = Duration::from_secs(30 * 60);
/// Quiet time after a network change before the public-IP lookup runs.
pub const SETTLE: Duration = Duration::from_secs(5);
/// Waits before retrying a failed weather fetch (the network is often not up yet at login);
/// after the last step the regular period applies. A success starts over.
pub const WEATHER_RETRY: [Duration; 4] = [
    Duration::from_secs(30),
    Duration::from_secs(60),
    Duration::from_secs(120),
    Duration::from_secs(300),
];

/// Outcome of one weather fetch.
enum Fetch {
    Ok,
    Failed,
    Stopping,
}

/// The bridge's network features: the weather loop and the gated public-IP lookup. Background
/// work stops on `stop`, including while blocked in a send.
pub struct Net {
    client: NetClient,
    tx: mpsc::Sender<Outbound>,
    settings: watch::Receiver<Option<Settings>>,
    stop: CancellationToken,
    settle: Duration,
    retry: Vec<Duration>,
    /// The last weather fetch failed; a network change then triggers a refetch.
    wx_failed: AtomicBool,
    wx_kick: Notify,
    ip: Mutex<IpState>,
}

#[derive(Default)]
struct IpState {
    gate: IpGate,
    pending: Option<tokio::task::JoinHandle<()>>,
}

impl Net {
    pub fn new(
        client: NetClient,
        tx: mpsc::Sender<Outbound>,
        settings: watch::Receiver<Option<Settings>>,
        stop: CancellationToken,
        settle: Duration,
    ) -> Arc<Net> {
        Net::new_with_retry(client, tx, settings, stop, settle, WEATHER_RETRY.to_vec())
    }

    pub fn new_with_retry(
        client: NetClient,
        tx: mpsc::Sender<Outbound>,
        settings: watch::Receiver<Option<Settings>>,
        stop: CancellationToken,
        settle: Duration,
        retry: Vec<Duration>,
    ) -> Arc<Net> {
        Arc::new(Net {
            retry,
            wx_failed: AtomicBool::new(false),
            wx_kick: Notify::new(),
            client,
            tx,
            settings,
            stop,
            settle,
            ip: Mutex::new(IpState::default()),
        })
    }

    pub async fn geocode(&self, name: &str) -> Result<Value, String> {
        let hits = self.client.geocode(name).await.map_err(|e| e.to_string())?;
        Ok(json!(
            hits.iter()
                .map(|h| json!({"name": h.name, "country": h.country, "lat": h.lat, "lon": h.lon}))
                .collect::<Vec<_>>()
        ))
    }

    /// Send an event; false when the bridge is stopping or the writer is gone.
    async fn emit(&self, event: &str, data: Value) -> bool {
        let out = Outbound::Event {
            event: event.into(),
            data,
        };
        tokio::select! { _ = self.stop.cancelled() => false, r = self.tx.send(out) => r.is_ok() }
    }

    /// Weather at start, when the location changes, and every `period`. A failed fetch emits
    /// nothing (the shell keeps the last value) and is retried after `self.retry` steps, capped
    /// at `period`; a network change after a failure retries at once.
    pub async fn weather_loop(self: Arc<Self>, period: Duration) {
        let mut rx = self.settings.clone();
        let mut current: Option<Location> = None;
        let mut next: Option<tokio::time::Instant> = None;
        let mut failures = 0usize;
        loop {
            // Only while the Weather island is on the bar: nothing else shows the weather.
            let want = rx.borrow_and_update().as_ref().and_then(|s| {
                s.shell
                    .bar
                    .as_ref()
                    .is_some_and(|b| b.has("weather"))
                    .then(|| s.weather_location.clone())
                    .flatten()
            });
            if want != current {
                let had = current.is_some();
                current = want;
                match &current {
                    Some(l) => {
                        if !self.refresh(l, period, &mut failures, &mut next).await {
                            return;
                        }
                    }
                    None => {
                        next = None;
                        failures = 0;
                        self.wx_failed.store(false, Ordering::SeqCst);
                        if had && !self.emit("Weather", Value::Null).await {
                            return;
                        }
                    }
                }
            }
            tokio::select! {
                _ = self.stop.cancelled() => return,
                r = rx.changed() => if r.is_err() { return },
                _ = self.wx_kick.notified() => {
                    if self.wx_failed.load(Ordering::SeqCst)
                        && let Some(l) = &current
                        && !self.refresh(l, period, &mut failures, &mut next).await
                    {
                        return;
                    }
                }
                _ = tokio::time::sleep_until(next.unwrap_or_else(tokio::time::Instant::now)), if next.is_some() => {
                    if let Some(l) = &current
                        && !self.refresh(l, period, &mut failures, &mut next).await
                    {
                        return;
                    }
                }
            }
        }
    }

    /// Fetch, then schedule the next one; false only when the bridge is stopping.
    async fn refresh(
        &self,
        loc: &Location,
        period: Duration,
        failures: &mut usize,
        next: &mut Option<tokio::time::Instant>,
    ) -> bool {
        let wait = match self.fetch_weather(loc).await {
            Fetch::Stopping => return false,
            Fetch::Ok => {
                *failures = 0;
                self.wx_failed.store(false, Ordering::SeqCst);
                period
            }
            Fetch::Failed => {
                let step = self.retry.get(*failures).copied().unwrap_or(period);
                *failures += 1;
                self.wx_failed.store(true, Ordering::SeqCst);
                step.min(period)
            }
        };
        *next = Some(tokio::time::Instant::now() + wait);
        true
    }

    async fn fetch_weather(&self, loc: &Location) -> Fetch {
        let got = tokio::select! { _ = self.stop.cancelled() => return Fetch::Stopping, r = self.client.weather(loc) => r };
        match got {
            Ok(w) => {
                let sent = self.emit("Weather", w.to_json()).await;
                if sent { Fetch::Ok } else { Fetch::Stopping }
            }
            Err(e) => {
                eprintln!("uqctl bridge: weather lookup failed: {e}");
                if self.stop.is_cancelled() {
                    Fetch::Stopping
                } else {
                    Fetch::Failed
                }
            }
        }
    }

    /// Turning the public-IP lookup off clears the shown country at once and drops any pending
    /// lookup.
    pub async fn privacy_loop(self: Arc<Self>) {
        let mut rx = self.settings.clone();
        let mut was = rx
            .borrow_and_update()
            .as_ref()
            .is_some_and(|s| s.privacy.public_ip_lookup);
        loop {
            tokio::select! {
                _ = self.stop.cancelled() => return,
                r = rx.changed() => if r.is_err() { return },
            }
            let now = rx
                .borrow_and_update()
                .as_ref()
                .is_some_and(|s| s.privacy.public_ip_lookup);
            if was && !now {
                if let Some(h) = self.ip.lock().await.pending.take() {
                    h.abort();
                }
                if !self.emit("PublicIp", Value::Null).await {
                    return;
                }
            }
            was = now;
        }
    }

    /// The shell reported a network change. Without a VPN the country is cleared at once; with
    /// one, the lookup waits out the settle time (a newer change restarts it), then runs if the
    /// gate and the privacy setting allow.
    pub async fn network_changed(self: &Arc<Self>, vpn_active: bool) {
        if self.wx_failed.load(Ordering::SeqCst) {
            self.wx_kick.notify_one();
        }
        let mut st = self.ip.lock().await;
        st.gate.vpn_active = vpn_active;
        if let Some(h) = st.pending.take() {
            h.abort();
        }
        if !vpn_active {
            drop(st);
            self.emit("PublicIp", Value::Null).await;
            return;
        }
        let me = self.clone();
        st.pending = Some(tokio::spawn(async move {
            tokio::select! { _ = me.stop.cancelled() => return, _ = tokio::time::sleep(me.settle) => {} }
            let enabled = me
                .settings
                .borrow()
                .as_ref()
                .is_some_and(|s| s.privacy.public_ip_lookup);
            let allowed = {
                let mut st = me.ip.lock().await;
                st.gate.should_lookup(enabled, Instant::now())
            };
            if !allowed {
                return;
            }
            let got = tokio::select! { _ = me.stop.cancelled() => return, r = me.client.public_country() => r };
            match got {
                Ok(c) => {
                    me.emit("PublicIp", json!(c)).await;
                }
                Err(e) => eprintln!("uqctl bridge: public ip lookup failed: {e}"),
            }
        }));
    }
}

#[cfg(test)]
mod tests;
