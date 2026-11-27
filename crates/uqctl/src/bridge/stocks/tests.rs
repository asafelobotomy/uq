use super::super::feeds::test_http::*;
use super::super::protocol::Outbound;
use super::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use uq_core::settings::BarItem;

fn chart(sym: &str, price: f64) -> String {
    format!(
        r#"{{"chart":{{"result":[{{"meta":{{"currency":"USD","symbol":"{sym}","regularMarketPrice":{price},
        "chartPreviousClose":100.0,"regularMarketDayHigh":{price},"regularMarketDayLow":99.0,
        "regularMarketTime":1763000000,"shortName":"{sym} Inc.","longName":"{sym} Incorporated"}}}}],"error":null}}}}"#
    )
}

#[test]
fn parses_meta() {
    let q = parse_chart(chart("AAPL", 101.5).as_bytes(), "AAPL").unwrap();
    assert_eq!(
        q,
        json!({"symbol":"AAPL","name":"AAPL Inc.","price":101.5,"change":1.5,"change_pct":1.5,
            "currency":"USD","day_high":101.5,"day_low":99.0,"market_time":1763000000,"ok":true})
    );
}

#[test]
fn parses_meta_with_missing_fields() {
    // Only previousClose and longName; no range, time or currency.
    let body = br#"{"chart":{"result":[{"meta":{"regularMarketPrice":50,"previousClose":40,"longName":"Long Co"}}]}}"#;
    let q = parse_chart(body, "X").unwrap();
    assert_eq!(q["name"], "Long Co");
    assert_eq!(q["change"], 10.0);
    assert_eq!(q["change_pct"], 25.0);
    assert!(q["currency"].is_null() && q["day_high"].is_null() && q["market_time"].is_null());
    // No previous close: no change; no name: the symbol.
    let q = parse_chart(
        br#"{"chart":{"result":[{"meta":{"regularMarketPrice":5}}]}}"#,
        "Y",
    )
    .unwrap();
    assert!(q["change"].is_null() && q["change_pct"].is_null());
    assert_eq!(q["name"], "Y");
    // No price, unknown symbol, junk.
    for bad in [
        &br#"{"chart":{"result":[{"meta":{"currency":"USD"}}]}}"#[..],
        br#"{"chart":{"result":null,"error":{"code":"Not Found"}}}"#,
        br#"<html>"#,
    ] {
        assert!(parse_chart(bad, "Z").is_err());
    }
}

#[test]
fn symbols_are_encoded() {
    assert_eq!(encode("^GSPC"), "%5EGSPC");
    assert_eq!(encode("BTC-USD"), "BTC-USD");
    assert_eq!(encode("EURUSD=X"), "EURUSD%3DX");
    let s = Stocks::new();
    assert_eq!(
        s.url("^GSPC"),
        "https://query1.finance.yahoo.com/v8/finance/chart/%5EGSPC?range=1d&interval=1d"
    );
}

pub(crate) fn settings_with(placed: bool, symbols: &[&str]) -> Option<Settings> {
    let mut s = Settings::default();
    let bar = s.shell.bar.as_mut().unwrap();
    if placed {
        bar.end.push(BarItem::new("stocks"));
    }
    bar.options.stocks.symbols = symbols.iter().map(|s| s.to_string()).collect();
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

fn make(
    base: &str,
    srx: watch::Receiver<Option<Settings>>,
    cache: Option<std::path::PathBuf>,
    period: Duration,
    retry: Vec<Duration>,
) -> (Arc<StocksLoop>, mpsc::Receiver<Outbound>, CancellationToken) {
    let (tx, rx) = mpsc::channel(16);
    let stop = CancellationToken::new();
    let l = FeedLoop::new(
        Stocks::with_base(&format!("{base}/chart/")),
        tx,
        srx,
        stop.clone(),
        period,
        retry,
        cache,
        Duration::from_millis(200),
    );
    (l, rx, stop)
}

#[tokio::test]
async fn gated_on_island_partial_order_and_removal() {
    let (base, seen) = serve(
        routes(&[
            ("/chart/AAPL", Reply::ok(chart("AAPL", 110.0))),
            ("/chart/%5EGSPC", Reply::ok(chart("^GSPC", 90.0))),
        ]),
        |_| None,
    )
    .await;
    let (stx, srx) = watch::channel(settings_with(false, &["^GSPC", "NOPE", "AAPL"]));
    let (l, mut rx, stop) = make(&base, srx, None, Duration::from_secs(600), vec![]);
    let task = tokio::spawn(l.clone().run());
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        seen.lock().unwrap().is_empty(),
        "no request while not placed"
    );
    assert!(rx.try_recv().is_err());
    assert!(!l.refresh_now(), "no manual refresh while not placed");

    stx.send_replace(settings_with(true, &["^GSPC", "NOPE", "AAPL"]));
    let (e, d) = event(&mut rx).await;
    assert_eq!(e, "Stocks");
    assert_eq!(d["status"], "partial");
    let syms: Vec<_> = d["quotes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| (q["symbol"].as_str().unwrap(), q["ok"].as_bool().unwrap()))
        .collect();
    assert_eq!(syms, [("^GSPC", true), ("NOPE", false), ("AAPL", true)]);
    assert_eq!(d["quotes"][0]["change_pct"], -10.0);
    assert!(d["fetched_at"].is_i64());
    {
        let s = seen.lock().unwrap();
        assert_eq!(s.len(), 3);
        assert!(s.iter().all(|(_, _, ua)| ua.starts_with("uq/")));
        assert!(
            s.iter()
                .any(|(_, l, _)| l.contains("/chart/%5EGSPC?range=1d&interval=1d"))
        );
    }

    // Manual refresh: once, then rate limited.
    assert!(l.refresh_now());
    event(&mut rx).await;
    assert!(!l.refresh_now());
    assert_eq!(seen.lock().unwrap().len(), 6);

    // Removed: null once, and no more fetching.
    stx.send_replace(settings_with(false, &["^GSPC"]));
    assert_eq!(event(&mut rx).await, ("Stocks".into(), Value::Null));
    stx.send_replace(settings_with(false, &["AAPL"]));
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(rx.try_recv().is_err());
    assert_eq!(seen.lock().unwrap().len(), 6);
    stop.cancel();
    task.await.unwrap();
}

#[tokio::test]
async fn total_failure_keeps_last_values_and_backs_off() {
    // Request 1 good; then everything fails.
    let (base, seen) = serve(routes(&[("*", Reply::ok(chart("AAPL", 120.0)))]), |n| {
        (n > 1).then(|| Reply::status("500 Err"))
    })
    .await;
    let (_stx, srx) = watch::channel(settings_with(true, &["AAPL"]));
    let (l, mut rx, stop) = make(
        &base,
        srx,
        None,
        Duration::from_millis(100),
        vec![Duration::from_millis(200), Duration::from_millis(400)],
    );
    let task = tokio::spawn(l.clone().run());
    let (_, d) = event(&mut rx).await;
    assert_eq!(d["status"], "ok");
    let good_at = d["fetched_at"].clone();
    let (_, d) = event(&mut rx).await;
    assert_eq!(d["status"], "error");
    assert_eq!(d["fetched_at"], good_at, "as old as the last good data");
    assert_eq!(d["quotes"][0]["price"], 120.0);
    assert_eq!(d["quotes"][0]["ok"], false);
    event(&mut rx).await;
    event(&mut rx).await;
    let t: Vec<_> = seen.lock().unwrap().iter().map(|s| s.0).collect();
    let gaps: Vec<u128> = t
        .windows(2)
        .map(|w| w[1].duration_since(w[0]).as_millis())
        .collect();
    // Period 100 after the success, then steps 200 and 400 capped at the period.
    assert!((95..190).contains(&gaps[0]), "{gaps:?}");
    assert!((95..190).contains(&gaps[1]), "{gaps:?}");
    assert!((95..190).contains(&gaps[2]), "{gaps:?}");
    stop.cancel();
    task.await.unwrap();
}

#[tokio::test]
async fn backoff_steps_and_network_change() {
    let (base, seen) = serve(routes(&[("*", Reply::ok(chart("AAPL", 1.0)))]), |n| {
        (n <= 3).then(|| Reply::status("500 Err"))
    })
    .await;
    let (_stx, srx) = watch::channel(settings_with(true, &["AAPL"]));
    let (l, mut rx, stop) = make(
        &base,
        srx,
        None,
        Duration::from_secs(600),
        vec![Duration::from_millis(50), Duration::from_secs(60)],
    );
    let task = tokio::spawn(l.clone().run());
    assert_eq!(event(&mut rx).await.1["status"], "error");
    assert!(event(&mut rx).await.1["quotes"][0]["price"].is_null());
    let t = seen.lock().unwrap().clone();
    let gap = t[1].0.duration_since(t[0].0).as_millis();
    assert!((45..120).contains(&gap), "{gap}");
    // Second step is 60 s; a network change retries at once.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(seen.lock().unwrap().len(), 2);
    l.network_changed();
    assert_eq!(event(&mut rx).await.1["status"], "error");
    l.network_changed();
    assert_eq!(event(&mut rx).await.1["status"], "ok");
    // After a success a network change does nothing.
    l.network_changed();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(seen.lock().unwrap().len(), 4);
    stop.cancel();
    task.await.unwrap();
}

#[tokio::test]
async fn cache_round_trip_and_stale_emit() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("uq/feeds");
    let (base, _) = serve(routes(&[("*", Reply::ok(chart("AAPL", 7.0)))]), |_| None).await;
    let (_stx, srx) = watch::channel(settings_with(true, &["AAPL"]));
    let (l, mut rx, stop) = make(
        &base,
        srx,
        Some(cache.clone()),
        Duration::from_secs(600),
        vec![],
    );
    let task = tokio::spawn(l.run());
    let (_, fresh) = event(&mut rx).await;
    assert_eq!(fresh["status"], "ok");
    stop.cancel();
    task.await.unwrap();
    let file = cache.join("stocks.json");
    let stored: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(stored["key"], json!(["AAPL"]));
    assert_eq!(stored["payload"], fresh);

    // A new start with the same symbols shows the cache at once as stale, then fetches;
    // with no server the fetch fails and keeps the cached values.
    let (_stx, srx) = watch::channel(settings_with(true, &["AAPL"]));
    let (l, mut rx, stop) = make(
        "http://127.0.0.1:1",
        srx,
        Some(cache.clone()),
        Duration::from_secs(600),
        vec![],
    );
    let task = tokio::spawn(l.run());
    let (_, d) = event(&mut rx).await;
    assert_eq!(d["status"], "stale");
    assert_eq!(d["quotes"], fresh["quotes"]);
    assert_eq!(d["fetched_at"], fresh["fetched_at"]);
    let (_, d) = event(&mut rx).await;
    assert_eq!(d["status"], "error");
    assert_eq!(d["quotes"][0]["price"], 7.0);
    assert_eq!(d["fetched_at"], fresh["fetched_at"]);
    stop.cancel();
    task.await.unwrap();

    // Other symbols: the cache doesn't apply.
    let (_stx, srx) = watch::channel(settings_with(true, &["MSFT"]));
    let (l, mut rx, stop) = make(
        "http://127.0.0.1:1",
        srx,
        Some(cache),
        Duration::from_secs(600),
        vec![],
    );
    let task = tokio::spawn(l.run());
    assert_eq!(event(&mut rx).await.1["status"], "error");
    stop.cancel();
    task.await.unwrap();
}
