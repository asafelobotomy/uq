//! The `Exec` key: lexing, field codes, quoting and writing it back.

use super::*;

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
pub(crate) struct Lexer {
    pub(crate) tokens: Vec<Token>,
    pub(crate) cur: String,
    // An argument exists once it has literal text or was quoted (so "" is an empty argument).
    pub(crate) live: bool,
    pub(crate) fields: Vec<char>,
    pub(crate) lossy: bool,
}

impl Lexer {
    pub(crate) fn flush(&mut self) {
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
pub(crate) fn lex(exec: &str, fields: bool) -> Result<Tokens, ExecError> {
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

pub(crate) fn quote_arg(a: &str, percent: bool) -> String {
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
