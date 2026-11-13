//! The news island's headlines: RSS 2.0 / RSS 1.0 / Atom feeds the user picked, fetched only
//! while `news` is placed. Titles are reduced to plain text; links must be http(s). `OpenLink`
//! opens only links this module itself last sent.

use super::feeds::{self, FeedLoop, Source};
use futures_util::StreamExt;
use quick_xml::XmlVersion;
use quick_xml::events::Event;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Mutex;
use uq_core::settings::Settings;
use uq_core::settings::bar::{MAX_FEEDS, valid_feed_url};

pub const EVERY: std::time::Duration = std::time::Duration::from_secs(15 * 60);
const CAP: usize = 512 * 1024;
const CONCURRENT: usize = 4;
const MAX_ITEMS: usize = 40;
const MAX_TITLE: usize = 300;
const MAX_SOURCE: usize = 80;
const MAX_LINK: usize = 2048;

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub title: String,
    pub link: String,
    pub published: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    /// The feed's own title, plain text, at most 80 characters.
    pub title: Option<String>,
    pub items: Vec<Item>,
}

pub struct News {
    http: reqwest::Client,
    /// Only the test constructor sets this: plain-HTTP feeds on a local fake.
    allow_http: bool,
    /// Every link in the last `News` event; `OpenLink` opens only these.
    links: Mutex<HashSet<String>>,
    opener: &'static str,
}

pub type NewsLoop = FeedLoop<News>;

impl News {
    pub fn new() -> News {
        News::build(false, "xdg-open")
    }

    /// Accepts http feed URLs and opens links with `opener` instead of xdg-open.
    #[cfg(test)]
    pub fn for_tests(opener: &'static str) -> News {
        News::build(true, opener)
    }

    fn build(allow_http: bool, opener: &'static str) -> News {
        News {
            http: feeds::client(feeds::https_redirects()),
            allow_http,
            links: Mutex::new(HashSet::new()),
            opener,
        }
    }

    fn url_ok(&self, u: &str) -> bool {
        valid_feed_url(u) || (self.allow_http && u.starts_with("http://127.0.0.1:"))
    }

    async fn fetch_feed(&self, url: &str) -> Result<Parsed, &'static str> {
        if !self.url_ok(url) {
            return Err("url");
        }
        let body = feeds::get_capped(&self.http, url, CAP, true).await?;
        parse_feed(&body)
    }

    /// `PreviewFeed(url)`: `{title, items}` for a feed URL before it is added.
    pub async fn preview(&self, url: &str) -> Result<Value, String> {
        if !self.url_ok(url) {
            return Err("not an https feed address".into());
        }
        match self.fetch_feed(url).await {
            Ok(p) => Ok(json!({
                "title": p.title.unwrap_or_else(|| host(url)),
                "items": p.items.len(),
            })),
            Err("not a feed") => Err("not an RSS or Atom feed".into()),
            Err(k) => Err(format!("request failed ({k})")),
        }
    }

    /// `OpenLink(url)`: open a link from the last `News` event in the user's browser. Returns
    /// once xdg-open is started; it runs detached in its own process group.
    pub fn open_link(&self, url: &str) -> Result<Value, String> {
        let known = self
            .links
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(url);
        if !known || !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err("not a link from the news feed".into());
        }
        let mut child = tokio::process::Command::new(self.opener)
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|e| format!("could not start {} ({})", self.opener, e.kind()))?;
        // Reap it whenever it ends, so it never lingers as a zombie.
        tokio::spawn(async move {
            let _ = child.wait().await;
        });
        Ok(Value::Null)
    }
}

fn host(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_default()
}

impl Source for News {
    type Key = Vec<String>;
    const EVENT: &'static str = "News";
    const CACHE: &'static str = "news.json";

    fn key(s: &Settings) -> Option<Vec<String>> {
        feeds::placed(s, "news").then(|| {
            s.shell
                .bar
                .as_ref()
                .map(|b| {
                    b.options
                        .news
                        .feeds
                        .iter()
                        .take(MAX_FEEDS)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    async fn fetch(&self, urls: &Vec<String>, last: Option<&Value>) -> Value {
        let got: Vec<Result<Parsed, &'static str>> = futures_util::stream::iter(urls.clone())
            .map(|u| async move { self.fetch_feed(&u).await })
            .buffered(CONCURRENT)
            .collect()
            .await;
        let mut ok = 0;
        let mut kinds = Vec::new();
        let mut items: Vec<Value> = Vec::new();
        let mut feed_list = Vec::new();
        for (url, r) in urls.iter().zip(got) {
            match r {
                Ok(p) => {
                    ok += 1;
                    let source = p.title.clone().unwrap_or_else(|| host(url));
                    items.extend(p.items.into_iter().map(|i| {
                        json!({"title": i.title, "link": i.link, "source": source,
                               "published": i.published, "feed": url})
                    }));
                    feed_list.push(json!({"url": url, "title": p.title, "ok": true}));
                }
                Err(k) => {
                    kinds.push(k);
                    // Keep this feed's last items and title.
                    if let Some(l) = last {
                        items.extend(
                            l["items"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter(|i| i["feed"] == url.as_str())
                                .cloned(),
                        );
                    }
                    let title = last
                        .and_then(|l| l["feeds"].as_array())
                        .and_then(|f| f.iter().find(|f| f["url"] == url.as_str()))
                        .map_or(Value::Null, |f| f["title"].clone());
                    feed_list.push(json!({"url": url, "title": title, "ok": false}));
                }
            }
        }
        if !kinds.is_empty() {
            kinds.sort_unstable();
            kinds.dedup();
            eprintln!(
                "uqctl bridge: {} of {} news feeds failed ({})",
                urls.len() - ok,
                urls.len(),
                kinds.join(", ")
            );
        }
        let status = feeds::status(ok, urls.len());
        let fetched_at = if status == "error" {
            last.map_or(Value::Null, |l| l["fetched_at"].clone())
        } else {
            json!(feeds::unix_now())
        };
        json!({"status": status, "fetched_at": fetched_at, "items": merge(items),
               "feeds": feed_list})
    }

    fn emitted(&self, payload: &Value) {
        let links = payload["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|i| i["link"].as_str().map(str::to_owned))
            .collect();
        *self.links.lock().unwrap_or_else(|e| e.into_inner()) = links;
    }
}

/// Newest first (undated last, in feed order), one per link, at most 40.
fn merge(mut items: Vec<Value>) -> Vec<Value> {
    items.sort_by_key(|i| std::cmp::Reverse(i["published"].as_i64().unwrap_or(i64::MIN)));
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|i| seen.insert(i["link"].as_str().unwrap_or("").to_owned()))
        .take(MAX_ITEMS)
        .collect()
}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    FeedTitle,
    Title,
    Link,
    Date,
    Updated,
}

#[derive(Default)]
struct ItemBuf {
    title: String,
    link: Option<String>,
    alt_link: Option<String>,
    date: String,
    updated: String,
}

/// Parse an RSS 2.0, RSS 1.0 or Atom document. A document cut off at the size cap keeps the
/// items completed before the cut.
pub fn parse_feed(bytes: &[u8]) -> Result<Parsed, &'static str> {
    let text = String::from_utf8_lossy(bytes);
    let mut r = quick_xml::Reader::from_str(&text);
    r.config_mut().check_end_names = false;
    let mut stack: Vec<String> = Vec::new();
    let mut is_feed = false;
    let mut feed_title: Option<String> = None;
    let mut item: Option<ItemBuf> = None;
    let mut items = Vec::new();
    let mut capture: Option<(Field, usize)> = None;
    let mut buf = String::new();
    loop {
        let ev = match r.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(ev) => ev,
        };
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(ev, Event::Empty(_));
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if stack.is_empty() {
                    is_feed = matches!(name.as_str(), "rss" | "feed" | "rdf:RDF" | "RDF");
                    if !is_feed {
                        return Err("not a feed");
                    }
                }
                let parent = stack.last().map(String::as_str).unwrap_or("");
                if capture.is_none() {
                    let in_item = item.is_some() && matches!(parent, "item" | "entry");
                    match name.as_str() {
                        "item" | "entry" => item = Some(ItemBuf::default()),
                        "title" if in_item => capture = Some((Field::Title, stack.len() + 1)),
                        "title"
                            if feed_title.is_none()
                                && item.is_none()
                                && matches!(parent, "channel" | "feed") =>
                        {
                            capture = Some((Field::FeedTitle, stack.len() + 1));
                        }
                        "link" if in_item => {
                            let href = e.try_get_attribute("href").ok().flatten().and_then(|a| {
                                a.normalized_value(XmlVersion::Implicit1_0)
                                    .ok()
                                    .map(|v| v.into_owned())
                            });
                            match href {
                                Some(h) => {
                                    let rel = e
                                        .try_get_attribute("rel")
                                        .ok()
                                        .flatten()
                                        .and_then(|a| {
                                            a.normalized_value(XmlVersion::Implicit1_0).ok()
                                        })
                                        .map(|v| v.into_owned());
                                    let it = item.as_mut().expect("in item");
                                    if matches!(rel.as_deref(), None | Some("alternate")) {
                                        it.link.get_or_insert(h);
                                    } else {
                                        it.alt_link.get_or_insert(h);
                                    }
                                }
                                None => capture = Some((Field::Link, stack.len() + 1)),
                            }
                        }
                        "pubDate" | "published" | "dc:date" if in_item => {
                            capture = Some((Field::Date, stack.len() + 1));
                        }
                        "updated" if in_item => capture = Some((Field::Updated, stack.len() + 1)),
                        _ => {}
                    }
                }
                stack.push(name);
                // An empty element ends at once.
                if empty {
                    end_element(
                        &mut stack,
                        &mut capture,
                        &mut buf,
                        &mut item,
                        &mut items,
                        &mut feed_title,
                    );
                }
            }
            Event::End(_) => end_element(
                &mut stack,
                &mut capture,
                &mut buf,
                &mut item,
                &mut items,
                &mut feed_title,
            ),
            Event::Text(t) if capture.is_some() => {
                buf.push_str(&t.decode().unwrap_or_default());
            }
            Event::CData(t) if capture.is_some() => {
                buf.push_str(&t.decode().unwrap_or_default());
            }
            Event::GeneralRef(g) if capture.is_some() => {
                if let Ok(Some(c)) = g.resolve_char_ref() {
                    buf.push(c);
                } else {
                    let name = g.decode().unwrap_or_default();
                    match name.as_ref() {
                        "lt" => buf.push('<'),
                        "gt" => buf.push('>'),
                        "amp" => buf.push('&'),
                        "quot" => buf.push('"'),
                        "apos" => buf.push('\''),
                        // Left for the HTML pass (`&nbsp;` and friends).
                        other => {
                            buf.push('&');
                            buf.push_str(other);
                            buf.push(';');
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if !is_feed {
        return Err("not a feed");
    }
    Ok(Parsed {
        title: feed_title,
        items,
    })
}

fn end_element(
    stack: &mut Vec<String>,
    capture: &mut Option<(Field, usize)>,
    buf: &mut String,
    item: &mut Option<ItemBuf>,
    items: &mut Vec<Item>,
    feed_title: &mut Option<String>,
) {
    if let Some((field, depth)) = *capture
        && depth == stack.len()
    {
        let text = std::mem::take(buf);
        match (field, item.as_mut()) {
            (Field::FeedTitle, _) => {
                *feed_title = Some(plain_text(&text, MAX_SOURCE)).filter(|t| !t.is_empty());
            }
            (Field::Title, Some(it)) => it.title = text,
            (Field::Link, Some(it)) => {
                it.link.get_or_insert(text);
            }
            (Field::Date, Some(it)) => it.date = text,
            (Field::Updated, Some(it)) => it.updated = text,
            _ => {}
        }
        *capture = None;
    }
    let name = stack.pop().unwrap_or_default();
    if matches!(name.as_str(), "item" | "entry")
        && let Some(it) = item.take()
    {
        let title = plain_text(&it.title, MAX_TITLE);
        let link = it.link.or(it.alt_link).and_then(|l| clean_link(&l));
        if let Some(link) = link
            && !title.is_empty()
        {
            let date = if it.date.trim().is_empty() {
                &it.updated
            } else {
                &it.date
            };
            items.push(Item {
                title,
                link,
                published: parse_date(date),
            });
        }
    }
}

/// An absolute http(s) URL with a host, normalised; anything else is dropped.
pub fn clean_link(l: &str) -> Option<String> {
    let l = l.trim();
    if l.len() > MAX_LINK {
        return None;
    }
    let u = reqwest::Url::parse(l).ok()?;
    (matches!(u.scheme(), "http" | "https") && u.host_str().is_some_and(|h| !h.is_empty()))
        .then(|| u.to_string())
}

/// RFC 2822 (RSS) or RFC 3339 (Atom) to unix seconds.
pub fn parse_date(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(s)
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(&s.replacen(' ', "T", 1)))
    {
        return Some(d.timestamp());
    }
    // RFC 2822 without the weekday (feeds often get it wrong, and chrono checks it), with a
    // `UTC` zone name read as `+0000`.
    let s = match s.split_once(',') {
        Some((day, rest)) if day.len() <= 9 && day.chars().all(|c| c.is_ascii_alphabetic()) => {
            rest.trim()
        }
        _ => s,
    };
    let fixed = s.strip_suffix(" UTC").map(|b| format!("{b} +0000"));
    chrono::DateTime::parse_from_rfc2822(fixed.as_deref().unwrap_or(s))
        .ok()
        .map(|d| d.timestamp())
}

/// Markup and entities out, whitespace collapsed, at most `max` characters.
pub fn plain_text(s: &str, max: usize) -> String {
    let s = strip_tags(&decode_entities(&strip_tags(s)));
    let mut out = String::new();
    for word in s.split(|c: char| c.is_whitespace() || c.is_control()) {
        if word.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.chars().count() > max {
        out = out
            .chars()
            .take(max - 1)
            .collect::<String>()
            .trim_end()
            .to_owned();
        out.push('…');
    }
    out
}

/// Remove `<tag ...>`, `</tag>`, `<!-- -->` and `<?...?>`; a `<` not starting a tag stays.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let tag_like = after
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '/' | '!' | '?'));
        match (tag_like, after.find('>')) {
            (true, Some(j)) => {
                out.push(' ');
                rest = &after[j + 1..];
            }
            (true, None) => {
                rest = "";
            }
            (false, _) => {
                out.push('<');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Decode numeric and the common named HTML entities; unknown ones stay as written.
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let end = after.find(';').filter(|&j| j <= 10);
        let decoded = end.and_then(|j| {
            let name = &after[..j];
            let c = if let Some(num) = name.strip_prefix('#') {
                let n = match num.strip_prefix(['x', 'X']) {
                    Some(h) => u32::from_str_radix(h, 16).ok(),
                    None => num.parse().ok(),
                };
                n.and_then(char::from_u32).filter(|c| *c != '\0')
            } else {
                named_entity(name)
            };
            c.map(|c| (c, j))
        });
        match decoded {
            Some((c, j)) => {
                out.push(c);
                rest = &after[j + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn named_entity(n: &str) -> Option<char> {
    Some(match n {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "ndash" => '–',
        "mdash" => '—',
        "lsquo" => '‘',
        "rsquo" => '’',
        "ldquo" => '“',
        "rdquo" => '”',
        "hellip" => '…',
        "euro" => '€',
        "pound" => '£',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "deg" => '°',
        "middot" => '·',
        "bull" => '•',
        "eacute" => 'é',
        "egrave" => 'è',
        "aacute" => 'á',
        "oacute" => 'ó',
        "uuml" => 'ü',
        "ouml" => 'ö',
        "auml" => 'ä',
        "szlig" => 'ß',
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
