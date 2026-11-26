//! Editing desktop files key by key, keeping the rest of the text.

use super::*;

pub(crate) fn group_of(line: &str) -> Option<&str> {
    let l = line.trim();
    l.strip_prefix('[').and_then(|r| r.strip_suffix(']'))
}

/// Set one key in a group: every line with that key is replaced, else the key is added at the end
/// of the group (or the group at the end of the file). All other lines stay byte for byte. The
/// value must already be escaped; a stray line break in it becomes a space.
pub fn set_key(text: &str, group: &str, key: &str, value: &str) -> String {
    let value = value.replace(['\n', '\r'], " ");
    let new_line = format!("{key}={value}");
    let mut out = String::with_capacity(text.len() + new_line.len() + 2);
    let mut in_group = false;
    let mut seen_group = false;
    let mut replaced = false;
    // Where in `out` a new key goes: just after the group's last non-blank line.
    let mut insert_at = 0;
    for raw in text.split_inclusive('\n') {
        let line = raw.trim_end_matches(['\n', '\r']);
        let eol = &raw[line.len()..];
        if let Some(g) = group_of(line) {
            in_group = g == group;
            seen_group |= in_group;
            out.push_str(raw);
            if in_group {
                insert_at = out.len();
            }
            continue;
        }
        let is_key = in_group
            && !line.trim_start().starts_with('#')
            && line.split_once('=').is_some_and(|(k, _)| k.trim() == key);
        if is_key {
            out.push_str(&new_line);
            out.push_str(if eol.is_empty() { "\n" } else { eol });
            replaced = true;
            insert_at = out.len();
            continue;
        }
        out.push_str(raw);
        if in_group && !line.trim().is_empty() {
            insert_at = out.len();
        }
    }
    if replaced {
        return out;
    }
    if seen_group {
        let mut ins = String::new();
        if !out[..insert_at].ends_with('\n') {
            ins.push('\n');
        }
        ins.push_str(&new_line);
        ins.push('\n');
        out.insert_str(insert_at, &ins);
        return out;
    }
    if !out.is_empty() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str(&format!("[{group}]\n{new_line}\n"));
    out
}

/// One key's value in a group, unescaped; the last occurrence wins, as in [`read_entry`].
pub fn get_key(text: &str, group: &str, key: &str) -> Option<String> {
    let mut in_group = false;
    let mut found = None;
    for line in text.lines() {
        if let Some(g) = group_of(line) {
            in_group = g == group;
            continue;
        }
        let line = line.trim();
        if !in_group || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=')
            && k.trim() == key
        {
            found = Some(v.trim());
        }
    }
    found.map(unescape_value)
}

/// `Desktop Entry` and each `Desktop Action *` group that has an `Exec` key, in file order.
pub fn groups_with_exec(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<&str> = None;
    for line in text.lines() {
        if let Some(g) = group_of(line) {
            cur = (g == "Desktop Entry" || g.starts_with("Desktop Action ")).then_some(g);
            continue;
        }
        if let Some(g) = cur
            && !line.trim_start().starts_with('#')
            && line
                .split_once('=')
                .is_some_and(|(k, _)| k.trim() == "Exec")
            && !out.iter().any(|o| o == g)
        {
            out.push(g.to_string());
        }
    }
    out
}
