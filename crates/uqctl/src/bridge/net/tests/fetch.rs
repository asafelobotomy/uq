//! Fetching weather, places and the country.

use super::*;

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
async fn public_country_keeps_only_what_the_vpn_details_show() {
    let (base, _) = fake(
        r#"{"ip":"203.0.113.9","hostname":"x.example","city":"Amsterdam","loc":"52.37,4.89","org":"AS1 Example","postal":"1012","timezone":"Europe/Amsterdam","country":"NL","region":"North Holland"}"#,
    )
    .await;
    let c = NetClient::with_bases(&base, &base, &base);
    let got = c.public_country().await.unwrap();
    // ipinfo's `country` is the ISO code; the display name comes from the code.
    assert_eq!(got.country_code, "NL");
    let v = serde_json::to_value(&got).unwrap();
    let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        ["city", "country", "country_code", "ip", "org", "region"]
    );
    assert_eq!(v["ip"], "203.0.113.9");
    assert_eq!(v["city"], "Amsterdam");
    let s = v.to_string();
    for leak in ["52.37", "x.example", "1012", "Europe/Amsterdam"] {
        assert!(!s.contains(leak), "{s}");
    }
}

#[tokio::test]
async fn public_country_drops_bad_details() {
    let (base, _) =
        fake(r#"{"ip":"not an ip","city":"bad\u0007name","org":"","country":"de"}"#).await;
    let c = NetClient::with_bases(&base, &base, &base);
    let got = c.public_country().await.unwrap();
    assert_eq!(got.country_code, "DE");
    assert_eq!((got.ip, got.city, got.org), (None, None, None));
    let v = serde_json::to_value(c.public_country().await.unwrap()).unwrap();
    assert_eq!(v, json!({"country_code": "DE", "country": "DE"}));
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
