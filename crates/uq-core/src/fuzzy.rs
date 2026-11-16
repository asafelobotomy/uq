//! Fuzzy matching shared by Settings' search and look=see>do_'s Settings results: a
//! case-insensitive subsequence match where word starts and runs of consecutive characters score
//! higher.

/// A match at a word start scores this much extra.
const WORD_START: u32 = 10;
/// Each matched character right after the previous one scores this much extra.
const CONSECUTIVE: u32 = 6;
const PER_CHAR: u32 = 1;

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
    if q.len() > raw.len() {
        return None;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsequence_and_word_start_rank_higher() {
        assert_eq!(score("zz", "Cursor size"), None);
        assert_eq!(score("sc", "Cursor"), None, "order matters");
        assert!(score("CUR", "cursor size").is_some(), "case-insensitive");
        assert!(score("cs", "Cursor size").unwrap() > score("cs", "Facsimile").unwrap());
        assert!(score("cur", "Cursor").unwrap() > score("cur", "Cool user area").unwrap());
        assert_eq!(score("", "x"), None);
    }
}
