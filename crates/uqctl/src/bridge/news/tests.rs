use super::super::feeds::test_http::*;
use super::super::protocol::Outbound;
use super::*;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use uq_core::settings::BarItem;

const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:atom="http://www.w3.org/2005/Atom" xmlns:media="http://search.yahoo.com/mrss/">
<channel>
  <title>World &amp; More</title>
  <atom:link href="https://example.org/self" rel="self"/>
  <link>https://example.org/</link>
  <item>
    <title><![CDATA[Markets <b>rally</b>   as   rates fall]]></title>
    <link>https://example.org/a?x=1&amp;y=2</link>
    <pubDate>Thu, 13 Nov 2026 10:00:00 GMT</pubDate>
    <media:title>Not this</media:title>
  </item>
  <item>
    <title>Caf&#233; &lt;i&gt;opens&lt;/i&gt; &amp;nbsp;today &#x2014; &quot;big&quot;</title>
    <link> https://example.org/b </link>
    <dc:date>2026-11-13T11:00:00+01:00</dc:date>
  </item>
  <item><title>Bad link</title><link>javascript:alert(1)</link></item>
  <item><title>Relative</title><link>/c</link></item>
  <item><title></title><link>https://example.org/empty-title</link></item>
  <item><title>No date</title><link>http://example.org/d</link><pubDate>whenever</pubDate></item>
</channel>
</rss>"#;

const ATOM: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title type="text">Atom Site</title>
  <link href="https://atom.example/"/>
  <entry>
    <title type="html">&lt;p&gt;Hello &amp;amp; welcome&lt;/p&gt;</title>
    <link rel="self" href="https://atom.example/self/1"/>
    <link rel="alternate" type="text/html" href="https://atom.example/1"/>
    <updated>2026-11-13T09:00:00Z</updated>
    <published>2026-11-12T09:00:00Z</published>
  </entry>
  <entry>
    <title>Only updated</title>
    <link href="https://atom.example/2"></link>
    <updated>2026-11-13T12:30:00Z</updated>
  </entry>
</feed>"#;

#[test]
fn parses_rss() {
    let p = parse_feed(RSS.as_bytes()).unwrap();
    assert_eq!(p.title.as_deref(), Some("World & More"));
    let got: Vec<_> = p
        .items
        .iter()
        .map(|i| (i.title.as_str(), i.link.as_str(), i.published))
        .collect();
    assert_eq!(
        got,
        [
            (
                "Markets rally as rates fall",
                "https://example.org/a?x=1&y=2",
                Some(1_794_564_000)
            ),
            (
                "Café opens today — \"big\"",
                "https://example.org/b",
                Some(1_794_564_000)
            ),
            ("No date", "http://example.org/d", None),
        ]
    );
}

#[test]
fn parses_atom() {
    let p = parse_feed(ATOM.as_bytes()).unwrap();
    assert_eq!(p.title.as_deref(), Some("Atom Site"));
    assert_eq!(p.items.len(), 2);
    assert_eq!(p.items[0].title, "Hello & welcome");
    assert_eq!(p.items[0].link, "https://atom.example/1");
    // `published` wins over `updated`; without it `updated` is used.
    assert_eq!(p.items[0].published, Some(1_794_474_000));
    assert_eq!(p.items[1].link, "https://atom.example/2");
    assert_eq!(p.items[1].published, Some(1_794_573_000));
}

#[test]
fn rejects_non_feeds_and_keeps_items_before_a_cut() {
    assert_eq!(
        parse_feed(b"<html><body>hi</body></html>"),
        Err("not a feed")
    );
    assert_eq!(parse_feed(b""), Err("not a feed"));
    assert_eq!(parse_feed(b"{\"json\": true}"), Err("not a feed"));
    let cut = &RSS[..RSS.find("<dc:date>").unwrap()];
    let p = parse_feed(cut.as_bytes()).unwrap();
    assert_eq!(p.items.len(), 1);
    assert_eq!(p.items[0].link, "https://example.org/a?x=1&y=2");
}

#[test]
fn plain_text_rules() {
    assert_eq!(plain_text("  a\n\t b  ", 300), "a b");
    assert_eq!(plain_text("x < y and <b>bold</b>", 300), "x < y and bold");
    assert_eq!(plain_text("&lt;script&gt;x&lt;/script&gt;", 300), "x");
    assert_eq!(
        plain_text("&#39;q&#39; &unknown; &amp;", 300),
        "'q' &unknown; &"
    );
    assert_eq!(plain_text("<!-- c -->t", 300), "t");
    let long = "word ".repeat(100);
    let t = plain_text(&long, 300);
    assert!(t.chars().count() <= 300 && t.ends_with('…'), "{t}");
    assert_eq!(plain_text(&"é".repeat(100), 80).chars().count(), 80);
}

#[test]
fn dates_and_links() {
    assert_eq!(
        parse_date("Thu, 13 Nov 2026 10:00:00 +0000"),
        Some(1_794_564_000)
    );
    assert_eq!(
        parse_date("Thu, 13 Nov 2026 10:00:00 UTC"),
        Some(1_794_564_000)
    );
    assert_eq!(parse_date("2026-11-13T10:00:00Z"), Some(1_794_564_000));
    assert_eq!(parse_date("2026-11-13 10:00:00Z"), Some(1_794_564_000));
    // A wrong weekday (13 Nov 2026 is a Friday) doesn't lose the date.
    assert_eq!(
        parse_date("Thu, 13 Nov 2026 11:00:00 +0100"),
        Some(1_794_564_000)
    );
    assert_eq!(parse_date("13 Nov 2026 10:00 GMT"), Some(1_794_564_000));
    assert_eq!(parse_date("yesterday"), None);
    assert_eq!(parse_date(""), None);
    assert_eq!(clean_link("https://x.org"), Some("https://x.org/".into()));
    for bad in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "/relative",
        "mailto:a@b",
        "data:text/html,x",
    ] {
        assert_eq!(clean_link(bad), None, "{bad}");
    }
}

fn settings(placed: bool, feeds: &[String]) -> Option<Settings> {
    let mut s = Settings::default();
    let bar = s.shell.bar.as_mut().unwrap();
    if placed {
        bar.center.push(BarItem::new("news"));
    }
    bar.options.news.feeds = feeds.to_vec();
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

#[tokio::test]
async fn loop_merges_gates_and_opens_only_sent_links() {
    let older = r#"<rss><channel><title>Second</title>
        <item><title>Old news</title><link>https://example.org/old</link><pubDate>Mon, 10 Nov 2026 10:00:00 GMT</pubDate></item>
        <item><title>Dup</title><link>https://example.org/b</link><pubDate>Mon, 10 Nov 2026 10:00:00 GMT</pubDate></item>
        </channel></rss>"#;
    let (base, seen) = serve(
        routes(&[
            ("/rss", Reply::ok(RSS)),
            ("/older", Reply::ok(older)),
            ("/atom", Reply::ok(ATOM)),
        ]),
        |_| None,
    )
    .await;
    let feeds: Vec<String> = ["/rss", "/missing", "/older", "/atom"]
        .iter()
        .map(|p| format!("{base}{p}"))
        .collect();
    let (stx, srx) = watch::channel(settings(false, &feeds));
    let (tx, mut rx) = mpsc::channel(16);
    let stop = CancellationToken::new();
    let l = FeedLoop::new(
        News::for_tests("true"),
        tx,
        srx,
        stop.clone(),
        Duration::from_secs(600),
        vec![],
        None,
        Duration::from_secs(30),
    );
    let task = tokio::spawn(l.clone().run());
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(seen.lock().unwrap().is_empty(), "nothing while not placed");

    stx.send_replace(settings(true, &feeds));
    let (e, d) = event(&mut rx).await;
    assert_eq!(e, "News");
    assert_eq!(d["status"], "partial");
    let f: Vec<_> = d["feeds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["title"].clone(), f["ok"].as_bool().unwrap()))
        .collect();
    assert_eq!(
        f,
        [
            (json!("World & More"), true),
            (Value::Null, false),
            (json!("Second"), true),
            (json!("Atom Site"), true),
        ]
    );
    let links: Vec<_> = d["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["link"].as_str().unwrap())
        .collect();
    // Newest first, the duplicate link once (its newer copy), undated last.
    assert_eq!(
        links,
        [
            "https://atom.example/2",
            "https://example.org/a?x=1&y=2",
            "https://example.org/b",
            "https://atom.example/1",
            "https://example.org/old",
            "http://example.org/d",
        ]
    );
    assert_eq!(d["items"][1]["source"], "World & More");
    assert_eq!(d["items"][1]["feed"], feeds[0]);

    // OpenLink: only links from that event.
    let src = &l.src;
    assert!(src.open_link("https://example.org/b").is_ok());
    assert!(src.open_link("https://evil.example/").is_err());
    assert!(src.open_link("file:///etc/passwd").is_err());
    assert!(src.open_link("").is_err());

    // Removed: null once, and the links are forgotten.
    stx.send_replace(settings(false, &feeds));
    assert_eq!(event(&mut rx).await, ("News".into(), Value::Null));
    assert!(src.open_link("https://example.org/b").is_err());
    stop.cancel();
    task.await.unwrap();
}

#[tokio::test]
async fn all_feeds_failing_keeps_last_items() {
    let (base, _) = serve(routes(&[("/rss", Reply::ok(RSS))]), |n| {
        (n > 1).then(|| Reply::status("503 Busy"))
    })
    .await;
    let feeds = vec![format!("{base}/rss")];
    let (_stx, srx) = watch::channel(settings(true, &feeds));
    let (tx, mut rx) = mpsc::channel(16);
    let stop = CancellationToken::new();
    let l = FeedLoop::new(
        News::for_tests("true"),
        tx,
        srx,
        stop.clone(),
        Duration::from_millis(100),
        vec![],
        None,
        Duration::from_secs(30),
    );
    let task = tokio::spawn(l.run());
    let (_, good) = event(&mut rx).await;
    assert_eq!(good["status"], "ok");
    let (_, d) = event(&mut rx).await;
    assert_eq!(d["status"], "error");
    assert_eq!(d["items"], good["items"]);
    assert_eq!(d["fetched_at"], good["fetched_at"]);
    assert_eq!(
        d["feeds"],
        json!([{"url": feeds[0], "title": "World & More", "ok": false}])
    );
    stop.cancel();
    task.await.unwrap();
}

#[tokio::test]
async fn preview_feed() {
    let (base, _) = serve(
        routes(&[
            ("/rss", Reply::ok(RSS)),
            ("/html", Reply::ok("<html></html>")),
            ("/to-http", Reply::redirect("http://example.org/rss")),
        ]),
        |_| None,
    )
    .await;
    let n = News::for_tests("true");
    assert_eq!(
        n.preview(&format!("{base}/rss")).await.unwrap(),
        json!({"title": "World & More", "items": 3})
    );
    assert_eq!(
        n.preview(&format!("{base}/html")).await,
        Err("not an RSS or Atom feed".into())
    );
    assert_eq!(
        n.preview(&format!("{base}/nope")).await,
        Err("request failed (status)".into())
    );
    // The test server is plain http, so even its first redirect is refused.
    assert_eq!(
        n.preview(&format!("{base}/to-http")).await,
        Err("request failed (redirect)".into())
    );
    // The real client takes https only.
    let real = News::new();
    for bad in ["http://example.org/rss", "ftp://x.org/", "https://", "x"] {
        assert_eq!(
            real.preview(bad).await,
            Err("not an https feed address".into())
        );
    }
}
