//! Network lookups (spec §2.11): Open-Meteo weather and geocoding, ipinfo.io country.
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

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Country {
    pub country_code: String,
    pub country: String,
}

pub struct NetClient {
    http: reqwest::Client,
    weather_base: String,
    geo_base: String,
    ipinfo_base: String,
}

impl NetClient {
    /// The real HTTPS endpoints.
    pub fn new() -> NetClient {
        NetClient::build(
            "https://api.open-meteo.com/v1/forecast",
            "https://geocoding-api.open-meteo.com/v1/search",
            "https://ipinfo.io/json",
        )
    }

    /// Plain-HTTP bases for tests against a local fake; not constructible in release code.
    #[cfg(test)]
    pub fn with_bases(weather: &str, geo: &str, ipinfo: &str) -> NetClient {
        NetClient::build(weather, geo, ipinfo)
    }

    fn build(weather: &str, geo: &str, ipinfo: &str) -> NetClient {
        let http = super::feeds::client(reqwest::redirect::Policy::none());
        NetClient {
            http,
            weather_base: weather.into(),
            geo_base: geo.into(),
            ipinfo_base: ipinfo.into(),
        }
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<T, NetError> {
        let resp = self
            .http
            .get(url)
            .query(query)
            .send()
            .await
            .map_err(|e| NetError::Request(kind(&e)))?;
        if !resp.status().is_success() {
            return Err(NetError::Request("status"));
        }
        resp.json().await.map_err(|e| NetError::Request(kind(&e)))
    }

    /// Up to five matches for a city name, coordinates rounded to 2 dp.
    pub async fn geocode(&self, name: &str) -> Result<Vec<GeoHit>, NetError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(NetError::BadInput("name is empty"));
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(NetError::BadInput("name over 100 characters"));
        }
        #[derive(Deserialize)]
        struct Resp {
            #[serde(default)]
            results: Vec<Hit>,
        }
        #[derive(Deserialize)]
        struct Hit {
            name: String,
            #[serde(default)]
            country: String,
            latitude: f64,
            longitude: f64,
        }
        let r: Resp = self
            .get(
                &self.geo_base,
                &[
                    ("name", name),
                    ("count", "5"),
                    ("language", "en"),
                    ("format", "json"),
                ],
            )
            .await?;
        Ok(r.results
            .into_iter()
            .take(5)
            .map(|h| GeoHit {
                name: h.name,
                country: h.country,
                lat: round2(h.latitude),
                lon: round2(h.longitude),
            })
            .collect())
    }

    /// Current conditions for a location; coordinates are rounded to 2 dp before sending.
    pub async fn weather(&self, loc: &Location) -> Result<Weather, NetError> {
        self.weather_at(loc, chrono::Utc::now().timestamp()).await
    }

    async fn weather_at(&self, loc: &Location, now: i64) -> Result<Weather, NetError> {
        #[derive(Deserialize)]
        struct Resp {
            current: Cur,
            hourly: Option<Hourly>,
            daily: Option<Daily>,
        }
        #[derive(Deserialize)]
        struct Hourly {
            time: Vec<i64>,
            temperature_2m: Vec<Option<f64>>,
            weather_code: Vec<Option<u16>>,
        }
        #[derive(Deserialize)]
        struct Daily {
            temperature_2m_max: Vec<Option<f64>>,
            temperature_2m_min: Vec<Option<f64>>,
        }
        #[derive(Deserialize)]
        struct Cur {
            temperature_2m: f64,
            weather_code: u16,
            is_day: u8,
        }
        let lat = format!("{:.2}", round2(loc.lat));
        let lon = format!("{:.2}", round2(loc.lon));
        let r: Resp = self
            .get(
                &self.weather_base,
                &[
                    ("latitude", &lat),
                    ("longitude", &lon),
                    ("current", "temperature_2m,weather_code,is_day"),
                    ("hourly", "temperature_2m,weather_code"),
                    ("daily", "temperature_2m_max,temperature_2m_min"),
                    ("timezone", "auto"),
                    ("timeformat", "unixtime"),
                    ("forecast_days", "2"),
                ],
            )
            .await?;
        let today = r.daily.and_then(|d| {
            Some((
                (*d.temperature_2m_max.first()?)?,
                (*d.temperature_2m_min.first()?)?,
            ))
        });
        let hourly = r.hourly.map_or_else(Vec::new, |h| {
            next_hours(&h.time, &h.temperature_2m, &h.weather_code, now)
        });
        Ok(Weather {
            temp_c: r.current.temperature_2m,
            code: r.current.weather_code,
            is_day: r.current.is_day != 0,
            location_name: loc.name.clone(),
            fetched_at: now,
            today,
            hourly,
        })
    }

    /// The caller's country. ipinfo's plain endpoint gives only the ISO code in `country`; every
    /// other field (ip, city, loc, org...) is dropped at parse time.
    pub async fn public_country(&self) -> Result<Country, NetError> {
        #[derive(Deserialize)]
        struct Resp {
            country: String,
        }
        let r: Resp = self.get(&self.ipinfo_base, &[]).await?;
        let code = r.country.trim().to_ascii_uppercase();
        if code.len() != 2 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(NetError::Request("decode"));
        }
        Ok(Country {
            country_code: code.clone(),
            country: code,
        })
    }
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
            let want = rx
                .borrow_and_update()
                .as_ref()
                .and_then(|s| s.weather_location.clone());
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
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serves `body` (200, JSON) to every connection; returns the base URL and the request lines.
    async fn fake(body: &'static str) -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let s2 = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = l.accept().await else {
                    return;
                };
                let s3 = s2.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 1024];
                    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        match sock.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        }
                    }
                    let text = String::from_utf8_lossy(&buf).to_string();
                    s3.lock()
                        .unwrap()
                        .push(text.lines().next().unwrap().to_string());
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                });
            }
        });
        (base, seen)
    }

    #[tokio::test]
    async fn weather_rounds_coordinates() {
        let (base, seen) =
            fake(r#"{"current":{"temperature_2m":12.5,"weather_code":3,"is_day":1}}"#).await;
        let c = NetClient::with_bases(&base, &base, &base);
        let loc = Location {
            name: "London".into(),
            lat: 51.50735,
            lon: -0.12776,
        };
        let w = c.weather_at(&loc, 1000).await.unwrap();
        assert_eq!(
            w,
            Weather {
                temp_c: 12.5,
                code: 3,
                is_day: true,
                location_name: "London".into(),
                fetched_at: 1000,
                today: None,
                hourly: Vec::new(),
            }
        );
        let line = seen.lock().unwrap()[0].clone();
        assert!(line.contains("latitude=51.51"), "{line}");
        assert!(line.contains("longitude=-0.13"), "{line}");
        assert!(!line.contains("51.50735"), "{line}");
        assert!(line.contains("current=temperature_2m"), "{line}");
        for q in [
            "hourly=temperature_2m%2Cweather_code",
            "daily=temperature_2m_max%2Ctemperature_2m_min",
            "timezone=auto",
            "timeformat=unixtime",
            "forecast_days=2",
        ] {
            assert!(line.contains(q), "{q} in {line}");
        }
    }

    #[tokio::test]
    async fn weather_forecast_today_and_next_hours() {
        // 30 hours from t=0; the fetch happens at 7200 + 1800 (half past the third hour).
        let times: Vec<i64> = (0..30).map(|h| h * 3600).collect();
        let temps: Vec<Value> = (0..30)
            .map(|h| if h == 5 { Value::Null } else { json!(h as f64) })
            .collect();
        let codes: Vec<u16> = (0..30).map(|h| h as u16).collect();
        let body = json!({
            "current": {"temperature_2m": 1.0, "weather_code": 2, "is_day": 0},
            "hourly": {"time": times, "temperature_2m": temps, "weather_code": codes},
            "daily": {"time": [0, 86400], "temperature_2m_max": [9.5, 8.0], "temperature_2m_min": [-1.5, 0.0]}
        })
        .to_string();
        let (base, _) = fake(Box::leak(body.into_boxed_str())).await;
        let c = NetClient::with_bases(&base, &base, &base);
        let loc = Location {
            name: "X".into(),
            lat: 1.0,
            lon: 2.0,
        };
        let w = c.weather_at(&loc, 7200 + 1800).await.unwrap();
        assert_eq!(w.today, Some((9.5, -1.5)));
        let hours: Vec<i64> = w.hourly.iter().map(|h| h.0 / 3600).collect();
        // From the current hour (2), 12 entries, the one without a temperature left out.
        assert_eq!(hours, [2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
        let v = w.to_json();
        assert_eq!(v["today"], json!({"max_c": 9.5, "min_c": -1.5}));
        assert_eq!(
            v["hourly"][0],
            json!({"time": 7200, "temp_c": 2.0, "code": 2})
        );
        assert_eq!(v["fetched_at"], 9000);
    }

    #[tokio::test]
    async fn public_country_drops_other_fields() {
        let (base, _) = fake(
            r#"{"ip":"203.0.113.9","city":"Amsterdam","loc":"52.37,4.89","org":"AS1 Example","country":"NL","region":"NH"}"#,
        )
        .await;
        let c = NetClient::with_bases(&base, &base, &base);
        let got = c.public_country().await.unwrap();
        // ipinfo's `country` is the ISO code; the display name comes from the code.
        assert_eq!(got.country_code, "NL");
        let v = serde_json::to_value(&got).unwrap();
        let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["country", "country_code"]);
        let s = v.to_string();
        for leak in ["203.0.113.9", "Amsterdam", "52.37", "Example"] {
            assert!(!s.contains(leak), "{s}");
        }
    }

    #[test]
    fn ip_gate() {
        let t0 = Instant::now();
        let mut g = IpGate::default();
        assert!(!g.should_lookup(true, t0), "no VPN");
        g.vpn_active = true;
        assert!(!g.should_lookup(false, t0), "setting off");
        assert!(g.should_lookup(true, t0));
        assert!(!g.should_lookup(true, t0 + Duration::from_secs(59)));
        assert!(g.should_lookup(true, t0 + Duration::from_secs(60)));
    }

    #[tokio::test]
    async fn geocode_name_too_long_rejected() {
        let c = NetClient::with_bases(
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
        );
        assert!(matches!(
            c.geocode(&"x".repeat(101)).await,
            Err(NetError::BadInput(_))
        ));
        assert!(matches!(c.geocode("  ").await, Err(NetError::BadInput(_))));
    }

    #[tokio::test]
    async fn geocode_caps_and_rounds() {
        let (base, seen) = fake(
            r#"{"results":[
            {"name":"A","country":"X","latitude":1.005,"longitude":2.004},
            {"name":"B","country":"X","latitude":1,"longitude":2},
            {"name":"C","country":"X","latitude":1,"longitude":2},
            {"name":"D","country":"X","latitude":1,"longitude":2},
            {"name":"E","country":"X","latitude":1,"longitude":2},
            {"name":"F","country":"X","latitude":1,"longitude":2}]}"#,
        )
        .await;
        let c = NetClient::with_bases(&base, &base, &base);
        let hits = c.geocode("Springfield").await.unwrap();
        assert_eq!(hits.len(), 5);
        assert_eq!(hits[0].lon, 2.0);
        assert!(seen.lock().unwrap()[0].contains("count=5"));
    }

    #[test]
    fn http_base_rejected_outside_tests() {
        let c = NetClient::new();
        for b in [&c.weather_base, &c.geo_base, &c.ipinfo_base] {
            assert!(b.starts_with("https://"), "{b}");
        }
        assert_eq!(c.weather_base, "https://api.open-meteo.com/v1/forecast");
        assert_eq!(c.geo_base, "https://geocoding-api.open-meteo.com/v1/search");
        assert_eq!(c.ipinfo_base, "https://ipinfo.io/json");
    }

    fn settings(loc: Option<(&str, f64)>, ip: bool) -> Option<Settings> {
        let mut s = Settings {
            weather_location: loc.map(|(n, lat)| Location {
                name: n.into(),
                lat,
                lon: 0.5,
            }),
            ..Settings::default()
        };
        s.privacy.public_ip_lookup = ip;
        Some(s)
    }

    async fn event(rx: &mut mpsc::Receiver<Outbound>) -> (String, Value) {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            Outbound::Event { event, data } => (event, data),
            _ => panic!("expected event"),
        }
    }

    const WX: &str = r#"{"current":{"temperature_2m":7.0,"weather_code":61,"is_day":0}}"#;

    #[tokio::test]
    async fn weather_loop_start_change_interval_and_failure() {
        let (base, seen) = fake(WX).await;
        let (tx, mut rx) = mpsc::channel(16);
        let (stx, srx) = watch::channel(None);
        let stop = CancellationToken::new();
        let net = Net::new(
            NetClient::with_bases(&base, &base, &base),
            tx,
            srx,
            stop.clone(),
            SETTLE,
        );
        let task = tokio::spawn(net.weather_loop(Duration::from_millis(300)));

        // No location: nothing happens.
        stx.send_replace(settings(None, false));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(rx.try_recv().is_err());

        stx.send_replace(settings(Some(("Here", 1.0)), false));
        let (e, d) = event(&mut rx).await;
        assert_eq!(e, "Weather");
        assert!(d["fetched_at"].is_i64());
        for (k, v) in [
            ("temp_c", json!(7.0)),
            ("code", json!(61)),
            ("is_day", json!(false)),
            ("location_name", json!("Here")),
            ("today", Value::Null),
            ("hourly", json!([])),
        ] {
            assert_eq!(d[k], v, "{k}");
        }

        // An unrelated settings change does not refetch; the interval does.
        stx.send_replace(settings(Some(("Here", 1.0)), true));
        assert_eq!(seen.lock().unwrap().len(), 1);
        event(&mut rx).await;
        assert_eq!(seen.lock().unwrap().len(), 2);

        // A changed location refetches at once, well before the next tick.
        stx.send_replace(settings(Some(("There", 2.0)), true));
        let (_, d) = event(&mut rx).await;
        assert_eq!(d["location_name"], "There");
        assert!(
            seen.lock()
                .unwrap()
                .last()
                .unwrap()
                .contains("latitude=2.00")
        );

        // Clearing the location clears the shown weather.
        stx.send_replace(settings(None, true));
        assert_eq!(event(&mut rx).await, ("Weather".into(), Value::Null));

        stop.cancel();
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn weather_failure_emits_nothing() {
        let (tx, mut rx) = mpsc::channel(16);
        let (stx, srx) = watch::channel(settings(Some(("Here", 1.0)), false));
        let stop = CancellationToken::new();
        let dead = "http://127.0.0.1:1";
        let net = Net::new(
            NetClient::with_bases(dead, dead, dead),
            tx,
            srx,
            stop.clone(),
            SETTLE,
        );
        let task = tokio::spawn(net.weather_loop(Duration::from_millis(100)));
        tokio::time::sleep(Duration::from_millis(350)).await;
        assert!(rx.try_recv().is_err());
        drop(stx);
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn network_changed_gates_and_emits_country_only() {
        let (base, seen) = fake(r#"{"ip":"203.0.113.9","city":"X","country":"NL"}"#).await;
        let (tx, mut rx) = mpsc::channel(16);
        let (stx, srx) = watch::channel(settings(None, false));
        let stop = CancellationToken::new();
        let net = Net::new(
            NetClient::with_bases(&base, &base, &base),
            tx,
            srx,
            stop.clone(),
            Duration::from_millis(50),
        );

        // Setting off: VPN up produces no lookup.
        net.network_changed(true).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(seen.lock().unwrap().is_empty());
        assert!(rx.try_recv().is_err());

        // Setting on: one lookup after the settle; an immediate repeat is rate limited.
        stx.send_replace(settings(None, true));
        net.network_changed(true).await;
        assert!(rx.try_recv().is_err(), "waits for the settle");
        let (e, d) = event(&mut rx).await;
        assert_eq!(e, "PublicIp");
        assert_eq!(d, json!({"country_code":"NL","country":"NL"}));
        net.network_changed(true).await;
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert_eq!(seen.lock().unwrap().len(), 1);

        // VPN down clears at once, without waiting.
        net.network_changed(false).await;
        assert_eq!(event(&mut rx).await, ("PublicIp".into(), Value::Null));
        stop.cancel();
    }

    /// Serves WX (200) except for the 1-based request numbers in `fail` (500); records the time
    /// of each request.
    async fn scripted(fail: &'static [usize]) -> (String, Arc<std::sync::Mutex<Vec<Instant>>>) {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let times = Arc::new(std::sync::Mutex::new(Vec::new()));
        let t2 = times.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = l.accept().await else {
                    return;
                };
                let n = {
                    let mut t = t2.lock().unwrap();
                    t.push(Instant::now());
                    t.len()
                };
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 1024];
                    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        match sock.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(k) => buf.extend_from_slice(&chunk[..k]),
                        }
                    }
                    let resp = if fail.contains(&n) {
                        "HTTP/1.1 500 Err\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_string()
                    } else {
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{WX}",
                            WX.len()
                        )
                    };
                    let _ = sock.write_all(resp.as_bytes()).await;
                });
            }
        });
        (base, times)
    }

    fn ms(a: Instant, b: Instant) -> u128 {
        b.duration_since(a).as_millis()
    }

    #[tokio::test]
    async fn weather_retries_with_backoff_and_success_resets() {
        // Requests 1-3 fail, 4 succeeds, 5 fails, 6 succeeds.
        let (base, times) = scripted(&[1, 2, 3, 5]).await;
        let (tx, mut rx) = mpsc::channel(16);
        let (stx, srx) = watch::channel(settings(Some(("Here", 1.0)), false));
        let stop = CancellationToken::new();
        let net = Net::new_with_retry(
            NetClient::with_bases(&base, &base, &base),
            tx,
            srx,
            stop.clone(),
            SETTLE,
            vec![
                Duration::from_millis(50),
                Duration::from_millis(100),
                Duration::from_millis(200),
            ],
        );
        let task = tokio::spawn(net.weather_loop(Duration::from_millis(500)));
        event(&mut rx).await; // request 4's weather
        event(&mut rx).await; // request 6's weather
        let t = times.lock().unwrap().clone();
        assert!(t.len() >= 6, "{}", t.len());
        let gaps: Vec<u128> = t.windows(2).take(5).map(|w| ms(w[0], w[1])).collect();
        // 50, 100, 200 backoff, then the regular period, then the reset first step (50).
        assert!((45..95).contains(&gaps[0]), "{gaps:?}");
        assert!((95..190).contains(&gaps[1]), "{gaps:?}");
        assert!((195..320).contains(&gaps[2]), "{gaps:?}");
        assert!((495..650).contains(&gaps[3]), "{gaps:?}");
        assert!((45..95).contains(&gaps[4]), "{gaps:?}");
        stop.cancel();
        drop(stx);
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn weather_backoff_caps_at_the_period() {
        let (base, times) = scripted(&[1, 2, 3, 4]).await;
        let (tx, _rx) = mpsc::channel(16);
        let (_stx, srx) = watch::channel(settings(Some(("Here", 1.0)), false));
        let stop = CancellationToken::new();
        let net = Net::new_with_retry(
            NetClient::with_bases(&base, &base, &base),
            tx,
            srx,
            stop.clone(),
            SETTLE,
            vec![Duration::from_millis(400)],
        );
        let task = tokio::spawn(net.weather_loop(Duration::from_millis(100)));
        tokio::time::sleep(Duration::from_millis(450)).await;
        stop.cancel();
        let _ = task.await;
        // A step longer than the period is capped: retries every 100 ms, not 400.
        assert!(times.lock().unwrap().len() >= 4);
    }

    #[tokio::test]
    async fn network_changed_after_failed_weather_refetches() {
        let (base, times) = scripted(&[1]).await;
        let (tx, mut rx) = mpsc::channel(16);
        let (_stx, srx) = watch::channel(settings(Some(("Here", 1.0)), false));
        let stop = CancellationToken::new();
        let net = Net::new_with_retry(
            NetClient::with_bases(&base, &base, &base),
            tx,
            srx,
            stop.clone(),
            SETTLE,
            vec![Duration::from_secs(60)],
        );
        let task = tokio::spawn(net.clone().weather_loop(Duration::from_secs(600)));
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(times.lock().unwrap().len(), 1);
        net.network_changed(false).await;
        let mut got = Vec::new();
        got.push(event(&mut rx).await);
        got.push(event(&mut rx).await);
        assert!(got.iter().any(|(e, _)| e == "Weather"), "{got:?}");
        assert_eq!(times.lock().unwrap().len(), 2);
        // With the last fetch good, a further change does not refetch.
        net.network_changed(false).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(times.lock().unwrap().len(), 2);
        stop.cancel();
        let _ = task.await;
    }

    #[tokio::test]
    async fn ip_lookup_turned_off_clears_country_and_aborts_pending() {
        let (base, seen) = fake(r#"{"ip":"203.0.113.9","country":"NL"}"#).await;
        let (tx, mut rx) = mpsc::channel(16);
        let (stx, srx) = watch::channel(settings(None, true));
        let stop = CancellationToken::new();
        let net = Net::new(
            NetClient::with_bases(&base, &base, &base),
            tx,
            srx,
            stop.clone(),
            Duration::from_millis(300),
        );
        let task = tokio::spawn(net.clone().privacy_loop());
        // A lookup is pending (settle not elapsed) when the setting goes off.
        net.network_changed(true).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        stx.send_replace(settings(None, false));
        assert_eq!(event(&mut rx).await, ("PublicIp".into(), Value::Null));
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(seen.lock().unwrap().is_empty(), "pending lookup aborted");
        assert!(rx.try_recv().is_err());
        // Unrelated or off-to-on changes emit nothing.
        stx.send_replace(settings(Some(("X", 1.0)), false));
        stx.send_replace(settings(Some(("X", 1.0)), true));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(rx.try_recv().is_err());
        stop.cancel();
        let _ = task.await;
    }
}
