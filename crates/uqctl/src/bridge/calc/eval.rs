//! The calculator's tokenizer and evaluator (precedence climbing over f64).

use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Tok {
    /// A number as typed.
    Num(f64),
    /// `pi`, `e`, `tau`, `phi`, `ans`: may follow a value (`2pi`).
    Const(f64),
    Func(usize),
    Bin(char),
    Percent,
    Fact,
    Deg,
    Of,
    LParen,
    RParen,
}

pub(crate) fn tokenize(s: &str) -> Option<Vec<Tok>> {
    let s = s.replace("**", "^");
    let chars: Vec<char> = s
        .chars()
        .map(|c| match c {
            '×' | '·' | '∙' => '*',
            '÷' => '/',
            '−' | '–' => '-',
            c => c,
        })
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '0'..='9' | '.' => out.push(Tok::Num(number(&chars, &mut i)?)),
            '+' | '-' | '*' | '/' | '^' => {
                out.push(Tok::Bin(c));
                i += 1;
            }
            // `7 % 3` is the remainder; `18%`, `18% of x` and `x + 18%` are percentages.
            '%' => {
                i += 1;
                let mut j = i;
                while chars.get(j).is_some_and(|c| c.is_whitespace()) {
                    j += 1;
                }
                let operand = match chars.get(j) {
                    Some(c) if c.is_ascii_digit() || *c == '.' || *c == '(' => true,
                    Some(c) if c.is_alphabetic() => {
                        let w: String = chars[j..]
                            .iter()
                            .take_while(|c| c.is_alphabetic())
                            .collect();
                        w != "of"
                    }
                    _ => false,
                };
                out.push(if operand { Tok::Bin('%') } else { Tok::Percent });
            }
            '!' => {
                out.push(Tok::Fact);
                i += 1;
            }
            '°' => {
                out.push(Tok::Deg);
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
            'π' => {
                out.push(Tok::Const(std::f64::consts::PI));
                i += 1;
            }
            'τ' => {
                out.push(Tok::Const(std::f64::consts::TAU));
                i += 1;
            }
            c if c.is_ascii_alphabetic() => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let mut word: String = chars[start..i].iter().collect();
                if word == "log" && chars.get(i) == Some(&'2') {
                    word.push('2');
                    i += 1;
                }
                out.push(match word.as_str() {
                    "pi" => Tok::Const(std::f64::consts::PI),
                    "e" => Tok::Const(std::f64::consts::E),
                    "tau" => Tok::Const(std::f64::consts::TAU),
                    "phi" => Tok::Const((1.0 + 5f64.sqrt()) / 2.0),
                    "ans" => Tok::Const(f64::NAN), // filled in by `eval`
                    "of" => Tok::Of,
                    "mod" => Tok::Bin('%'),
                    w => {
                        let idx = FUNCTIONS.iter().position(|f| *f == w)?;
                        // Function calls need parentheses.
                        let mut j = i;
                        while chars.get(j).is_some_and(|c| c.is_whitespace()) {
                            j += 1;
                        }
                        if chars.get(j) != Some(&'(') {
                            return None;
                        }
                        Tok::Func(idx)
                    }
                });
            }
            _ => return None,
        }
    }
    Some(out)
}

/// One number at `chars[*i]`: `12.5`, `1,234,567`, `1_000`, `1e-3`, `0xff`, `0b101`, `0o17`.
pub(crate) fn number(chars: &[char], i: &mut usize) -> Option<f64> {
    let radix = match (chars[*i], chars.get(*i + 1)) {
        ('0', Some('x')) => 16,
        ('0', Some('b')) => 2,
        ('0', Some('o')) => 8,
        _ => 10,
    };
    if radix != 10 {
        *i += 2;
        let start = *i;
        while chars
            .get(*i)
            .is_some_and(|c| c.is_digit(radix) || *c == '_')
        {
            *i += 1;
        }
        let digits: String = chars[start..*i].iter().filter(|c| **c != '_').collect();
        // Above 2^53 an integer can't be held exactly: refuse rather than round.
        return u64::from_str_radix(&digits, radix)
            .ok()
            .filter(|n| *n <= 1 << 53)
            .map(|n| n as f64);
    }
    let mut text = String::new();
    let mut seen_dot = false;
    while let Some(&c) = chars.get(*i) {
        let group = |at: usize| {
            (at..at + 3).all(|k| chars.get(k).is_some_and(char::is_ascii_digit))
                && !chars.get(at + 3).is_some_and(char::is_ascii_digit)
        };
        match c {
            '0'..='9' => text.push(c),
            '.' if !seen_dot => {
                seen_dot = true;
                text.push(c);
            }
            // A thousands comma only before exactly three digits, and only before the point.
            ',' if !seen_dot && !text.is_empty() && group(*i + 1) => {}
            '_' if !text.is_empty() && chars.get(*i + 1).is_some_and(char::is_ascii_digit) => {}
            _ => break,
        }
        *i += 1;
    }
    // An exponent only when digits follow: `2e` is 2 × e.
    if chars.get(*i) == Some(&'e') {
        let sign = matches!(chars.get(*i + 1), Some('+' | '-')) as usize;
        if chars.get(*i + 1 + sign).is_some_and(char::is_ascii_digit) {
            text.push('e');
            *i += 1;
            while let Some(&c) = chars.get(*i) {
                if !(c.is_ascii_digit() || ((c == '+' || c == '-') && text.ends_with('e'))) {
                    break;
                }
                text.push(c);
                *i += 1;
            }
        }
    }
    text.parse().ok()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Op {
    Bin(char),
    Neg,
    Func(usize),
    LParen,
}

pub(crate) fn prec(op: Op) -> u8 {
    match op {
        Op::Bin('+' | '-') => 1,
        Op::Bin('*' | '/' | '%') => 2,
        Op::Neg => 3,
        Op::Bin(_) => 4,
        Op::Func(_) | Op::LParen => 0,
    }
}

/// A value, and whether it is a bare percentage (so `x + 18%` adds 18% of x).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Val {
    pub(crate) v: f64,
    pub(crate) pct: bool,
}

pub(crate) fn apply_bin(c: char, a: f64, b: Val) -> f64 {
    match c {
        '+' if b.pct => a + a * b.v,
        '-' if b.pct => a - a * b.v,
        '+' => a + b.v,
        '-' => a - b.v,
        '*' => a * b.v,
        '/' => a / b.v,
        '%' => a % b.v,
        _ => a.powf(b.v),
    }
}

pub(crate) fn apply_func(idx: usize, x: f64) -> f64 {
    match FUNCTIONS[idx] {
        "sqrt" => x.sqrt(),
        "cbrt" => x.cbrt(),
        "sin" => x.sin(),
        "cos" => x.cos(),
        "tan" => x.tan(),
        "asin" => x.asin(),
        "acos" => x.acos(),
        "atan" => x.atan(),
        "sinh" => x.sinh(),
        "cosh" => x.cosh(),
        "tanh" => x.tanh(),
        "ln" => x.ln(),
        "log" => x.log10(),
        "log2" => x.log2(),
        "exp" => x.exp(),
        "abs" => x.abs(),
        "floor" => x.floor(),
        "ceil" => x.ceil(),
        _ => x.round(),
    }
}

pub(crate) fn factorial(x: f64) -> f64 {
    if !(0.0..=170.0).contains(&x) || x.fract() != 0.0 {
        return f64::NAN;
    }
    (1..=x as u32).fold(1.0, |acc, n| acc * f64::from(n))
}

/// Pop one operator off the stack and apply it to the value stack.
pub(crate) fn reduce(ops: &mut Vec<Op>, vals: &mut Vec<Val>) -> Option<()> {
    match ops.pop()? {
        Op::Bin(c) => {
            let b = vals.pop()?;
            let a = vals.pop()?;
            vals.push(val(apply_bin(c, a.v, b)));
        }
        Op::Neg => {
            let a = vals.pop()?;
            vals.push(Val { v: -a.v, ..a });
        }
        Op::Func(_) | Op::LParen => return None,
    }
    Some(())
}

pub(crate) fn eval(tokens: &[Tok], ans: Option<f64>) -> Option<f64> {
    let mut vals: Vec<Val> = Vec::new();
    let mut ops: Vec<Op> = Vec::new();
    let mut depth = 0usize;
    // True when the next token must start an operand.
    let mut want_operand = true;
    // True right after a postfix `%`, the only place `of` is allowed.
    let mut after_percent = false;
    for &t in tokens {
        let was_percent = std::mem::take(&mut after_percent);
        // `2pi`, `3(1+1)`, `2 sqrt(9)`, `(1+1)(2+2)`: a value followed by something that starts
        // one multiplies. Two plain numbers (`2 3`) stay an error.
        if !want_operand && matches!(t, Tok::Const(_) | Tok::Func(_) | Tok::LParen) {
            push_binary(Op::Bin('*'), &mut ops, &mut vals)?;
            want_operand = true;
        }
        match t {
            Tok::Num(n) if want_operand => {
                vals.push(val(n));
                want_operand = false;
            }
            Tok::Const(c) if want_operand => {
                vals.push(val(if c.is_nan() { ans? } else { c }));
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
            Tok::Bin('+') if want_operand => {}
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
                vals.push(Val {
                    v: x.v / 100.0,
                    pct: true,
                });
                after_percent = true;
            }
            Tok::Fact if !want_operand => {
                let x = vals.pop()?;
                vals.push(val(factorial(x.v)));
            }
            Tok::Deg if !want_operand => {
                let x = vals.pop()?;
                vals.push(val(x.v.to_radians()));
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
                    vals.push(val(apply_func(idx, x.v)));
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
    if vals.len() == 1 {
        vals.pop().map(|x| x.v)
    } else {
        None
    }
}

pub(crate) fn push_binary(op: Op, ops: &mut Vec<Op>, vals: &mut Vec<Val>) -> Option<()> {
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
