//! Pattern and value checks for rules.
//!
//! Hyprland matches with RE2, which has no builder error path: a pattern it can't compile just
//! never matches, and `--verify-config` can't see it. So every pattern is checked here, with
//! Rust's `regex` (the same family: RE2 syntax, linear time) after refusing the syntax that only
//! Rust accepts. Refusals use fixed plain reasons; the regex crate's own text (several lines
//! long, repeating the pattern) never reaches a caller.

use super::hypr::{self, Field};
use crate::settings::{SettingError, invalid};
use regex_syntax::ast::{self, Ast, Visitor};

pub const MAX_PATTERN_CHARS: usize = 256;
/// RE2's `kMaxRepeat`.
const MAX_REPEAT: u32 = 1000;
const SIZE_LIMIT: usize = 1 << 18;

const UNSUPPORTED: &str = "this uses something Hyprland's patterns don't support";

/// `^[A-Za-z0-9_.-]{1,32}$`, and not one of uQ's own `uq_` tags.
pub fn check_tag(key: &str, tag: &str) -> Result<(), SettingError> {
    if !(1..=32).contains(&tag.len())
        || !tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    {
        return Err(invalid(
            key,
            "tag names use letters, digits, dots, dashes and underscores, up to 32",
        ));
    }
    if tag.starts_with("uq_") {
        return Err(invalid(
            key,
            "names starting with uq_ are kept for uQ's own use",
        ));
    }
    Ok(())
}

/// A whole number within the table's range for `f`.
pub fn in_range(key: &str, f: &Field, n: i64) -> Result<(), SettingError> {
    let (lo, hi) = (f.min.unwrap_or(i64::MIN), f.max.unwrap_or(i64::MAX));
    if n < lo || n > hi {
        return Err(invalid(key, format!("must be between {lo} and {hi}")));
    }
    Ok(())
}

/// A two-place number (the table holds hundredths) within the table's range for `f`; returns it
/// rounded to two places.
pub fn in_range_f(key: &str, f: &Field, x: f64) -> Result<f64, SettingError> {
    let hundredths = (x * 100.0).round();
    let (lo, hi) = (f.min.unwrap_or(0), f.max.unwrap_or(100));
    if !x.is_finite() || hundredths < lo as f64 || hundredths > hi as f64 {
        return Err(invalid(
            key,
            format!(
                "must be between {} and {}",
                lo as f64 / 100.0,
                hi as f64 / 100.0
            ),
        ));
    }
    Ok(hundredths / 100.0)
}

/// One of the choices of a `Kind::Choice` field.
pub fn in_choices(key: &str, choices: &[&str], v: &str) -> Result<(), SettingError> {
    if choices.contains(&v) {
        return Ok(());
    }
    Err(invalid(
        key,
        format!("expected one of {}", choices.join(", ")),
    ))
}

/// Checks a pattern as a window or panel rule uses it. `field` is the full key the refusal names.
pub fn check_pattern(field: &str, p: &str) -> Result<(), SettingError> {
    pattern_reason(p)
        .map_err(|reason| invalid(field, format!("this pattern can't be used: {reason}")))
}

fn pattern_reason(p: &str) -> Result<(), &'static str> {
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

pub(super) fn parse_reason(kind: &ast::ErrorKind) -> &'static str {
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
struct Refuse;

impl Refuse {
    fn flags(f: &ast::Flags) -> Result<(), &'static str> {
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

/// A hypr table lookup that can't miss: the field is in the table by construction.
pub fn window_field(uq: &str) -> &'static Field {
    &hypr::window_effect(uq)
        .expect("window effect is in the table")
        .field
}

/// Rejects keys no rule has, naming the object that holds them. `list` is `window`, `layer` or
/// `workspace`; `key` is the entry's key.
pub fn known_keys(list: &str, key: &str, v: &serde_json::Value) -> Result<(), SettingError> {
    use crate::settings::check_known;
    match list {
        "window" => {
            check_known(key, v, &["id", "name", "enabled", "match", "effects"])?;
            let names = |fs: &[Field]| fs.iter().map(|f| f.uq).collect::<Vec<_>>();
            if let Some(m) = v.get("match") {
                check_known(&format!("{key}.match"), m, &names(&hypr::MATCH_KEYS))?;
            }
            if let Some(e) = v.get("effects") {
                let mut known: Vec<&str> =
                    hypr::WINDOW_EFFECTS.iter().map(|e| e.field.uq).collect();
                known.push("silent");
                check_known(&format!("{key}.effects"), e, &known)?;
            }
        }
        "layer" => {
            check_known(key, v, &["id", "name", "enabled", "match", "effects"])?;
            if let Some(m) = v.get("match") {
                check_known(&format!("{key}.match"), m, &["namespace"])?;
            }
            if let Some(e) = v.get("effects") {
                let known: Vec<&str> = hypr::LAYER_EFFECTS.iter().map(|f| f.uq).collect();
                check_known(&format!("{key}.effects"), e, &known)?;
            }
        }
        _ => {
            let known: Vec<&str> = std::iter::once("workspace")
                .chain(hypr::WORKSPACE_FIELDS.iter().map(|f| f.uq))
                .collect();
            check_known(key, v, &known)?;
        }
    }
    Ok(())
}

fn name(key: &str, name: &str) -> Result<(), SettingError> {
    let n = name.chars().count();
    if !(1..=super::MAX_NAME_CHARS).contains(&n) {
        return Err(invalid(key, "a name is 1 to 100 characters"));
    }
    if name.chars().any(char::is_control) {
        return Err(invalid(key, "a name can't hold control characters"));
    }
    Ok(())
}

/// A workspace selector as the layouts use it, with the error re-keyed to the rule's field.
fn selector(key: &str, s: &str) -> Result<String, SettingError> {
    crate::settings::rule_selector(s).map_err(|e| match e {
        SettingError::Invalid { reason, .. } => invalid(key, reason),
        other => other,
    })
}

fn output(key: &str, s: &str) -> Result<(), SettingError> {
    if crate::settings::valid_output_name(s) {
        return Ok(());
    }
    Err(invalid(key, "not a screen name"))
}

/// Checks a window rule and normalises it in place (selectors, two-place numbers).
pub fn window(i: usize, r: &mut super::WindowRule) -> Result<(), SettingError> {
    let base = format!("rules.window[{i}]");
    name(&format!("{base}.name"), &r.name)?;
    let (m, e) = (&mut r.matches, &mut r.effects);
    let mk = format!("{base}.match");
    for (k, p) in [
        ("class", &m.class),
        ("title", &m.title),
        ("initial_class", &m.initial_class),
        ("initial_title", &m.initial_title),
    ] {
        if let Some(p) = p {
            check_pattern(&format!("{mk}.{k}"), p)?;
        }
    }
    if let Some(t) = &m.tag {
        check_tag(&format!("{mk}.tag"), t)?;
    }
    if let Some(w) = &m.workspace {
        m.workspace = Some(selector(&format!("{mk}.workspace"), w)?);
    }
    let has_match = m.class.is_some()
        || m.title.is_some()
        || m.initial_class.is_some()
        || m.initial_title.is_some()
        || m.tag.is_some()
        || m.workspace.is_some()
        || [m.xwayland, m.floating, m.fullscreen, m.pinned, m.modal]
            .iter()
            .any(Option::is_some);
    if !has_match {
        return Err(invalid(&mk, "a rule needs at least one thing to match"));
    }
    let ek = format!("{base}.effects");
    let has_effect = [
        e.float,
        e.tile,
        e.fullscreen,
        e.maximize,
        e.center,
        e.pin,
        e.no_initial_focus,
        e.no_blur,
        e.no_shadow,
        e.no_anim,
        e.keep_aspect_ratio,
    ]
    .iter()
    .any(Option::is_some)
        || e.workspace.is_some()
        || e.monitor.is_some()
        || e.size.is_some()
        || e.position.is_some()
        || e.opacity.is_some()
        || e.rounding.is_some()
        || e.border_size.is_some()
        || e.idle_inhibit.is_some()
        || e.tag.is_some();
    if !has_effect {
        return Err(invalid(&ek, "a rule needs at least one effect"));
    }
    if e.float == Some(true) && e.tile == Some(true) {
        return Err(invalid(&ek, "Float and Tile can't both be on"));
    }
    if e.position.is_some() && e.center == Some(true) {
        return Err(invalid(&ek, "Position and Center can't both be used"));
    }
    if e.silent == Some(false) && e.workspace.is_none() {
        e.silent = None;
    }
    if e.silent.is_some() && e.workspace.is_none() {
        return Err(invalid(&ek, "Quietly needs a workspace to open on"));
    }
    if let Some(w) = &e.workspace {
        e.workspace = Some(selector(&format!("{ek}.workspace"), w)?);
    }
    if let Some(s) = &e.monitor {
        output(&format!("{ek}.monitor"), s)?;
    }
    if let Some(p) = e.size {
        let f = window_field("size");
        for n in p {
            in_range(&format!("{ek}.size"), f, i64::from(n))?;
        }
    }
    if let Some(p) = e.position {
        let f = window_field("move");
        for n in p {
            in_range(&format!("{ek}.move"), f, i64::from(n))?;
        }
    }
    if let Some(x) = e.opacity {
        e.opacity = Some(in_range_f(
            &format!("{ek}.opacity"),
            window_field("opacity"),
            x,
        )?);
    }
    if let Some(n) = e.rounding {
        in_range(
            &format!("{ek}.rounding"),
            window_field("rounding"),
            i64::from(n),
        )?;
    }
    if let Some(n) = e.border_size {
        in_range(
            &format!("{ek}.border_size"),
            window_field("border_size"),
            i64::from(n),
        )?;
    }
    if let Some(v) = &e.idle_inhibit {
        in_choices(&format!("{ek}.idle_inhibit"), &hypr::IDLE_INHIBIT, v)?;
    }
    if let Some(t) = &e.tag {
        check_tag(&format!("{ek}.tag"), t)?;
    }
    Ok(())
}

/// Checks a panel rule and normalises it in place.
pub fn layer(i: usize, r: &mut super::LayerRule) -> Result<(), SettingError> {
    let base = format!("rules.layer[{i}]");
    name(&format!("{base}.name"), &r.name)?;
    check_pattern(&format!("{base}.match.namespace"), &r.matches.namespace)?;
    let (ek, e) = (format!("{base}.effects"), &mut r.effects);
    if e.blur.is_none() && e.no_anim.is_none() && e.ignore_alpha.is_none() && e.animation.is_none()
    {
        return Err(invalid(&ek, "a rule needs at least one effect"));
    }
    if let Some(x) = e.ignore_alpha {
        let f = hypr::LAYER_EFFECTS
            .iter()
            .find(|f| f.uq == "ignore_alpha")
            .expect("ignore_alpha is in the table");
        e.ignore_alpha = Some(in_range_f(&format!("{ek}.ignore_alpha"), f, x)?);
    }
    if let Some(v) = &e.animation {
        in_choices(&format!("{ek}.animation"), &hypr::LAYER_ANIMATIONS, v)?;
    }
    Ok(())
}

/// Checks a workspace rule and normalises its selector in place.
pub fn workspace(i: usize, r: &mut super::WorkspaceRule) -> Result<(), SettingError> {
    let base = format!("rules.workspace[{i}]");
    r.workspace = selector(&format!("{base}.workspace"), &r.workspace)?;
    if r.monitor.is_none()
        && r.default.is_none()
        && r.persistent.is_none()
        && r.gaps_in.is_none()
        && r.gaps_out.is_none()
        && r.border_size.is_none()
    {
        return Err(invalid(
            &base,
            "a rule needs at least one setting besides the workspace",
        ));
    }
    if let Some(s) = &r.monitor {
        output(&format!("{base}.monitor"), s)?;
    }
    for (k, v) in [
        ("gaps_in", r.gaps_in),
        ("gaps_out", r.gaps_out),
        ("border_size", r.border_size),
    ] {
        if let Some(n) = v {
            let f = hypr::WORKSPACE_FIELDS
                .iter()
                .find(|f| f.uq == k)
                .expect("workspace field is in the table");
            in_range(&format!("{base}.{k}"), f, i64::from(n))?;
        }
    }
    Ok(())
}
