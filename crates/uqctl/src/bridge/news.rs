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
mod parse;
pub use parse::*;

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
    /// Open links in a scope of their own in `app.slice` (off in tests, which use a fake opener).
    scoped: bool,
}

pub type NewsLoop = FeedLoop<News>;

impl News {
    pub fn new() -> News {
        News::build(false, "xdg-open", true)
    }

    /// Accepts http feed URLs and opens links with `opener` instead of xdg-open.
    #[cfg(test)]
    pub fn for_tests(opener: &'static str) -> News {
        News::build(true, opener, false)
    }

    fn build(allow_http: bool, opener: &'static str, scoped: bool) -> News {
        News {
            http: feeds::client(feeds::https_redirects()),
            allow_http,
            links: Mutex::new(HashSet::new()),
            opener,
            scoped,
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
        // In a scope of its own, so a browser it starts is an app, not part of the shell's
        // service (which scdr treats as the desktop).
        super::opener::open(self.opener, url, self.scoped.then_some("link"))?;
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

#[cfg(test)]
mod tests;
