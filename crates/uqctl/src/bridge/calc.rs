//! Calculator provider for the Spotlight `=` queries.
//!
//! Tokenizer plus precedence climbing with explicit stacks (no recursion), so
//! hostile input cannot overflow the stack. Nothing here executes code.

use serde::Serialize;

const MAX_LEN: usize = 256;
const MAX_DEPTH: usize = 32;
const FUNCTIONS: [&str; 8] = ["sqrt", "sin", "cos", "tan", "ln", "log", "abs", "round"];

/// One search result row, shared by all built-in providers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct ProviderItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: String,
    pub score: f64,
    pub actions: Vec<String>,
}

/// True when the query should be offered to the calculator.
pub(crate) fn applies(query: &str) -> bool {
    if query.starts_with('=') {
        return true;
    }
    let q = query.trim();
    if q.chars().skip(1).any(|c| "+-*/^%".contains(c)) {
        return true;
    }
    FUNCTIONS.iter().any(|f| {
        q.match_indices(f)
            .any(|(i, _)| q[i + f.len()..].starts_with('('))
    })
}

/// Evaluate the query (a leading `=` is stripped) and format the result.
pub(crate) fn evaluate(query: &str) -> Option<String> {
    if query.chars().count() > MAX_LEN {
        return None;
    }
    let expr = query.strip_prefix('=').unwrap_or(query);
    let tokens = tokenize(expr)?;
    let value = eval(&tokens)?;
    format_number(value)
}

pub(crate) fn item(query: &str) -> Option<ProviderItem> {
    let title = evaluate(query)?;
    Some(ProviderItem {
        id: "calc".into(),
        title,
        subtitle: query.strip_prefix('=').unwrap_or(query).trim().to_string(),
        icon: "accessories-calculator".into(),
        score: 1.0,
        actions: vec!["copy".into()],
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tok {
    Num(f64),
    Func(usize),
    Bin(char),
    Percent,
    Of,
    LParen,
    RParen,
}

fn tokenize(s: &str) -> Option<Vec<Tok>> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '0'..='9' | '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                out.push(Tok::Num(text.parse().ok()?));
            }
            '+' | '-' | '*' | '/' | '^' => {
                out.push(Tok::Bin(c));
                i += 1;
            }
            '%' => {
                out.push(Tok::Percent);
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            c if c.is_ascii_alphabetic() => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect::<String>().to_lowercase();
                match word.as_str() {
                    "pi" => out.push(Tok::Num(std::f64::consts::PI)),
                    "e" => out.push(Tok::Num(std::f64::consts::E)),
                    "of" => out.push(Tok::Of),
                    w => {
                        let idx = FUNCTIONS.iter().position(|f| *f == w)?;
                        // Function calls need parentheses.
                        let mut j = i;
                        while j < chars.len() && chars[j].is_whitespace() {
                            j += 1;
                        }
                        if chars.get(j) != Some(&'(') {
                            return None;
                        }
                        out.push(Tok::Func(idx));
                    }
                }
            }
            _ => return None,
        }
    }
    Some(out)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Bin(char),
    Neg,
    Func(usize),
    LParen,
}

fn prec(op: Op) -> u8 {
    match op {
        Op::Bin('+' | '-') => 1,
        Op::Bin('*' | '/') => 2,
        Op::Neg => 3,
        Op::Bin(_) => 4,
        Op::Func(_) | Op::LParen => 0,
    }
}

fn apply_bin(c: char, a: f64, b: f64) -> f64 {
    match c {
        '+' => a + b,
        '-' => a - b,
        '*' => a * b,
        '/' => a / b,
        _ => a.powf(b),
    }
}

fn apply_func(idx: usize, x: f64) -> f64 {
    match FUNCTIONS[idx] {
        "sqrt" => x.sqrt(),
        "sin" => x.sin(),
        "cos" => x.cos(),
        "tan" => x.tan(),
        "ln" => x.ln(),
        "log" => x.log10(),
        "abs" => x.abs(),
        _ => x.round(),
    }
}

/// Pop one operator off the stack and apply it to the value stack.
fn reduce(ops: &mut Vec<Op>, vals: &mut Vec<f64>) -> Option<()> {
    match ops.pop()? {
        Op::Bin(c) => {
            let b = vals.pop()?;
            let a = vals.pop()?;
            vals.push(apply_bin(c, a, b));
        }
        Op::Neg => {
            let a = vals.pop()?;
            vals.push(-a);
        }
        Op::Func(_) | Op::LParen => return None,
    }
    Some(())
}

fn eval(tokens: &[Tok]) -> Option<f64> {
    let mut vals: Vec<f64> = Vec::new();
    let mut ops: Vec<Op> = Vec::new();
    let mut depth = 0usize;
    // True when the next token must start an operand.
    let mut want_operand = true;
    // True right after a postfix `%`, the only place `of` is allowed.
    let mut after_percent = false;
    for &t in tokens {
        let was_percent = std::mem::take(&mut after_percent);
        match t {
            Tok::Num(n) if want_operand => {
                vals.push(n);
                want_operand = false;
            }
            Tok::Func(idx) if want_operand => ops.push(Op::Func(idx)),
            Tok::LParen if want_operand => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return None;
                }
                ops.push(Op::LParen);
            }
            Tok::Bin('-') if want_operand => ops.push(Op::Neg),
            Tok::Bin(c) if !want_operand => {
                push_binary(Op::Bin(c), &mut ops, &mut vals)?;
                want_operand = true;
            }
            Tok::Of if !want_operand && was_percent => {
                push_binary(Op::Bin('*'), &mut ops, &mut vals)?;
                want_operand = true;
            }
            Tok::Percent if !want_operand => {
                let x = vals.pop()?;
                vals.push(x / 100.0);
                after_percent = true;
            }
            Tok::RParen if !want_operand => {
                while !matches!(ops.last()?, Op::LParen) {
                    reduce(&mut ops, &mut vals)?;
                }
                ops.pop();
                depth -= 1;
                if let Some(&Op::Func(idx)) = ops.last() {
                    ops.pop();
                    let x = vals.pop()?;
                    vals.push(apply_func(idx, x));
                }
            }
            _ => return None,
        }
    }
    if want_operand {
        return None;
    }
    while !ops.is_empty() {
        reduce(&mut ops, &mut vals)?;
    }
    if vals.len() == 1 { vals.pop() } else { None }
}

fn push_binary(op: Op, ops: &mut Vec<Op>, vals: &mut Vec<f64>) -> Option<()> {
    let right_assoc = op == Op::Bin('^');
    while let Some(&top) = ops.last() {
        let (p, q) = (prec(top), prec(op));
        if top == Op::LParen || matches!(top, Op::Func(_)) || p < q || (p == q && right_assoc) {
            break;
        }
        reduce(ops, vals)?;
    }
    ops.push(op);
    Some(())
}

fn format_number(v: f64) -> Option<String> {
    if !v.is_finite() {
        return None;
    }
    // Round to 12 significant digits via scientific formatting.
    let rounded: f64 = format!("{v:.11e}").parse().ok()?;
    if rounded == 0.0 {
        return Some("0".into());
    }
    let mag = rounded.abs();
    Some(if !(1e-6..1e15).contains(&mag) {
        format!("{rounded:e}")
    } else {
        format!("{rounded}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence() {
        assert_eq!(evaluate("= 2+3*4").as_deref(), Some("14"));
        assert_eq!(evaluate("2^3^2").as_deref(), Some("512"));
        assert_eq!(evaluate("-2^2").as_deref(), Some("-4"));
    }

    #[test]
    fn percent() {
        assert_eq!(evaluate("18% of 2450").as_deref(), Some("441"));
        assert_eq!(evaluate("50%").as_deref(), Some("0.5"));
    }

    #[test]
    fn functions() {
        assert_eq!(evaluate("sqrt(16)").as_deref(), Some("4"));
        assert_eq!(evaluate("round(2.5)").as_deref(), Some("3"));
        assert_eq!(evaluate("ln(e)").as_deref(), Some("1"));
    }

    #[test]
    fn bare_number_not_calc() {
        assert!(!applies("42"));
        assert!(applies("=42"));
        assert!(!applies("-5"));
        assert!(!applies("e"));
        assert!(applies("2+2"));
        assert!(applies("sqrt(4)"));
    }

    #[test]
    fn limits() {
        let long = format!("{}1", "1+".repeat(128));
        assert_eq!(long.chars().count(), 257);
        assert_eq!(evaluate(&long), None);
        let nest = |n: usize| format!("{}1{}", "(".repeat(n), ")".repeat(n));
        assert_eq!(evaluate(&nest(33)), None);
        assert_eq!(evaluate(&nest(32)).as_deref(), Some("1"));
    }

    #[test]
    fn errors() {
        assert_eq!(evaluate("1/0"), None);
        assert_eq!(evaluate("2+"), None);
        assert_eq!(evaluate("foo(1)"), None);
        assert_eq!(evaluate("()"), None);
        assert_eq!(evaluate("sqrt(-1)"), None);
    }

    #[test]
    fn no_code_execution() {
        assert_eq!(evaluate("= system(\"x\")"), None);
    }

    #[test]
    fn rounding() {
        assert_eq!(evaluate("0.1+0.2").as_deref(), Some("0.3"));
        assert_eq!(evaluate("1/3").as_deref(), Some("0.333333333333"));
    }

    #[test]
    fn item_shape() {
        let it = item("= 1+1").unwrap();
        assert_eq!((it.id.as_str(), it.title.as_str()), ("calc", "2"));
        assert_eq!(it.subtitle, "1+1");
        assert_eq!(it.actions, vec!["copy"]);
    }
}
