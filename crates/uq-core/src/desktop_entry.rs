//! Desktop Entry handling shared by the bridge, the autostart runner and the settings pages: the
//! `Exec` parser and writer, the `[Desktop Entry]` reader, safe ids and a line-preserving key
//! setter. Pure text work; nothing here runs a process or touches the disk.

use crate::settings::valid_pinned_id;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExecError {
    #[error("empty Exec line")]
    Empty,
    #[error("unterminated quote in Exec")]
    Unterminated,
    #[error("deprecated field code %{0} in Exec")]
    Deprecated(char),
    #[error("unknown field code %{0} in Exec")]
    Unknown(char),
    #[error("dangling % at end of Exec")]
    DanglingPercent,
    #[error("single quotes are not supported; use double quotes")]
    SingleQuote,
}

/// One argument of an `Exec` line: literal text, or a field code kept as such.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Arg(String),
    Field(char),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens {
    pub tokens: Vec<Token>,
    /// Parsing and writing back would change the line: a field code inside a word was dropped
    /// from it, or an unquoted `'` was met (the format has no single quotes).
    pub lossy: bool,
}

/// What the expandable field codes stand for.
#[derive(Debug, Default, Clone, Copy)]
pub struct FieldCtx<'a> {
    pub icon: Option<&'a str>,
    pub name: Option<&'a str>,
    /// `%k`: the `.desktop` file's own location, not the entry's `Path=` working directory.
    pub path: Option<&'a str>,
}

#[derive(Default)]
struct Lexer {
    tokens: Vec<Token>,
    cur: String,
    // An argument exists once it has literal text or was quoted (so "" is an empty argument).
    live: bool,
    fields: Vec<char>,
    lossy: bool,
}

impl Lexer {
    fn flush(&mut self) {
        let cur = std::mem::take(&mut self.cur);
        let live = std::mem::take(&mut self.live);
        let fields = std::mem::take(&mut self.fields);
        if let ([c], true) = (fields.as_slice(), cur.is_empty()) {
            self.tokens.push(Token::Field(*c));
            return;
        }
        if !fields.is_empty() {
            self.lossy = true;
        }
        if live || !cur.is_empty() {
            self.tokens.push(Token::Arg(cur));
        }
    }
}

/// The shared lexer. Quoting uses `"`; inside quotes a backslash escapes one of `" ` $ \`.
/// With `fields`, `%%` is a literal `%`, the field codes are kept apart, and deprecated or
/// unknown codes are errors; without it `%` is literal.
fn lex(exec: &str, fields: bool) -> Result<Tokens, ExecError> {
    let mut lx = Lexer::default();
    let mut quoted = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                lx.live = true;
            }
            '\\' if quoted => match chars.next() {
                Some(n @ ('"' | '`' | '$' | '\\')) => lx.cur.push(n),
                Some(n) => {
                    lx.cur.push('\\');
                    lx.cur.push(n);
                }
                None => return Err(ExecError::Unterminated),
            },
            '%' if fields => match chars.next() {
                Some('%') => {
                    lx.cur.push('%');
                    lx.live = true;
                }
                Some(f @ ('f' | 'F' | 'u' | 'U' | 'i' | 'c' | 'k')) => lx.fields.push(f),
                Some(d @ ('d' | 'D' | 'n' | 'N' | 'v' | 'm')) => {
                    return Err(ExecError::Deprecated(d));
                }
                Some(o) => return Err(ExecError::Unknown(o)),
                None => return Err(ExecError::DanglingPercent),
            },
            c if c.is_ascii_whitespace() && !quoted => lx.flush(),
            c => {
                if c == '\'' && !quoted {
                    lx.lossy = true;
                }
                lx.cur.push(c);
                lx.live = true;
            }
        }
    }
    if quoted {
        return Err(ExecError::Unterminated);
    }
    lx.flush();
    if lx.tokens.is_empty() {
        return Err(ExecError::Empty);
    }
    Ok(Tokens {
        tokens: lx.tokens,
        lossy: lx.lossy,
    })
}

/// Split an `Exec` value into tokens, keeping whole-argument field codes (`%F`, `"%f"`).
pub fn exec_tokens(exec: &str) -> Result<Tokens, ExecError> {
    lex(exec, true)
}

/// Split an `Exec` value into argv. The plain field codes `%f %F %u %U` are dropped (this
/// launcher passes no files or URLs); `%i` becomes `--icon <icon>`, `%c` the name and `%k` the
/// entry's path.
pub fn exec_argv(exec: &str, ctx: &FieldCtx) -> Result<Vec<String>, ExecError> {
    let mut out = Vec::new();
    for t in exec_tokens(exec)?.tokens {
        match t {
            Token::Arg(a) => out.push(a),
            Token::Field('i') => {
                if let Some(i) = ctx.icon {
                    out.push("--icon".into());
                    out.push(i.into());
                }
            }
            Token::Field('c') => out.extend(ctx.name.map(String::from)),
            Token::Field('k') => out.extend(ctx.path.map(String::from)),
            Token::Field(_) => {}
        }
    }
    if out.is_empty() {
        return Err(ExecError::Empty);
    }
    Ok(out)
}

pub fn parse_exec(exec: &str) -> Result<Vec<String>, ExecError> {
    exec_argv(exec, &FieldCtx::default())
}

/// Split a user-typed command with the `Exec` quoting rules and `%` literal. An unquoted `'` is
/// an error: the format has no single quotes, so it would silently split the wrong way.
pub fn split_command(cmd: &str) -> Result<Vec<String>, ExecError> {
    let lexed = lex(cmd, false)?;
    if lexed.lossy {
        return Err(ExecError::SingleQuote);
    }
    Ok(lexed
        .tokens
        .into_iter()
        .filter_map(|t| match t {
            Token::Arg(a) => Some(a),
            Token::Field(_) => None,
        })
        .collect())
}

fn quote_arg(a: &str, percent: bool) -> String {
    let needs = a.is_empty()
        || a.chars().any(|c| {
            c.is_ascii_whitespace()
                || matches!(
                    c,
                    '"' | '\''
                        | '\\'
                        | '>'
                        | '<'
                        | '~'
                        | '|'
                        | '&'
                        | ';'
                        | '$'
                        | '*'
                        | '?'
                        | '#'
                        | '('
                        | ')'
                        | '`'
                )
        });
    let mut out = String::new();
    if needs {
        out.push('"');
    }
    for c in a.chars() {
        match c {
            '"' | '`' | '$' | '\\' if needs => {
                out.push('\\');
                out.push(c);
            }
            '%' if percent => out.push_str("%%"),
            c => out.push(c),
        }
    }
    if needs {
        out.push('"');
    }
    out
}

/// The `Exec` value for these tokens (a `Field` must be one of `fFuUick`), as it goes after `Exec=`: quoted, `%` doubled and the
/// key-value escapes applied so it is always one line.
pub fn write_exec(tokens: &[Token]) -> String {
    let line: Vec<String> = tokens
        .iter()
        .map(|t| match t {
            Token::Arg(a) => quote_arg(a, true),
            Token::Field(c) => {
                debug_assert!("fFuUick".contains(*c), "not a field code: %{c}");
                format!("%{c}")
            }
        })
        .collect();
    escape_value(&line.join(" "))
}

/// An argv as text for showing: quoted like `Exec`, `%` left alone; `split_command` reads it back.
pub fn quote_command(argv: &[String]) -> String {
    argv.iter()
        .map(|a| quote_arg(a, false))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Make a string safe to show on one line: control and bidi format characters become spaces,
/// then it is cut to `max` characters.
pub fn clean_display(s: &str, max: usize) -> String {
    s.chars()
        .map(|c| {
            if c.is_control()
                || matches!(c, '\u{202A}'..='\u{202E}'
                        | '\u{2066}'..='\u{2069}'
                        | '\u{2028}'
                        | '\u{2029}'
                        | '\u{200E}'
                        | '\u{200F}'
                        | '\u{061C}'
                )
            {
                ' '
            } else {
                c
            }
        })
        .take(max)
        .collect()
}

/// Apply the key-value escapes of the desktop-entry format so a value is one line.
pub fn escape_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

/// Undo the key-value escapes of the desktop-entry format (`\s \n \t \r \\`).
fn unescape_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut it = v.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// A `;`-separated list, the trailing `;` optional, `\;` a literal one; items are unescaped.
fn split_list(v: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut cur = String::new();
    let mut it = v.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\\' if it.peek() == Some(&';') => {
                it.next();
                cur.push(';');
            }
            '\\' => {
                cur.push('\\');
                if let Some(n) = it.next() {
                    cur.push(n);
                }
            }
            ';' => items.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    items.push(cur);
    items
        .into_iter()
        .map(|i| unescape_value(&i))
        .filter(|i| !i.is_empty())
        .collect()
}

/// The keys of the `[Desktop Entry]` group that uQ reads.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Entry {
    pub kind: Option<String>,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub exec: Option<String>,
    pub try_exec: Option<String>,
    pub terminal: bool,
    /// `X-TerminalArgExec`: the argument a terminal takes before the command to run.
    pub terminal_arg: Option<String>,
    pub hidden: bool,
    pub only_show_in: Option<Vec<String>>,
    pub not_show_in: Option<Vec<String>>,
    pub dbus_activatable: bool,
    pub path: Option<String>,
    pub wm_class: Option<String>,
    pub gnome_enabled: Option<bool>,
    pub gnome_delay: Option<u32>,
    pub uq_override: bool,
    pub uq_source: Option<String>,
    pub uq_autostart: bool,
}

/// Read the `[Desktop Entry]` group; the last occurrence of a key wins.
pub fn read_entry(text: &str) -> Entry {
    let mut e = Entry::default();
    let mut in_group = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_group = line == "[Desktop Entry]";
            continue;
        }
        if !in_group || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim();
        let s = || Some(unescape_value(v));
        let b = v == "true";
        match k.trim() {
            "Type" => e.kind = Some(v.to_string()),
            "Name" => e.name = s(),
            "Icon" => e.icon = s(),
            "Exec" => e.exec = s(),
            "TryExec" => e.try_exec = s(),
            "Terminal" => e.terminal = b,
            "X-TerminalArgExec" => e.terminal_arg = s(),
            "Hidden" => e.hidden = b,
            "OnlyShowIn" => e.only_show_in = Some(split_list(v)).filter(|l| !l.is_empty()),
            "NotShowIn" => e.not_show_in = Some(split_list(v)).filter(|l| !l.is_empty()),
            "DBusActivatable" => e.dbus_activatable = b,
            "Path" => e.path = s(),
            "StartupWMClass" => e.wm_class = s(),
            "X-GNOME-Autostart-enabled" => e.gnome_enabled = Some(b),
            "X-GNOME-Autostart-Delay" => e.gnome_delay = v.parse().ok(),
            "X-uQ-Override" => e.uq_override = b,
            "X-uQ-Source" => e.uq_source = s(),
            "X-uQ-Autostart" => e.uq_autostart = b,
            _ => {}
        }
    }
    e
}

/// The desktops uQ answers to in `OnlyShowIn` and `NotShowIn`.
pub const UQ_DESKTOPS: [&str; 2] = ["Hyprland", "uQ"];

/// Whether the entry is for these desktops: `OnlyShowIn` needs an overlap and decides on its
/// own; otherwise `NotShowIn` refuses one.
pub fn shows_in(e: &Entry, desktops: &[&str]) -> bool {
    let hit = |l: &Vec<String>| l.iter().any(|d| desktops.contains(&d.as_str()));
    match (&e.only_show_in, &e.not_show_in) {
        (Some(only), _) => hit(only),
        (None, Some(not)) => !hit(not),
        (None, None) => true,
    }
}

/// `id` with `.desktop` added when it is missing.
pub fn normalise_id(id: &str) -> String {
    if id.ends_with(".desktop") {
        id.to_string()
    } else {
        format!("{id}.desktop")
    }
}

/// A desktop id that is safe to join onto a directory: `valid_pinned_id` once normalised, with
/// no leading `.` and no `..`.
pub fn valid_desktop_id(id: &str) -> bool {
    let n = normalise_id(id);
    valid_pinned_id(&n) && !n.starts_with('.') && !n.contains("..")
}

/// A systemd unit-name fragment for an app id: alphanumerics, `_` and `-`, the rest `_`, at most 64.
pub fn unit_id(app_id: &str) -> String {
    app_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

/// A small non-cryptographic value: it only keeps scope names apart.
pub fn random_suffix() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let mut x =
        nanos ^ std::process::id().rotate_left(16) ^ N.fetch_add(0x9e37_79b9, Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 17;
    x ^ (x << 5)
}

fn group_of(line: &str) -> Option<&str> {
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

#[cfg(test)]
mod tests;
