//! HTTP fetches for weather, places and the country: one client, bounded replies.

use super::*;

pub struct NetClient {
    pub(crate) http: reqwest::Client,
    pub(crate) weather_base: String,
    pub(crate) geo_base: String,
    pub(crate) ipinfo_base: String,
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

    pub(crate) fn build(weather: &str, geo: &str, ipinfo: &str) -> NetClient {
        let http = super::super::feeds::client(reqwest::redirect::Policy::none());
        NetClient {
            http,
            weather_base: weather.into(),
            geo_base: geo.into(),
            ipinfo_base: ipinfo.into(),
        }
    }

    pub(crate) async fn get<T: serde::de::DeserializeOwned>(
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

    pub(crate) async fn weather_at(&self, loc: &Location, now: i64) -> Result<Weather, NetError> {
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

    /// The caller's country, and the public address, city, region and provider (see
    /// [`Country`]). ipinfo's plain endpoint gives only the ISO code in `country`. A malformed
    /// address or name is dropped, not an error.
    pub async fn public_country(&self) -> Result<Country, NetError> {
        #[derive(Deserialize)]
        struct Resp {
            country: String,
            #[serde(default)]
            ip: Option<String>,
            #[serde(default)]
            city: Option<String>,
            #[serde(default)]
            region: Option<String>,
            #[serde(default)]
            org: Option<String>,
        }
        let r: Resp = self.get(&self.ipinfo_base, &[]).await?;
        let code = r.country.trim().to_ascii_uppercase();
        if code.len() != 2 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(NetError::Request("decode"));
        }
        let ip =
            r.ip.and_then(|a| a.trim().parse::<std::net::IpAddr>().ok())
                .map(|a| a.to_string());
        Ok(Country {
            country_code: code.clone(),
            country: code,
            ip,
            city: clean_name(r.city),
            region: clean_name(r.region),
            org: clean_name(r.org),
        })
    }
}
