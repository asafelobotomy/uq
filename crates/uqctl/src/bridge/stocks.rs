//! The stocks island's quotes: Yahoo Finance's keyless chart endpoint, one request per symbol,
//! only while `stocks` is placed. Only the symbol is sent; no location, no cookies.

use super::feeds::{self, FeedLoop, Source};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use uq_core::settings::Settings;
use uq_core::settings::bar::clean_symbol;

pub const EVERY: std::time::Duration = std::time::Duration::from_secs(5 * 60);
const CAP: usize = 256 * 1024;
const CONCURRENT: usize = 4;
const MAX_NAME: usize = 80;

pub struct Stocks {
    http: reqwest::Client,
    /// Up to and including `/chart/`; the encoded symbol follows.
    base: String,
}

pub type StocksLoop = FeedLoop<Stocks>;

impl Stocks {
    pub fn new() -> Stocks {
        Stocks::build("https://query1.finance.yahoo.com/v8/finance/chart/")
    }

    #[cfg(test)]
    pub fn with_base(base: &str) -> Stocks {
        Stocks::build(base)
    }

    fn build(base: &str) -> Stocks {
        Stocks {
            http: feeds::client(reqwest::redirect::Policy::none()),
            base: base.into(),
        }
    }

    fn url(&self, symbol: &str) -> String {
        format!("{}{}?range=1d&interval=1d", self.base, encode(symbol))
    }

    async fn quote(&self, symbol: &str) -> Result<Value, &'static str> {
        if clean_symbol(symbol).as_deref() != Some(symbol) {
            return Err("symbol");
        }
        let body = feeds::get_capped(&self.http, &self.url(symbol), CAP, false).await?;
        parse_chart(&body, symbol)
    }
}

/// Percent-encode everything but unreserved characters (`^GSPC` → `%5EGSPC`).
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn round(x: f64, dp: i32) -> f64 {
    let m = 10f64.powi(dp);
    (x * m).round() / m
}

/// A quote from a chart response's `chart.result[0].meta`; `symbol` is the one asked for.
pub fn parse_chart(body: &[u8], symbol: &str) -> Result<Value, &'static str> {
    #[derive(Deserialize)]
    struct Resp {
        chart: Chart,
    }
    #[derive(Deserialize)]
    struct Chart {
        result: Option<Vec<Res>>,
    }
    #[derive(Deserialize)]
    struct Res {
        meta: Meta,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Meta {
        short_name: Option<String>,
        long_name: Option<String>,
        regular_market_price: Option<f64>,
        chart_previous_close: Option<f64>,
        previous_close: Option<f64>,
        currency: Option<String>,
        regular_market_day_high: Option<f64>,
        regular_market_day_low: Option<f64>,
        regular_market_time: Option<i64>,
    }
    let r: Resp = serde_json::from_slice(body).map_err(|_| "decode")?;
    let m = r
        .chart
        .result
        .and_then(|v| v.into_iter().next())
        .ok_or("unknown")?
        .meta;
    let price = m
        .regular_market_price
        .filter(|p| p.is_finite())
        .ok_or("decode")?;
    let prev = m
        .chart_previous_close
        .or(m.previous_close)
        .filter(|p| p.is_finite() && *p != 0.0);
    let name = m
        .short_name
        .or(m.long_name)
        .map(|n| n.trim().chars().take(MAX_NAME).collect::<String>())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| symbol.to_string());
    let currency = m
        .currency
        .filter(|c| (1..=8).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_alphanumeric()));
    Ok(json!({
        "symbol": symbol,
        "name": name,
        "price": price,
        "change": prev.map(|p| round(price - p, 4)),
        "change_pct": prev.map(|p| round((price - p) / p * 100.0, 2)),
        "currency": currency,
        "day_high": m.regular_market_day_high,
        "day_low": m.regular_market_day_low,
        "market_time": m.regular_market_time,
        "ok": true,
    }))
}

/// The last good values for `symbol` from an earlier payload, marked not ok; or an empty entry.
fn stale_quote(symbol: &str, last: Option<&Value>) -> Value {
    let prev = last
        .and_then(|l| l["quotes"].as_array())
        .and_then(|q| {
            q.iter()
                .find(|q| q["symbol"] == symbol && q["price"].is_number())
        })
        .cloned();
    match prev {
        Some(mut q) => {
            q["ok"] = json!(false);
            q
        }
        None => json!({"symbol": symbol, "name": null, "price": null, "change": null,
            "change_pct": null, "currency": null, "day_high": null, "day_low": null,
            "market_time": null, "ok": false}),
    }
}

impl Source for Stocks {
    type Key = Vec<String>;
    const EVENT: &'static str = "Stocks";
    const CACHE: &'static str = "stocks.json";

    fn key(s: &Settings) -> Option<Vec<String>> {
        feeds::placed(s, "stocks").then(|| {
            s.shell
                .bar
                .as_ref()
                .map(|b| b.options.stocks.symbols.clone())
                .unwrap_or_default()
        })
    }

    async fn fetch(&self, symbols: &Vec<String>, last: Option<&Value>) -> Value {
        let got: Vec<Result<Value, &'static str>> = futures_util::stream::iter(symbols.clone())
            .map(|s| async move { self.quote(&s).await })
            .buffered(CONCURRENT)
            .collect()
            .await;
        let mut ok = 0;
        let mut kinds = Vec::new();
        let quotes: Vec<Value> = symbols
            .iter()
            .zip(got)
            .map(|(s, r)| match r {
                Ok(q) => {
                    ok += 1;
                    q
                }
                Err(k) => {
                    kinds.push(k);
                    stale_quote(s, last)
                }
            })
            .collect();
        if !kinds.is_empty() {
            kinds.sort_unstable();
            kinds.dedup();
            eprintln!(
                "uqctl bridge: {} of {} quotes failed ({})",
                symbols.len() - ok,
                symbols.len(),
                kinds.join(", ")
            );
        }
        let status = feeds::status(ok, symbols.len());
        // On total failure the data is as old as the last good fetch.
        let fetched_at = if status == "error" {
            last.map_or(Value::Null, |l| l["fetched_at"].clone())
        } else {
            json!(feeds::unix_now())
        };
        json!({"status": status, "fetched_at": fetched_at, "quotes": quotes})
    }
}

#[cfg(test)]
mod tests;
