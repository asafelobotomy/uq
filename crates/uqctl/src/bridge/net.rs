//! Network lookups (spec §2.11): Open-Meteo weather and geocoding, ipinfo.io country.
//! HTTPS only, 5 s timeout, nothing logged or persisted.

use super::protocol::Outbound;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, mpsc, watch};
use tokio_util::sync::CancellationToken;
use uq_core::settings::{Location, Settings, round2};

const TIMEOUT: Duration = Duration::from_secs(5);
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
        let http = reqwest::Client::builder()
            .user_agent(concat!("uq/", env!("CARGO_PKG_VERSION")))
            .timeout(TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("static reqwest client config");
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
        #[derive(Deserialize)]
        struct Resp {
            current: Cur,
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
                ],
            )
            .await?;
        Ok(Weather {
            temp_c: r.current.temperature_2m,
            code: r.current.weather_code,
            is_day: r.current.is_day != 0,
            location_name: loc.name.clone(),
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

fn kind(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connect"
    } else if e.is_decode() {
        "decode"
    } else {
        "other"
    }
}

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

/// The bridge's network features: the weather loop and the gated public-IP lookup. Background
/// work stops on `stop`, including while blocked in a send.
pub struct Net {
    client: NetClient,
    tx: mpsc::Sender<Outbound>,
    settings: watch::Receiver<Option<Settings>>,
    stop: CancellationToken,
    settle: Duration,
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
        Arc::new(Net {
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
    /// nothing, so the shell keeps the last value.
    pub async fn weather_loop(self: Arc<Self>, period: Duration) {
        let mut rx = self.settings.clone();
        let mut current: Option<Location> = None;
        let mut next: Option<tokio::time::Instant> = None;
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
                        if !self.fetch_weather(l).await {
                            return;
                        }
                        next = Some(tokio::time::Instant::now() + period);
                    }
                    None => {
                        next = None;
                        if had && !self.emit("Weather", Value::Null).await {
                            return;
                        }
                    }
                }
            }
            tokio::select! {
                _ = self.stop.cancelled() => return,
                r = rx.changed() => if r.is_err() { return },
                _ = tokio::time::sleep_until(next.unwrap_or_else(tokio::time::Instant::now)), if next.is_some() => {
                    if let Some(l) = &current
                        && !self.fetch_weather(l).await
                    {
                        return;
                    }
                    next = Some(tokio::time::Instant::now() + period);
                }
            }
        }
    }

    /// Fetch and emit; false only when the bridge is stopping.
    async fn fetch_weather(&self, loc: &Location) -> bool {
        let got = tokio::select! { _ = self.stop.cancelled() => return false, r = self.client.weather(loc) => r };
        match got {
            Ok(w) => {
                self.emit(
                    "Weather",
                    json!({"temp_c": w.temp_c, "code": w.code, "is_day": w.is_day, "location_name": w.location_name}),
                )
                .await
            }
            Err(e) => {
                eprintln!("uqctl bridge: weather lookup failed: {e}");
                !self.stop.is_cancelled()
            }
        }
    }

    /// The shell reported a network change. Without a VPN the country is cleared at once; with
    /// one, the lookup waits out the settle time (a newer change restarts it), then runs if the
    /// gate and the privacy setting allow.
    pub async fn network_changed(self: &Arc<Self>, vpn_active: bool) {
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
        let w = c.weather(&loc).await.unwrap();
        assert_eq!(
            w,
            Weather {
                temp_c: 12.5,
                code: 3,
                is_day: true,
                location_name: "London".into()
            }
        );
        let line = seen.lock().unwrap()[0].clone();
        assert!(line.contains("latitude=51.51"), "{line}");
        assert!(line.contains("longitude=-0.13"), "{line}");
        assert!(!line.contains("51.50735"), "{line}");
        assert!(line.contains("current=temperature_2m"), "{line}");
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
        assert_eq!(
            d,
            json!({"temp_c":7.0,"code":61,"is_day":false,"location_name":"Here"})
        );

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
}
