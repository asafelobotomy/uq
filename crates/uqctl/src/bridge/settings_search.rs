//! Spotlight's Settings results: every page and setting the Settings app can show, read from
//! `uq-settings --search-index` and matched with the same scorer Settings' own search uses.
//!
//! The list is read off the request path when Spotlight opens and the last read is missing,
//! older than [`STALE_AFTER`] (plugin pages come and go) or the binary changed. Opening a row goes
//! through `uqctl settings`' own path: a running Settings shows the page, else one starts in a
//! scope of its own.

use super::calc::ProviderItem;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};
use uq_core::fuzzy::score;

pub const APP: &str = "uq-settings";
pub const STALE_AFTER: Duration = Duration::from_secs(10 * 60);
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_ROWS: usize = 5;
const MAX_QUERY: usize = 64;
/// Separates page and setting in a row's id.
const SEP: char = '\u{1f}';

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub page: String,
    pub setting: String,
    pub label: String,
    pub page_name: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Default)]
pub struct Index {
    pub rows: Vec<Row>,
    pub read: Option<Instant>,
    /// The binary's modification time when it was read.
    pub stamp: Option<SystemTime>,
    pub reading: bool,
}

/// Where `uq-settings` is on PATH.
pub fn binary() -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|d| d.join(APP))
        .find(|p| p.is_file())
}

fn stamp_of(bin: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(bin).and_then(|m| m.modified()).ok()
}

impl Index {
    /// Whether to read the list again now.
    pub fn wants_read(&self, now: Instant, bin: &std::path::Path) -> bool {
        !self.reading
            && (self
                .read
                .is_none_or(|t| now.saturating_duration_since(t) >= STALE_AFTER)
                || self.stamp != stamp_of(bin))
    }
}

/// Parse `uq-settings --search-index`'s output. Anything else is an empty list.
pub fn parse(out: &[u8]) -> Vec<Row> {
    if out.len() > MAX_BYTES {
        return Vec::new();
    }
    serde_json::from_slice(out).unwrap_or_default()
}

/// Run `bin --search-index`: its rows and the binary's stamp, or None when it fails.
pub async fn read(bin: PathBuf) -> Option<(Vec<Row>, Option<SystemTime>)> {
    let stamp = stamp_of(&bin);
    let run = tokio::process::Command::new(&bin)
        .arg("--search-index")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(READ_TIMEOUT, run).await.ok()?.ok()?;
    out.status.success().then(|| (parse(&out.stdout), stamp))
}

/// The best rows for `query`: a label match ranks above a keyword match. Scores sit under an
/// app's name match: an exact label 0.85, a label prefix 0.75, other label matches 0.55, a
/// keyword 0.4.
pub fn search(rows: &[Row], query: &str) -> Vec<ProviderItem> {
    let q: String = query.trim().chars().take(MAX_QUERY).collect();
    if q.chars().count() < 2 {
        return Vec::new();
    }
    let ql = q.to_lowercase();
    // A match is the query in the text, or each of its words starting a word of the text; the
    // fuzzy score only orders those. A bare subsequence matches nearly any sentence ("fire" in
    // "How far to swipe before it switches").
    let words: Vec<&str> = ql.split_whitespace().collect();
    let matches = |text: &str| -> Option<u32> {
        let t = text.to_lowercase();
        let ok = t.contains(&ql)
            || words.iter().all(|w| {
                t.split(|c: char| !c.is_alphanumeric())
                    .any(|tw| tw.starts_with(w))
            });
        if ok { score(&q, text) } else { None }
    };
    let mut hits: Vec<(f64, u32, &Row)> = rows
        .iter()
        .filter_map(|r| {
            if let Some(s) = matches(&r.label) {
                let l = r.label.to_lowercase();
                let band = if l == ql {
                    0.85
                } else if l.starts_with(&ql) {
                    0.75
                } else {
                    0.55
                };
                return Some((band, s, r));
            }
            r.keywords
                .iter()
                .filter_map(|k| matches(k))
                .max()
                .map(|s| (0.4, s, r))
        })
        .collect();
    hits.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(b.1.cmp(&a.1))
            .then_with(|| a.2.label.cmp(&b.2.label))
    });
    hits.into_iter()
        .take(MAX_ROWS)
        .map(|(band, _, r)| ProviderItem {
            id: format!("{}{SEP}{}", r.page, r.setting),
            title: r.label.clone(),
            subtitle: if r.setting.is_empty() {
                "Settings page".into()
            } else {
                format!("Settings › {}", r.page_name)
            },
            icon: if super::providers::valid_icon(&r.icon) {
                r.icon.clone()
            } else {
                "preferences-system".into()
            },
            score: band,
            actions: Vec::new(),
        })
        .collect()
}

/// A row id back into `(page, setting)`, if it is one of `rows`.
pub fn target<'a>(rows: &'a [Row], id: &str) -> Option<(&'a str, &'a str)> {
    let (page, setting) = id.split_once(SEP)?;
    rows.iter()
        .find(|r| r.page == page && r.setting == setting)
        .map(|r| (r.page.as_str(), r.setting.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(page: &str, setting: &str, label: &str, kw: &[&str]) -> Row {
        Row {
            page: page.into(),
            setting: setting.into(),
            label: label.into(),
            page_name: "Page".into(),
            icon: "preferences-desktop".into(),
            keywords: kw.iter().map(|k| k.to_string()).collect(),
        }
    }

    #[test]
    fn labels_rank_above_keywords() {
        let rows = vec![
            row("look", "cursor", "Cursor size", &[]),
            row("input", "", "Input", &["cursor", "mouse"]),
            row("wallpaper", "", "Wallpaper", &[]),
        ];
        let hits = search(&rows, "cursor");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].title, "Cursor size");
        assert_eq!(hits[0].subtitle, "Settings › Page");
        assert_eq!(hits[1].title, "Input");
        assert_eq!(hits[1].subtitle, "Settings page");
        assert!(hits[0].score > hits[1].score);
        assert!(search(&rows, "w")[..].is_empty(), "one letter is too short");
        assert_eq!(search(&rows, "wallpaper")[0].score, 0.85);
    }

    #[test]
    fn scattered_letters_do_not_match() {
        let rows = vec![
            row(
                "input",
                "swipe",
                "How far to swipe before it switches",
                &["gesture"],
            ),
            row("look", "cursor", "Cursor size", &["pointer"]),
        ];
        assert!(search(&rows, "fire").is_empty());
        assert_eq!(
            search(&rows, "swipe")[0].title,
            "How far to swipe before it switches"
        );
        assert_eq!(search(&rows, "cur siz")[0].title, "Cursor size");
        assert_eq!(search(&rows, "point")[0].title, "Cursor size");
    }

    #[test]
    fn ids_round_trip_only_for_known_rows() {
        let rows = vec![row("look", "cursor", "Cursor size", &[])];
        let id = &search(&rows, "cursor")[0].id;
        assert_eq!(target(&rows, id), Some(("look", "cursor")));
        assert_eq!(target(&rows, "look\u{1f}nope"), None);
        assert_eq!(target(&rows, "look"), None);
    }

    #[test]
    fn parses_the_index_or_nothing() {
        let json = br#"[{"page":"a","setting":"","label":"A","pageName":"A","icon":"x","keywords":["k"]}]"#;
        assert_eq!(parse(json).len(), 1);
        assert!(parse(b"not json").is_empty());
        assert!(parse(&vec![b' '; MAX_BYTES + 1]).is_empty());
    }
}
