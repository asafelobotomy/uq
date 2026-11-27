//! Finding where `user.lua` sets something: rules, workspace rules, gestures, env, binds and
//! options. Hints for explain, not a Lua parser.

pub(crate) fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Whether `needle` occurs in `line` as a whole word (no identifier character on either side).
pub(crate) fn contains_word(line: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(i) = line[from..].find(needle) {
        let start = from + i;
        let end = start + needle.len();
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        if !before.is_some_and(is_ident) && !after.is_some_and(is_ident) {
            return true;
        }
        from = start + needle.chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// Tables nested deeper than this are counted but not named.
pub(crate) const MAX_TABLE_DEPTH: usize = 64;

/// The part of a `user.lua` line before a `--` comment. Strings are not parsed, so `--` inside
/// one also ends the scanned part.
pub(crate) fn strip_lua_comment(line: &str) -> &str {
    line.find("--").map_or(line, |i| &line[..i])
}

/// The 1-based line of `user.lua` that names uQ's rule `id` (`name = "uq:<id>"`): the text
/// `uq:<id>` followed by `"`, `'` or `]`. Comments are ignored.
pub fn find_rule_in_user_lua(text: &str, id: &str) -> Option<u32> {
    let marker = format!("uq:{id}");
    text.lines().enumerate().find_map(|(n, line)| {
        let line = strip_lua_comment(line);
        let mut from = 0;
        while let Some(i) = line[from..].find(&marker) {
            let end = from + i + marker.len();
            if matches!(line[end..].chars().next(), Some('"' | '\'' | ']')) {
                return Some(u32::try_from(n + 1).unwrap_or(u32::MAX));
            }
            from += i + 1;
        }
        None
    })
}

/// The 1-based line of `user.lua` with a `workspace_rule` call that names `selector` in quotes.
/// Comments are ignored.
pub fn find_workspace_rule_in_user_lua(text: &str, selector: &str) -> Option<u32> {
    let (dq, sq) = (format!("\"{selector}\""), format!("'{selector}'"));
    text.lines().enumerate().find_map(|(n, line)| {
        let line = strip_lua_comment(line);
        (line.contains("workspace_rule") && (line.contains(&dq) || line.contains(&sq)))
            .then(|| u32::try_from(n + 1).unwrap_or(u32::MAX))
    })
}

/// The 1-based line of `user.lua` with an `hl.gesture(` call. Comments are ignored. uQ's own
/// gestures render in `generated.lua`, which loads first, so Hyprland refuses a gesture here
/// that one of them shadows (and `--verify-config` fails for every later uQ write).
pub fn find_gesture_in_user_lua(text: &str) -> Option<u32> {
    text.lines().enumerate().find_map(|(n, line)| {
        let line = strip_lua_comment(line);
        let mut from = 0;
        while let Some(i) = line[from..].find("hl.gesture") {
            let start = from + i;
            from = start + 1;
            if line[..start].chars().next_back().is_some_and(is_ident) {
                continue;
            }
            if line[start + "hl.gesture".len()..]
                .trim_start()
                .starts_with('(')
            {
                return Some(u32::try_from(n + 1).unwrap_or(u32::MAX));
            }
        }
        None
    })
}

/// The 1-based line of `user.lua` with an `hl.env(` call that names variable `name` in quotes.
/// Comments are ignored.
pub fn find_env_in_user_lua(text: &str, name: &str) -> Option<u32> {
    text.lines().enumerate().find_map(|(n, line)| {
        let line = strip_lua_comment(line);
        let mut from = 0;
        while let Some(i) = line[from..].find("hl.env") {
            let start = from + i;
            from = start + 1;
            if line[..start].chars().next_back().is_some_and(is_ident) {
                continue;
            }
            let Some(rest) = line[start + "hl.env".len()..]
                .trim_start()
                .strip_prefix('(')
            else {
                continue;
            };
            let rest = rest.trim_start();
            let Some(q) = rest.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
                continue;
            };
            if rest[1..]
                .strip_prefix(name)
                .is_some_and(|r| r.starts_with(q))
            {
                return Some(u32::try_from(n + 1).unwrap_or(u32::MAX));
            }
        }
        None
    })
}

/// The first string literal of the call that starts at `args` (the text after `hl.bind(`), when
/// the call's first argument is a plain `"…"` or `'…'`. `mainMod .. " + Q"` has none.
pub(crate) fn first_string(args: &str) -> Option<&str> {
    let rest = args.trim_start();
    let q = rest.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let body = &rest[1..];
    body.find(q).map(|end| &body[..end])
}

/// The first `hl.bind(` or `hl.unbind(` call on `line` (comments already cut), as
/// `(is_unbind, first string argument)`. `hl.bind` must not be the tail of a longer name.
pub(crate) fn bind_call(line: &str) -> Option<(bool, &str)> {
    for (name, unbind) in [("hl.bind", false), ("hl.unbind", true)] {
        let mut from = 0;
        while let Some(i) = line[from..].find(name) {
            let start = from + i;
            from = start + 1;
            if line[..start].chars().next_back().is_some_and(is_ident) {
                continue;
            }
            let Some(args) = line[start + name.len()..].trim_start().strip_prefix('(') else {
                continue;
            };
            if let Some(s) = first_string(args) {
                return Some((unbind, s));
            }
        }
    }
    None
}

/// The 1-based line of `user.lua` with an `hl.bind(` or `hl.unbind(` call whose first string
/// literal is the same shortcut as `combo` (`WIN + q` is `SUPER + Q`), or an `hl.unbind("all")`
/// line, which overrides every shortcut. Comments are ignored; a key built with `..` doesn't
/// parse, so it never hits. A hint, not a parser.
pub fn find_bind_in_user_lua(text: &str, combo: crate::keybinds::Combo) -> Option<u32> {
    text.lines().enumerate().find_map(|(n, line)| {
        let (unbind, keys) = bind_call(strip_lua_comment(line))?;
        let hit = (unbind && keys == "all")
            || crate::keybinds::combo::parse(keys).is_ok_and(|c| c == combo);
        hit.then(|| u32::try_from(n + 1).unwrap_or(u32::MAX))
    })
}

/// A `user.lua` bind the user marked `-- uq:bind` (or `-- uq:bind Label`), listed read-only.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkedBind {
    pub line: u32,
    /// The shortcut in canonical form, when the first argument is a plain string that parses.
    pub combo: Option<String>,
    /// The label after the marker, else the line itself cut to 120 characters.
    pub text: String,
}

pub(crate) const MAX_MARKED: usize = 100;
pub(crate) const MARKED_TEXT_CHARS: usize = 120;

/// The text after a `-- uq:bind` marker on `line` (its trimmed label, possibly empty) and the
/// code before it.
pub(crate) fn marker(line: &str) -> Option<(&str, &str)> {
    // The first `--` starts the comment, as `strip_lua_comment` has it.
    let at = line.find("--")?;
    let label = line[at + 2..].trim_start().strip_prefix("uq:bind")?;
    (label.is_empty() || label.starts_with(char::is_whitespace))
        .then(|| (&line[..at], label.trim()))
}

/// Every line with an `hl.bind(` call and a `-- uq:bind` comment, at most 100.
pub fn marked_binds(text: &str) -> Vec<MarkedBind> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let Some((code, label)) = marker(line) else {
            continue;
        };
        if !code.contains("hl.bind") {
            continue;
        }
        let combo = bind_call(code)
            .filter(|(unbind, _)| !unbind)
            .and_then(|(_, keys)| crate::keybinds::combo::parse(keys).ok())
            .map(crate::keybinds::combo::canonical);
        let text = if label.is_empty() {
            crate::settings::clean_text_spaced(line.trim(), MARKED_TEXT_CHARS)
        } else {
            crate::settings::clean_text_spaced(label, MARKED_TEXT_CHARS)
        };
        out.push(MarkedBind {
            line: u32::try_from(n + 1).unwrap_or(u32::MAX),
            combo,
            text,
        });
        if out.len() == MAX_MARKED {
            break;
        }
    }
    out
}

/// The 1-based line of `user.lua` text that seems to set Hyprland option `hypr_path`
/// (`general:gaps_in`, `decoration:blur:enabled`). A case-sensitive, plain-text heuristic, not
/// a parser; the first hit wins. A line hits when it has the path in dotted form
/// (`general.gaps_in`) or colon form (`general:gaps_in`), or has `gaps_in =` inside a
/// `general = {` table (tables are tracked by brace depth, so `blur = { enabled = … }` inside
/// `decoration = {` finds `decoration:blur:enabled`). Lua `--` comments are ignored (a `--[[`
/// block comment only on its first line); strings are not parsed, so `--` inside one also ends
/// the scanned part of that line.
pub fn find_in_user_lua(text: &str, hypr_path: &str) -> Option<u32> {
    let (section, leaf) = hypr_path.rsplit_once(':')?;
    let sections: Vec<&str> = section.split(':').collect();
    let dotted = hypr_path.replace(':', ".");
    // One entry per open brace: the table's name when it is `name = {`.
    let mut stack: Vec<Option<&str>> = Vec::new();
    let mut deep = 0usize;
    for (n, line) in text.lines().enumerate() {
        let lineno = u32::try_from(n + 1).unwrap_or(u32::MAX);
        let line = strip_lua_comment(line);
        if contains_word(line, &dotted) || contains_word(line, hypr_path) {
            return Some(lineno);
        }
        let mut rest = line;
        while let Some(c) = rest.chars().next() {
            if c.is_ascii_alphabetic() || c == '_' {
                let end = rest.find(|c: char| !is_ident(c)).unwrap_or(rest.len());
                let ident = &rest[..end];
                rest = &rest[end..];
                let after = rest.trim_start();
                if after.starts_with('=') && !after.starts_with("==") {
                    let value = after[1..].trim_start();
                    if let Some(inner) = value.strip_prefix('{') {
                        if stack.len() < MAX_TABLE_DEPTH {
                            stack.push(Some(ident));
                        } else {
                            deep += 1;
                        }
                        rest = inner;
                    } else {
                        let named: Vec<&str> = stack.iter().flatten().copied().collect();
                        if ident == leaf && named.ends_with(&sections) {
                            return Some(lineno);
                        }
                        rest = value;
                    }
                }
                continue;
            }
            match c {
                '{' if stack.len() < MAX_TABLE_DEPTH => stack.push(None),
                '{' => deep += 1,
                '}' if deep > 0 => deep -= 1,
                '}' => {
                    stack.pop();
                }
                _ => {}
            }
            rest = &rest[c.len_utf8()..];
        }
    }
    None
}
