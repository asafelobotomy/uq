//! Editing `mimeapps.list` without touching anything but the keys being set or cleared: every
//! other line, comment, group and key stays exactly as it was.

use crate::mimeapps::GROUP;

/// `text` with each of `mimes` set to `id` in `[Default Applications]`: an existing key is
/// replaced where it is (a repeat of it is dropped), a missing one is added at the end of the
/// group, and the group is added at the end when there is none.
pub fn set_defaults<S: AsRef<str>>(text: &str, mimes: &[S], id: &str) -> String {
    let mimes: Vec<&str> = mimes.iter().map(AsRef::as_ref).collect();
    let mut out: Vec<String> = Vec::new();
    let mut inside = false;
    let mut seen_group = false;
    let mut done: Vec<&str> = Vec::new();
    // Where the group's last non-blank line is, to add missing keys after it.
    let mut group_end: Option<usize> = None;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            inside = t == GROUP;
            if inside {
                seen_group = true;
                group_end = Some(out.len() + 1);
            }
            out.push(line.to_string());
            continue;
        }
        if inside
            && !t.starts_with('#')
            && let Some((k, _)) = t.split_once('=')
            && let Some(m) = mimes.iter().find(|m| **m == k.trim())
        {
            if done.contains(m) {
                continue;
            }
            done.push(m);
            out.push(format!("{m}={id};"));
            group_end = Some(out.len());
            continue;
        }
        out.push(line.to_string());
        if inside && !t.is_empty() {
            group_end = Some(out.len());
        }
    }
    let missing: Vec<String> = mimes
        .iter()
        .filter(|m| !done.contains(m))
        .map(|m| format!("{m}={id};"))
        .collect();
    if !missing.is_empty() {
        match (seen_group, group_end) {
            (true, Some(at)) => {
                for (i, l) in missing.into_iter().enumerate() {
                    out.insert(at + i, l);
                }
            }
            _ => {
                if out.last().is_some_and(|l| !l.trim().is_empty()) {
                    out.push(String::new());
                }
                out.push(GROUP.to_string());
                out.extend(missing);
            }
        }
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// `text` without the `[Default Applications]` keys for `mimes`, so the next list down (the
/// system's) decides them. Everything else stays; `None` when nothing was there to remove.
pub fn remove_defaults<S: AsRef<str>>(text: &str, mimes: &[S]) -> Option<String> {
    let mut out: Vec<&str> = Vec::new();
    let mut inside = false;
    let mut removed = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            inside = t == GROUP;
        } else if inside
            && !t.starts_with('#')
            && let Some((k, _)) = t.split_once('=')
            && mimes.iter().any(|m| m.as_ref() == k.trim())
        {
            removed = true;
            continue;
        }
        out.push(line);
    }
    removed.then(|| {
        let mut s = out.join("\n");
        s.push('\n');
        s
    })
}
