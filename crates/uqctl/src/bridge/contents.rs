//! Spotlight's Contents group (`files:contents`): files whose text holds the query.
//!
//! When KDE's Baloo indexes the files (its index exists, it isn't switched off and
//! `baloosearch6` is installed), the matches come from Baloo, which also reads PDFs and office
//! files and answers in milliseconds. Without it, the text files already in the files index (code
//! and plain-text documents up to [`MAX_TEXT`], the recently changed first) are read until
//! [`SCAN_BUDGET`] runs out or a newer search starts.
//!
//! A file whose name already matches is left to its type's group. A row shows the first line
//! that matches, where the file's text can be read as it is, and its folder.

use super::calc::ProviderItem;
use super::files::{self, Index, Kind};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const PROVIDER: &str = "files:contents";
pub const NAME: &str = "Contents";
/// Rows shown.
const ROWS: usize = 5;
/// Shorter queries match too much text to be useful.
const MIN_CHARS: usize = 3;
const MAX_QUERY: usize = 128;
/// Baloo's rows (before names are left out) and how long it may take.
const BALOO_LIMIT: usize = 40;
const BALOO_TIMEOUT: Duration = Duration::from_millis(300);
/// How long the scan without Baloo may read files.
pub const SCAN_BUDGET: Duration = Duration::from_millis(150);
/// Larger files are not read, by the scan or for a row's line.
pub const MAX_TEXT: u64 = 1024 * 1024;
/// A row's line, in characters.
const SNIPPET: usize = 80;
/// Below every name match: a file found by its text is a weaker hint than one found by name.
const SCORE: f64 = 0.35;

/// The query, lowercased, with its words; None when it's too short (or too long).
struct Query {
    phrase: String,
    words: Vec<String>,
}

impl Query {
    fn new(query: &str) -> Option<Query> {
        let phrase = query.trim().to_lowercase();
        let n = phrase.chars().count();
        if !(MIN_CHARS..=MAX_QUERY).contains(&n) {
            return None;
        }
        let words = phrase.split_whitespace().map(str::to_string).collect();
        Some(Query { phrase, words })
    }

    /// Whether the file's name alone matches: its type's group shows it already.
    fn names(&self, path: &str) -> bool {
        let name = Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        self.words.iter().all(|w| name.contains(w.as_str()))
    }
}

/// `baloosearch6`, when Baloo has an index and isn't switched off.
fn baloo() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let xdg = |var: &str, dflt: &str| {
        std::env::var_os(var)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(dflt))
    };
    if !xdg("XDG_DATA_HOME", ".local/share")
        .join("baloo/index")
        .is_file()
    {
        return None;
    }
    let rc = std::fs::read_to_string(xdg("XDG_CONFIG_HOME", ".config").join("baloofilerc"))
        .unwrap_or_default();
    if rc.lines().any(|l| {
        l.replace(' ', "")
            .eq_ignore_ascii_case("Indexing-Enabled=false")
    }) {
        return None;
    }
    files::on_path("baloosearch6")
}

/// Baloo's matches for `q`: paths of files that exist, in Baloo's order.
async fn ask_baloo(bin: &Path, q: &Query) -> Option<Vec<String>> {
    let run = tokio::process::Command::new(bin)
        .args(["-l", &BALOO_LIMIT.to_string(), "--", &q.phrase])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(BALOO_TIMEOUT, run).await.ok()?.ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| l.starts_with('/'))
            .map(str::to_string)
            .collect(),
    )
}

/// The file's text, when it can be read as it is and isn't too large.
fn text_of(path: &str) -> Option<String> {
    let f = std::fs::File::open(path).ok()?;
    let meta = f.metadata().ok()?;
    if !meta.is_file() || meta.len() > MAX_TEXT {
        return None;
    }
    let mut buf = Vec::with_capacity(meta.len() as usize);
    f.take(MAX_TEXT).read_to_end(&mut buf).ok()?;
    // Binary data is not text, whatever the name says.
    if buf.contains(&0) {
        return None;
    }
    String::from_utf8(buf).ok()
}

/// The first line holding the phrase (or, failing that, its first word), trimmed to
/// [`SNIPPET`] characters around the match; control characters and runs of spaces collapse.
fn snippet(text: &str, q: &Query) -> Option<String> {
    let first = q.words.first()?;
    let find = |needle: &str| {
        text.lines().find_map(|l| {
            let low = l.to_lowercase();
            // Lowercasing can change a line's length (rarely): then start at the line's start.
            let at = low.find(needle)?;
            let chars_before = if low.len() == l.len() {
                l.get(..at).map_or(0, |s| s.chars().count())
            } else {
                0
            };
            Some((l, chars_before))
        })
    };
    let (line, at) = find(&q.phrase).or_else(|| find(first))?;
    let clean: Vec<char> = line
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .collect();
    let lead = line.chars().take_while(|c| c.is_whitespace()).count();
    let at = at.saturating_sub(lead);
    let start = at.saturating_sub(SNIPPET / 3).min(clean.len());
    let end = (start + SNIPPET).min(clean.len());
    let mut s: String = clean[start..end].iter().collect();
    if start > 0 {
        s.insert(0, '…');
    }
    if end < clean.len() {
        s.push('…');
    }
    (!s.is_empty()).then_some(s)
}

/// A row: the file's name; its matching line and folder.
fn row(path: &str, q: &Query, text: Option<&str>, rank: usize) -> ProviderItem {
    let name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);
    let mut r = files::row(Kind::of_file(name), path, SCORE - rank as f64 * 0.01);
    if let Some(line) = text.and_then(|t| snippet(t, q)) {
        r.subtitle = format!("“{line}” · {}", r.subtitle);
    }
    r
}

/// Rows for Baloo's matches: files only, not hidden, not already shown by name.
fn from_baloo(paths: Vec<String>, q: &Query) -> Vec<ProviderItem> {
    paths
        .into_iter()
        .filter(|p| !p.split('/').any(|seg| seg.starts_with('.')))
        .filter(|p| !q.names(p))
        .filter(|p| Path::new(p).is_file())
        .take(ROWS)
        .enumerate()
        .map(|(i, p)| {
            let text = files::is_text(&p).then(|| text_of(&p)).flatten();
            row(&p, q, text.as_deref(), i)
        })
        .collect()
}

/// Read the index's text files for the phrase until `budget` runs out, ROWS are found or
/// `stale()` says a newer search started.
fn scan(idx: &Index, q: &Query, budget: Duration, stale: &dyn Fn() -> bool) -> Vec<ProviderItem> {
    let until = Instant::now() + budget;
    let mut out = Vec::new();
    for (n, path) in idx.text_files().enumerate() {
        if out.len() >= ROWS || Instant::now() >= until || (n % 64 == 0 && stale()) {
            break;
        }
        if q.names(path) {
            continue;
        }
        let Some(text) = text_of(path) else {
            continue;
        };
        if text.to_lowercase().contains(&q.phrase) {
            out.push(row(path, q, Some(&text), out.len()));
        }
    }
    out
}

/// The Contents rows for `query`: from Baloo when it indexes, else a scan of `idx` (`stale`:
/// whether a newer search started, so the scan can stop early).
pub async fn search(
    query: &str,
    idx: Arc<Index>,
    stale: impl Fn() -> bool + Send + 'static,
) -> Vec<ProviderItem> {
    let Some(q) = Query::new(query) else {
        return Vec::new();
    };
    if let Some(bin) = baloo()
        && let Some(paths) = ask_baloo(&bin, &q).await
    {
        return tokio::task::spawn_blocking(move || from_baloo(paths, &q))
            .await
            .unwrap_or_default();
    }
    tokio::task::spawn_blocking(move || scan(&idx, &q, SCAN_BUDGET, &stale))
        .await
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::SystemTime;

    fn q(s: &str) -> Query {
        Query::new(s).unwrap()
    }

    #[test]
    fn short_queries_search_nothing() {
        assert!(Query::new("ab").is_none());
        assert!(Query::new(&"x".repeat(MAX_QUERY + 1)).is_none());
        assert!(Query::new(" abc ").is_some());
    }

    #[test]
    fn names_that_match_are_left_to_their_group() {
        assert!(q("fire wall").names("/x/firewall-notes.md"));
        assert!(!q("firewall").names("/x/notes.md"));
    }

    #[test]
    fn snippets_show_the_matching_line_around_the_match() {
        let text = "first line\n\t  the Firewall\x07 rules   are here\nlast";
        assert_eq!(
            snippet(text, &q("firewall")).as_deref(),
            Some("the Firewall rules are here")
        );
        // The phrase isn't there: the first word's line.
        assert_eq!(
            snippet(text, &q("rules nowhere")).as_deref(),
            Some("the Firewall rules are here")
        );
        assert_eq!(snippet(text, &q("absent")), None);
        let long = format!("{} needle {}", "a".repeat(200), "b".repeat(200));
        let s = snippet(&long, &q("needle")).unwrap();
        assert!(s.starts_with('…') && s.ends_with('…'), "{s}");
        assert!(s.contains("needle"), "{s}");
        assert_eq!(s.chars().count(), SNIPPET + 2);
    }

    #[test]
    fn binary_and_large_files_are_not_text() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("a.rs");
        fs::write(&bin, b"fn main\0()").unwrap();
        assert_eq!(text_of(bin.to_str().unwrap()), None);
        let big = dir.path().join("b.md");
        fs::write(&big, vec![b'a'; MAX_TEXT as usize + 1]).unwrap();
        assert_eq!(text_of(big.to_str().unwrap()), None);
        let ok = dir.path().join("c.md");
        fs::write(&ok, "hello").unwrap();
        assert_eq!(text_of(ok.to_str().unwrap()).as_deref(), Some("hello"));
    }

    #[test]
    fn the_scan_finds_text_in_text_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        fs::write(d.join("notes.md"), "Remember the Quarterly budget\n").unwrap();
        fs::write(
            d.join("main.rs"),
            "// quarterly budget maths\nfn main() {}\n",
        )
        .unwrap();
        fs::write(d.join("quarterly-budget.txt"), "quarterly budget").unwrap(); // its name matches
        fs::write(d.join("photo.png"), "quarterly budget").unwrap(); // not text
        fs::write(d.join("other.md"), "nothing here").unwrap();
        let idx = files::build(&[d.to_path_buf()], SystemTime::now());
        let rows = scan(
            &idx,
            &q("quarterly budget"),
            Duration::from_secs(5),
            &|| false,
        );
        let mut names: Vec<&str> = rows.iter().map(|r| r.title.as_str()).collect();
        names.sort();
        assert_eq!(names, ["main.rs", "notes.md"]);
        let notes = rows.iter().find(|r| r.title == "notes.md").unwrap();
        assert!(
            notes
                .subtitle
                .starts_with("“Remember the Quarterly budget” · "),
            "{}",
            notes.subtitle
        );
        assert!(rows.iter().all(|r| r.score < 0.4));
        // A newer search stops it at once.
        assert!(
            scan(
                &idx,
                &q("quarterly budget"),
                Duration::from_secs(5),
                &|| true
            )
            .is_empty()
        );
    }
}
