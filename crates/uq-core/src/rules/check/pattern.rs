//! Patterns: what RE2 (Hyprland's regex engine) would refuse, said plainly.

use super::*;

/// Checks a pattern as a window or panel rule uses it. `field` is the full key the refusal names.
pub fn check_pattern(field: &str, p: &str) -> Result<(), SettingError> {
    pattern_reason(p)
        .map_err(|reason| invalid(field, format!("this pattern can't be used: {reason}")))
}

pub(crate) fn pattern_reason(p: &str) -> Result<(), &'static str> {
    if p.is_empty() {
        return Err("patterns can't be empty");
    }
    if p.chars().count() > MAX_PATTERN_CHARS {
        return Err("patterns are at most 256 characters");
    }
    if p.chars().any(char::is_control) {
        return Err("patterns can't hold control characters");
    }
    if p.starts_with("negative:") {
        return Err("patterns can't start with negative:");
    }
    let parsed = ast::parse::Parser::new()
        .parse(p)
        .map_err(|e| parse_reason(e.kind()))?;
    ast::visit(&parsed, Refuse)?;
    // Hyprland uses a full match, so the pattern is wrapped as the matcher wraps it.
    regex::RegexBuilder::new(&format!("^(?:{p})$"))
        .size_limit(SIZE_LIMIT)
        .dfa_size_limit(SIZE_LIMIT)
        .build()
        .map(drop)
        .map_err(|e| match e {
            regex::Error::CompiledTooBig(_) => "this pattern is too complex",
            _ => "this pattern isn't valid",
        })
}

pub(in crate::rules) fn parse_reason(kind: &ast::ErrorKind) -> &'static str {
    use ast::ErrorKind::*;
    match kind {
        GroupUnclosed | GroupUnopened | ClassUnclosed | RepetitionCountUnclosed => {
            "a bracket or group isn't closed"
        }
        RepetitionMissing => "a repeat (*, + or ?) has nothing before it",
        NestLimitExceeded(_) => "this pattern is too complex",
        UnsupportedBackreference | UnsupportedLookAround | FlagUnrecognized => UNSUPPORTED,
        _ => "this pattern isn't valid",
    }
}

/// Refuses syntax Rust accepts and RE2 doesn't.
pub(crate) struct Refuse;

impl Refuse {
    pub(crate) fn flags(f: &ast::Flags) -> Result<(), &'static str> {
        use ast::{Flag, FlagsItemKind};
        for item in &f.items {
            if let FlagsItemKind::Flag(flag) = &item.kind
                && !matches!(
                    flag,
                    Flag::CaseInsensitive
                        | Flag::MultiLine
                        | Flag::DotMatchesNewLine
                        | Flag::SwapGreed
                )
            {
                return Err(UNSUPPORTED);
            }
        }
        Ok(())
    }
}

impl Visitor for Refuse {
    type Output = ();
    type Err = &'static str;

    fn finish(self) -> Result<(), &'static str> {
        Ok(())
    }

    fn visit_pre(&mut self, a: &Ast) -> Result<(), &'static str> {
        use ast::{AssertionKind as K, GroupKind, RepetitionKind, RepetitionRange as R};
        match a {
            Ast::Flags(f) => Self::flags(&f.flags),
            Ast::Group(g) => match &g.kind {
                GroupKind::NonCapturing(f) => Self::flags(f),
                _ => Ok(()),
            },
            Ast::Assertion(x) => match x.kind {
                K::StartLine
                | K::EndLine
                | K::StartText
                | K::EndText
                | K::WordBoundary
                | K::NotWordBoundary => Ok(()),
                _ => Err(UNSUPPORTED),
            },
            Ast::Repetition(r) => match &r.op.kind {
                RepetitionKind::Range(R::Exactly(n) | R::AtLeast(n)) if *n > MAX_REPEAT => {
                    Err(UNSUPPORTED)
                }
                RepetitionKind::Range(R::Bounded(m, n)) if *m > MAX_REPEAT || *n > MAX_REPEAT => {
                    Err(UNSUPPORTED)
                }
                _ => Ok(()),
            },
            Ast::ClassUnicode(_) => Err(UNSUPPORTED),
            _ => Ok(()),
        }
    }

    fn visit_class_set_item_pre(&mut self, i: &ast::ClassSetItem) -> Result<(), &'static str> {
        match i {
            ast::ClassSetItem::Unicode(_) => Err(UNSUPPORTED),
            _ => Ok(()),
        }
    }

    fn visit_class_set_binary_op_pre(
        &mut self,
        _: &ast::ClassSetBinaryOp,
    ) -> Result<(), &'static str> {
        Err(UNSUPPORTED)
    }
}
