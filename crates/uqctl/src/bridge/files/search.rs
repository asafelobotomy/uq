//! Scoring names against a query and searching the index and the system for files.

use super::*;

/// How well a lowercased name matches a lowercased query: whole name 1.0, the name without its
/// extension 0.95, a prefix 0.85, a word start 0.7, anywhere 0.5; None for no match.
pub(crate) fn name_score(q: &str, name: &str) -> Option<f64> {
    if name == q {
        return Some(1.0);
    }
    if name.rsplit_once('.').is_some_and(|(stem, _)| stem == q) {
        return Some(0.95);
    }
    if name.starts_with(q) {
        return Some(0.85);
    }
    let at = name.find(q)?;
    let before = name[..at].chars().next_back();
    if before.is_some_and(|c| !c.is_alphanumeric()) {
        Some(0.7)
    } else {
        Some(0.5)
    }
}

/// How well a name matches a query of one or more words: one word as [`name_score`]; several, every
/// word must be in the name, in any order and across `-`, `_` and spaces ("tax return" finds
/// `Tax_Return-2026.pdf`), scored a little under the weakest word.
pub(crate) fn query_score(words: &[&str], name: &str) -> Option<f64> {
    match words {
        [] => None,
        [one] => name_score(one, name),
        many => many
            .iter()
            .map(|w| name_score(w, name))
            .collect::<Option<Vec<f64>>>()
            .map(|s| s.into_iter().fold(1.0, f64::min) * 0.9),
    }
}

/// The best [`PER_KIND`] rows of each type for `query`, as provider rows. Scores sit under an
/// app's name match: at most 0.8.
pub fn search(idx: &Index, query: &str) -> Vec<(Kind, Vec<ProviderItem>)> {
    let q = query.trim().to_lowercase();
    if q.chars().count() < 2 {
        return Vec::new();
    }
    let words: Vec<&str> = q.split_whitespace().collect();
    let mut per: Vec<Vec<(f64, &Entry)>> = vec![Vec::new(); Kind::ALL.len()];
    for e in &idx.entries {
        let Some(s) = query_score(&words, &e.name) else {
            continue;
        };
        let s = s * 0.75 + if e.recent { 0.05 } else { 0.0 };
        let bucket = &mut per[e.kind as usize];
        // Keep the best PER_KIND, cheaply: a short sorted list.
        if bucket.len() < PER_KIND || bucket.last().is_some_and(|(b, _)| s > *b) {
            let at = bucket.partition_point(|(b, x)| *b > s || (*b == s && x.path < e.path));
            bucket.insert(at, (s, e));
            bucket.truncate(PER_KIND);
        }
    }
    Kind::ALL
        .iter()
        .zip(per)
        .filter(|(_, rows)| !rows.is_empty())
        .map(|(k, rows)| {
            (
                *k,
                rows.into_iter().map(|(s, e)| row(*k, &e.path, s)).collect(),
            )
        })
        .collect()
}

/// A provider row for a path: the name, then the folder it is in (with `~` for home).
pub fn row(kind: Kind, path: &str, score: f64) -> ProviderItem {
    let p = Path::new(path);
    let name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string();
    let parent = p
        .parent()
        .and_then(|d| d.to_str())
        .unwrap_or("")
        .to_string();
    let home = std::env::var("HOME").unwrap_or_default();
    let parent = match parent.strip_prefix(&home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => parent,
    };
    ProviderItem {
        id: path.to_string(),
        title: name,
        subtitle: parent,
        icon: kind.icon().into(),
        score,
        actions: Vec::new(),
    }
}

/// Files elsewhere on the system whose name holds `query`, from plocate's database: not under
/// `roots` (the index has those), not hidden. At most [`SYSTEM_ROWS`].
pub async fn system(query: &str, roots: &[PathBuf]) -> Vec<ProviderItem> {
    let q = query.trim();
    let Some(bin) = plocate() else {
        return Vec::new();
    };
    if q.chars().count() < 3 {
        return Vec::new();
    }
    let run = tokio::process::Command::new(bin)
        .args(["-i", "-b", "-l", &PLOCATE_LIMIT.to_string(), "--", q])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let Ok(Ok(out)) = tokio::time::timeout(PLOCATE_TIMEOUT, run).await else {
        return Vec::new();
    };
    let ql = q.to_lowercase();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with('/'))
        .filter(|l| !roots.iter().any(|r| Path::new(l).starts_with(r)))
        .filter(|l| !l.split('/').any(|seg| seg.starts_with('.')))
        .take(SYSTEM_ROWS)
        .map(|l| {
            let name = Path::new(l)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();
            let s = name_score(&ql, &name).unwrap_or(0.5) * 0.6;
            row(Kind::System, l, s)
        })
        .collect()
}
