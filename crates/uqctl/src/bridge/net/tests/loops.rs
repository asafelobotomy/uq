//! The weather and public-IP loops: gating, retries and backoff.

use super::*;

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

/// A city alone asks nothing: the Weather island must be on the bar. Taking it off clears
/// the shown weather and stops the lookups.
#[tokio::test]
async fn weather_only_while_its_island_is_placed() {
    let (base, seen) = fake(WX).await;
    let (tx, mut rx) = mpsc::channel(16);
    let mut off = settings(Some(("Here", 1.0)), false).unwrap();
    off.shell
        .bar
        .as_mut()
        .unwrap()
        .end
        .retain(|i| i.id != "weather");
    let (stx, srx) = watch::channel(Some(off.clone()));
    let stop = CancellationToken::new();
    let net = Net::new(
        NetClient::with_bases(&base, &base, &base),
        tx,
        srx,
        stop.clone(),
        SETTLE,
    );
    let task = tokio::spawn(net.weather_loop(Duration::from_secs(60)));
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(rx.try_recv().is_err());
    assert!(
        seen.lock().unwrap().is_empty(),
        "no request without the island"
    );

    stx.send_replace(settings(Some(("Here", 1.0)), false));
    assert_eq!(event(&mut rx).await.0, "Weather");
    stx.send_replace(Some(off));
    assert_eq!(event(&mut rx).await, ("Weather".to_string(), Value::Null));
    stop.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
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
async fn network_changed_gates_and_emits_public_ip() {
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
    assert_eq!(
        d,
        json!({"country_code":"NL","country":"NL","ip":"203.0.113.9","city":"X"})
    );
    net.network_changed(true).await;
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(seen.lock().unwrap().len(), 1);

    // VPN down clears at once, without waiting.
    net.network_changed(false).await;
    assert_eq!(event(&mut rx).await, ("PublicIp".into(), Value::Null));
    stop.cancel();
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
