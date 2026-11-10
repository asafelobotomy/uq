//! Fuzzy search over page names, setting labels and keywords (spec §2.2).

use serde::Serialize;

use crate::registry::PageEntry;

/// Added to every label match so a label always ranks above a keyword match.
const LABEL_BONUS: u32 = 10_000;
const WORD_START: u32 = 10;
const CONSECUTIVE: u32 = 6;
const PER_CHAR: u32 = 1;
pub const MAX_HITS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub page_id: String,
    /// Empty for a hit on the page itself.
    pub setting_id: String,
    pub label: String,
    pub page_name: String,
    pub score: u32,
}

/// A case-insensitive subsequence match of `query` in `text` (spaces in the query are ignored).
/// A match at a word start and consecutive matched characters score extra; the best placement
/// wins. `None` when `query` is not a subsequence of `text`.
pub fn score(query: &str, text: &str) -> Option<u32> {
    let q: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if q.is_empty() {
        return None;
    }
    let raw: Vec<char> = text.chars().collect();
    let t: Vec<char> = raw.iter().flat_map(|c| c.to_lowercase()).collect();
    if t.len() != raw.len() {
        // A character that lowercases to several: fall back to the plain comparison.
        return None;
    }
    let word_start = |j: usize| {
        j == 0
            || !raw[j - 1].is_alphanumeric()
            || (raw[j - 1].is_lowercase() && raw[j].is_uppercase())
    };
    // best[j]: the best score with the previous query character matched at text index j.
    let mut best: Vec<Option<u32>> = vec![None; t.len()];
    for (i, qc) in q.iter().enumerate() {
        let mut next: Vec<Option<u32>> = vec![None; t.len()];
        // The best score among earlier positions k < j, kept as j advances.
        let mut earlier: Option<u32> = if i == 0 { Some(0) } else { None };
        for j in 0..t.len() {
            if t[j] == *qc {
                let mut from = earlier;
                if i > 0
                    && j > 0
                    && let Some(prev) = best[j - 1]
                {
                    from = from.max(Some(prev + CONSECUTIVE));
                }
                if let Some(base) = from {
                    let bonus = if word_start(j) { WORD_START } else { 0 };
                    next[j] = Some(base + PER_CHAR + bonus);
                }
            }
            if i > 0
                && j > 0
                && let Some(b) = best[j - 1]
            {
                earlier = earlier.max(Some(b));
            }
        }
        best = next;
    }
    best.into_iter().flatten().max()
}

/// Hits for `query` over every page and setting: labels count over keywords, sorted by score
/// (best first) then label, at most [`MAX_HITS`]. An empty query returns nothing.
pub fn search(pages: &[PageEntry], query: &str) -> Vec<Hit> {
    let mut hits = Vec::new();
    for page in pages {
        let mut add = |setting_id: &str, label: &str, keywords: &[String]| {
            if let Some(score) = rank(query, label, keywords) {
                hits.push(Hit {
                    page_id: page.id.clone(),
                    setting_id: setting_id.into(),
                    label: label.into(),
                    page_name: page.name.clone(),
                    score,
                });
            }
        };
        add("", &page.name, &page.keywords);
        for s in &page.settings {
            add(&s.id, &s.label, &s.keywords);
        }
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.label.cmp(&b.label)));
    hits.truncate(MAX_HITS);
    hits
}

/// A label match scores above any keyword match.
fn rank(query: &str, label: &str, keywords: &[String]) -> Option<u32> {
    if let Some(s) = score(query, label) {
        return Some(LABEL_BONUS + s);
    }
    keywords.iter().filter_map(|k| score(query, k)).max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Category, OwnedSearchEntry};

    fn entry(id: &str, label: &str, keywords: &[&str]) -> OwnedSearchEntry {
        OwnedSearchEntry {
            id: id.into(),
            label: label.into(),
            keywords: keywords.iter().map(|k| k.to_string()).collect(),
        }
    }

    fn page(id: &str, name: &str, settings: Vec<OwnedSearchEntry>) -> PageEntry {
        PageEntry {
            id: id.into(),
            name: name.into(),
            icon: "x".into(),
            category: Category::Personal,
            qml: "x.qml".into(),
            plugin: false,
            keywords: Vec::new(),
            settings,
        }
    }

    #[test]
    fn subsequence_and_word_start_rank_higher() {
        assert_eq!(score("zz", "Cursor size"), None);
        assert_eq!(score("sc", "Cursor"), None, "order matters");
        assert!(score("CUR", "cursor size").is_some(), "case-insensitive");
        let word_start = score("cs", "Cursor size").unwrap(); // C(ursor) S(ize)
        let mid_word = score("cs", "Facsimile").unwrap();
        assert!(word_start > mid_word);
        let consecutive = score("cur", "Cursor").unwrap();
        let spread = score("cur", "Cool user area").unwrap();
        assert!(consecutive > spread);
        assert!(score("ui", "UI font").unwrap() > score("ui", "Quit").unwrap());
    }

    #[test]
    fn keywords_match_but_rank_below_labels() {
        let pages = vec![page(
            "appearance",
            "Appearance",
            vec![
                entry("mono_font", "Monospace font", &["terminal", "code"]),
                entry("size", "Text size", &["font", "scale"]),
            ],
        )];
        let hits = search(&pages, "font");
        let labels: Vec<_> = hits.iter().map(|h| h.label.as_str()).collect();
        assert_eq!(labels, ["Monospace font", "Text size"]);
        assert!(hits[0].score > hits[1].score);
        // A keyword-only match is found, and carries the setting and page.
        let hits = search(&pages, "terminal");
        assert_eq!(hits.len(), 1);
        assert_eq!(
            (hits[0].page_id.as_str(), hits[0].setting_id.as_str()),
            ("appearance", "mono_font")
        );
        assert_eq!(hits[0].page_name, "Appearance");
    }

    #[test]
    fn empty_query_returns_nothing() {
        let pages = vec![page("a", "Alpha", vec![entry("s", "Setting", &[])])];
        assert!(search(&pages, "").is_empty());
        assert!(search(&pages, "   ").is_empty());
    }

    #[test]
    fn caps_at_20() {
        let settings = (0..50)
            .map(|i| entry(&format!("s{i}"), &format!("Option {i:02}"), &[]))
            .collect();
        let pages = vec![page("p", "Page", settings)];
        let hits = search(&pages, "opt");
        assert_eq!(hits.len(), 20);
        let labels: Vec<_> = hits.iter().map(|h| h.label.clone()).collect();
        let mut sorted = labels.clone();
        sorted.sort();
        assert_eq!(labels, sorted, "equal scores sort by label");
    }

    #[test]
    fn a_page_is_found_by_name_and_keyword() {
        let mut p = page("wallpaper", "Wallpaper", vec![]);
        p.keywords = vec!["background".into()];
        let hits = search(&[p], "backgr");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].setting_id, "");
        assert_eq!(hits[0].label, "Wallpaper");
    }
}
