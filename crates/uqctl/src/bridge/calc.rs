//! Calculator provider for the look=see>do_ `=` queries (uQ's calculator: there is no KCalc).
//!
//! Tokenizer plus precedence climbing with explicit stacks (no recursion), so
//! hostile input cannot overflow the stack. Nothing here executes code.
//!
//! Accepts `+ - * / ^ mod`, `×` `÷` `−` `·` and `**`, implicit multiplication (`2pi`, `3(1+1)`),
//! percentages (`18% of 2450`, `2450 + 18%`), factorials (`5!`), degrees (`sin(30°)`),
//! `1e3`, `1,234,567`, `0xff`/`0b101`/`0o17`, a trailing `in hex|bin|oct|dec`, `ans` (the last
//! result copied) and the functions in [`FUNCTIONS`].

use std::sync::Mutex;

use serde::Serialize;
mod eval;
use eval::*;

const MAX_LEN: usize = 256;
const MAX_DEPTH: usize = 32;
const FUNCTIONS: [&str; 19] = [
    "sqrt", "cbrt", "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "ln",
    "log", "log2", "exp", "abs", "round", "floor", "ceil",
];
/// Integers up to here are exact in an f64, so they print in full.
const EXACT: f64 = 9_007_199_254_740_992.0;

/// The last result copied from the calculator row: what `ans` means.
static ANS: Mutex<Option<f64>> = Mutex::new(None);

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

#[derive(Debug, Clone, Copy, PartialEq)]
enum Base {
    Dec,
    Hex,
    Bin,
    Oct,
}

/// True when the query should be offered to the calculator.
pub(crate) fn applies(query: &str) -> bool {
    if query.starts_with('=') {
        return true;
    }
    let q = query.trim().to_lowercase();
    if q.chars().skip(1).any(|c| "+-*/^%×÷−·!°".contains(c)) {
        return true;
    }
    if q.contains(" mod ") || split_base(&q).1 != Base::Dec {
        return true;
    }
    let radix = ["0x", "0b", "0o"]
        .iter()
        .any(|p| q.len() > 2 && q.starts_with(p));
    radix
        || FUNCTIONS.iter().any(|f| {
            q.match_indices(f)
                .any(|(i, _)| q[i + f.len()..].trim_start().starts_with('('))
        })
}

/// Evaluate the query (a leading `=` is stripped) and format the result.
pub(crate) fn evaluate(query: &str) -> Option<String> {
    let ans = *ANS.lock().unwrap_or_else(|e| e.into_inner());
    evaluate_with(query, ans)
}

/// Evaluate and keep the value as `ans` for the next query (the row was copied).
pub(crate) fn remember(query: &str) -> Option<String> {
    let mut ans = ANS.lock().unwrap_or_else(|e| e.into_inner());
    let (v, base) = compute(query, *ans)?;
    let shown = format_in(v, base)?;
    *ans = Some(v);
    Some(shown)
}

fn evaluate_with(query: &str, ans: Option<f64>) -> Option<String> {
    let (v, base) = compute(query, ans)?;
    format_in(v, base)
}

fn compute(query: &str, ans: Option<f64>) -> Option<(f64, Base)> {
    if query.chars().count() > MAX_LEN {
        return None;
    }
    let expr = query
        .strip_prefix('=')
        .unwrap_or(query)
        .trim()
        .to_lowercase();
    let (expr, base) = split_base(&expr);
    let tokens = tokenize(expr)?;
    Some((eval(&tokens, ans)?, base))
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

/// `255 in hex` → (`255`, Hex). Also `to`/`as`, and the long names.
fn split_base(q: &str) -> (&str, Base) {
    let q = q.trim_end();
    let Some((head, last)) = q.rsplit_once(char::is_whitespace) else {
        return (q, Base::Dec);
    };
    let base = match last {
        "hex" | "hexadecimal" => Base::Hex,
        "bin" | "binary" => Base::Bin,
        "oct" | "octal" => Base::Oct,
        "dec" | "decimal" => Base::Dec,
        _ => return (q, Base::Dec),
    };
    match head.trim_end().rsplit_once(char::is_whitespace) {
        Some((expr, "in" | "to" | "as")) if !expr.trim().is_empty() => (expr, base),
        _ => (q, Base::Dec),
    }
}

fn val(v: f64) -> Val {
    Val { v, pct: false }
}

fn format_in(v: f64, base: Base) -> Option<String> {
    if base == Base::Dec {
        return format_number(v);
    }
    if !v.is_finite() || v.fract() != 0.0 || v.abs() > EXACT {
        return None;
    }
    let (sign, n) = (if v < 0.0 { "-" } else { "" }, v.abs() as u64);
    Some(match base {
        Base::Hex => format!("{sign}0x{n:x}"),
        Base::Bin => format!("{sign}0b{n:b}"),
        _ => format!("{sign}0o{n:o}"),
    })
}

fn format_number(v: f64) -> Option<String> {
    if !v.is_finite() {
        return None;
    }
    // Whole numbers that an f64 holds exactly print in full (2^53, 20!).
    if v.fract() == 0.0 && v.abs() <= EXACT {
        return Some(format!("{}", v as i64));
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
#[path = "calc_tests.rs"]
mod tests;
