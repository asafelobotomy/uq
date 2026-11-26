//! Defaults, strict and lenient loading, edits and the helpers.

use super::*;

#[test]
fn default_is_todays_bar() {
    let b = BarLayout::default();
    assert_eq!(ids(&b.start), ["system", "work"]);
    assert!(b.center.is_empty());
    assert_eq!(
        ids(&b.end),
        ["clock", "notif", "security", "tray", "status"]
    );
    assert!(b.validate().is_ok());
    assert_eq!(b.options.stocks.symbols, ["^GSPC", "AAPL", "MSFT"]);
}

#[test]
fn crumbs_island_and_new_island_options() {
    assert!(known_island("crumbs"));
    assert_eq!(default_size("crumbs"), Size::Small);
    let o = BarOptions::default();
    assert_eq!(
        (o.sysmon.cpu, o.sysmon.mem, o.sysmon.gpu, o.sysmon.temp),
        (true, true, true, true)
    );
    assert_eq!((o.crumbs.warn_gb, o.security.notify), (5, true));
    let b = BarLayout::from_value(&json!({
        "end": [{"id": "crumbs"}],
        "options": {"sysmon": {"gpu": false}, "crumbs": {"warn_gb": 0},
                    "security": {"notify": false}}
    }))
    .unwrap();
    assert!(!b.options.sysmon.gpu && b.options.sysmon.cpu);
    assert_eq!(
        (b.options.crumbs.warn_gb, b.options.security.notify),
        (0, false)
    );
    for bad in [
        json!({"options": {"crumbs": {"warn_gb": 501}}}),
        json!({"options": {"sysmon": {"fans": true}}}),
        json!({"options": {"security": {"notify": "yes"}}}),
    ] {
        assert!(BarLayout::from_value(&bad).is_err(), "{bad}");
    }
    let mut b = BarLayout::default();
    b.apply(&BarEdit::Options {
        island: "sysmon".into(),
        value: json!({"cpu": true, "mem": false, "gpu": true, "temp": true}),
    })
    .unwrap();
    assert!(!b.options.sysmon.mem);
}

#[test]
fn from_islands_keeps_switches_and_weather() {
    let i = Islands {
        work: false,
        notif: false,
        ..Islands::default()
    };
    let b = BarLayout::from_islands(&i, true);
    assert_eq!(ids(&b.start), ["system"]);
    assert_eq!(ids(&b.end), ["weather", "clock", "status"]);
}

#[test]
fn strict_rejects() {
    for bad in [
        json!([]),
        json!({"nope": []}),
        json!({"start": "system"}),
        json!({"start": [{"id": "nope"}]}),
        json!({"start": [{"id": "clock"}], "end": [{"id": "clock"}]}),
        json!({"start": [{"id": "clock", "size": "huge"}]}),
        json!({"start": [{"id": "clock", "x": 1}]}),
        json!({"start": [{"size": "wide"}]}),
        json!({"options": {"nope": {}}}),
        json!({"options": {"clock": {"seconds": "yes"}}}),
        json!({"options": {"clock": {"date": "medium"}}}),
        json!({"options": {"stocks": {"symbols": ["aapl"]}}}),
        json!({"options": {"stocks": {"symbols": ["AAPL", "AAPL"]}}}),
        json!({"options": {"stocks": {"symbols": ["A B"]}}}),
        json!({"options": {"news": {"feeds": ["http://x.org/rss"]}}}),
        json!({"options": {"news": {"feeds": ["https://"]}}}),
        json!({"options": {"news": {"feeds": ["https://u@x.org/"]}}}),
        json!({"options": {"news": {"feeds": ["https://x.org/a b"]}}}),
        json!({"options": {"worldclock": {"zones": ["../etc/passwd"]}}}),
        json!({"options": {"worldclock": {"zones": ["Nowhere"]}}}),
    ] {
        assert!(BarLayout::from_value(&bad).is_err(), "{bad}");
    }
    let many: Vec<_> = (0..25).map(|_| json!({"id": "separator"})).collect();
    assert!(BarLayout::from_value(&json!({"start": many})).is_err());
    let syms: Vec<_> = (0..13).map(|i| format!("S{i}")).collect();
    assert!(BarLayout::from_value(&json!({"options": {"stocks": {"symbols": syms}}})).is_err());
}

#[test]
fn strict_accepts() {
    let b = BarLayout::from_value(&json!({
        "start": [{"id": "work"}, {"id": "separator"}, {"id": "separator"}],
        "center": [{"id": "clock", "size": "wide"}],
        "end": [{"id": "stocks", "size": "compact"}],
        "options": {
            "clock": {"seconds": true, "date": "none"},
            "stocks": {"symbols": ["BTC-USD", "^FTSE", "VOD.L"]},
            "news": {"feeds": ["https://feeds.bbci.co.uk/news/world/rss.xml"]},
            "worldclock": {"zones": ["America/New_York", "UTC", "America/Argentina/Salta"]}
        }
    }))
    .unwrap();
    assert_eq!(b.center[0].size, Some(Size::Large), "wide is now large");
    assert_eq!(b.end[0].size, Some(Size::Small), "compact is now small");
    assert!(b.has("stocks") && !b.has("news"));
    assert_eq!(b.options.clock.date, DateStyle::None);
    // Round trip; normal sizes are not written.
    let v = serde_json::to_value(&b).unwrap();
    assert!(v["start"][0].get("size").is_none());
    assert_eq!(BarLayout::from_value(&v).unwrap(), b);
    // Absent options keep the defaults.
    let b = BarLayout::from_value(&json!({"start": []})).unwrap();
    assert_eq!(b.options, BarOptions::default());
}

#[test]
fn lenient_load_repairs() {
    #[derive(Deserialize)]
    struct W {
        #[serde(deserialize_with = "lenient_bar")]
        bar: Option<BarLayout>,
    }
    let w: W = serde_json::from_value(json!({"bar": {
        "start": [{"id": "work"}, {"id": "future"}, 3, {"id": "work"}, {"id": "clock", "size": "huge"}],
        "end": "x",
        "options": {"stocks": {"symbols": ["aapl", "AAPL", "bad sym"]}, "clock": 4,
                    "news": {"feeds": ["http://x", "https://ok.org/feed"]},
                    "worldclock": {"zones": ["Europe/Paris", 5]}}
    }}))
    .unwrap();
    let b = w.bar.unwrap();
    assert_eq!(ids(&b.start), ["work", "clock"]);
    assert_eq!(b.start[1].size, None);
    assert!(b.end.is_empty());
    assert_eq!(b.options.stocks.symbols, ["AAPL"]);
    assert_eq!(b.options.clock, ClockOptions::default());
    assert_eq!(b.options.news.feeds, ["https://ok.org/feed"]);
    assert_eq!(b.options.worldclock.zones, ["Europe/Paris"]);
    let w: W = serde_json::from_value(json!({"bar": 3})).unwrap();
    assert!(w.bar.is_none());
}

#[test]
fn settings_migrate_and_keep_bar() {
    // An old file: no bar, work off, weather on.
    let mut s: Settings = serde_json::from_value(json!({
        "shell": {"islands": {"work": false}},
        "weather_location": {"name": "Oslo", "lat": 59.91, "lon": 10.75}
    }))
    .unwrap();
    assert!(s.shell.bar.is_none());
    s = s.validated().0;
    let b = s.shell.bar.clone().unwrap();
    assert_eq!(ids(&b.start), ["system"]);
    assert_eq!(ids(&b.end), ["weather", "clock", "notif", "status"]);
    // A shell section written without `bar` keeps the layout.
    s.set_section("shell", json!({"clock_24h": false})).unwrap();
    assert_eq!(s.shell.bar.as_ref(), Some(&b));
    // And one with it replaces it.
    s.set_section("shell", json!({"bar": {"center": [{"id": "clock"}]}}))
        .unwrap();
    assert_eq!(ids(&s.shell.bar.as_ref().unwrap().center), ["clock"]);
    assert!(s.shell.bar.as_ref().unwrap().start.is_empty());
    assert_eq!(
        s.section("shell").unwrap()["bar"]["center"][0]["id"],
        "clock"
    );
    // A fresh install has a bar from the start.
    assert_eq!(Settings::default().shell.bar, Some(BarLayout::default()));
}

#[test]
fn edits() {
    // The bar of a file from before the Tray island (the edits don't depend on it).
    let mut b = BarLayout::from_islands(&Islands::default(), false);
    let e = |v: serde_json::Value| -> BarEdit { serde_json::from_value(v).unwrap() };
    b.apply(&e(
        json!({"op": "move", "id": "clock", "zone": "end", "index": 0,
        "to_zone": "center", "to_index": 0}),
    ))
    .unwrap();
    assert_eq!(ids(&b.center), ["clock"]);
    assert_eq!(ids(&b.end), ["notif", "status"]);
    // Stale: notif is no longer at end[0]... it is; status at end[0] isn't.
    let before = b.clone();
    assert!(
        b.apply(&e(
            json!({"op": "remove", "id": "status", "zone": "end", "index": 0})
        ))
        .is_err()
    );
    assert_eq!(b, before, "a refused edit changes nothing");
    b.apply(&e(
        json!({"op": "add", "id": "stocks", "zone": "start", "index": 1}),
    ))
    .unwrap();
    assert_eq!(ids(&b.start), ["system", "stocks", "work"]);
    assert!(
        b.apply(&e(json!({"op": "add", "id": "stocks", "zone": "end"})))
            .is_err(),
        "twice"
    );
    b.apply(&e(json!({"op": "add", "id": "separator", "zone": "end"})))
        .unwrap();
    assert_eq!(ids(&b.end), ["notif", "status", "separator"]);
    b.apply(&e(
        json!({"op": "size", "id": "stocks", "zone": "start", "index": 1, "size": "wide"}),
    ))
    .unwrap();
    assert_eq!(b.start[1].size, Some(Size::Large));
    // Setting an island back to its own default stores nothing.
    b.apply(&e(
        json!({"op": "size", "id": "stocks", "zone": "start", "index": 1, "size": "medium"}),
    ))
    .unwrap();
    assert_eq!(b.start[1].size, None);
    assert!(
        serde_json::from_value::<BarEdit>(
            json!({"op": "size", "id": "stocks", "zone": "start", "index": 1, "size": "normal"})
        )
        .is_err()
    );
    b.apply(&e(
        json!({"op": "options", "island": "stocks", "value": {"symbols": ["TSLA"]}}),
    ))
    .unwrap();
    assert_eq!(b.options.stocks.symbols, ["TSLA"]);
    for bad in [
        json!({"op": "options", "island": "stocks", "value": {"symbols": ["bad sym"]}}),
        json!({"op": "options", "island": "status", "value": {}}),
        json!({"op": "options", "island": "clock", "value": {"nope": 1}}),
        json!({"op": "add", "id": "nope", "zone": "end"}),
        json!({"op": "add", "id": "news", "zone": "middle"}),
        json!({"op": "move", "id": "notif", "zone": "end", "index": 0, "to_zone": "x", "to_index": 0}),
    ] {
        assert!(b.apply(&e(bad.clone())).is_err(), "{bad}");
    }
    assert!(serde_json::from_value::<BarEdit>(json!({"op": "fly"})).is_err());
    b.apply(&BarEdit::Reset).unwrap();
    assert_eq!(
        ids(&b.end),
        ["clock", "notif", "security", "tray", "status"]
    );
    assert_eq!(b.options.stocks.symbols, ["TSLA"], "reset keeps options");
}

#[test]
fn helpers() {
    assert_eq!(clean_symbol(" brk.b "), Some("BRK.B".into()));
    assert_eq!(clean_symbol("^"), None);
    assert_eq!(clean_symbol("TOOLONGSYMBOL12345"), None);
    assert!(valid_feed_url("https://news.ycombinator.com/rss"));
    assert!(valid_feed_url("https://x.org:8443/feed?x=1"));
    assert!(!valid_feed_url("https://x.org\n/feed"));
    assert!(valid_zone("UTC") && valid_zone("Etc/GMT+5"));
    assert!(!valid_zone("Europe/") && !valid_zone("/Paris"));
    assert_eq!(island_name("stocks"), Some("Stock ticker"));
    assert_eq!(island_name("time"), None);
}
