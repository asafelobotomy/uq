//! Seven base colours make a whole theme (R11): the rest of the palette is blended from them, and
//! everything else comes from the theme being edited. Also the plain wording of contrast
//! problems, shared by the editor and by the checks every user theme passes.

use super::{Color, Colors, Theme, Variant};
use crate::contrast::{self, AA, ContrastFailure};
use serde::{Deserialize, Serialize};

/// The seven colours a person edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseColors {
    pub bg: Color,
    pub surface: Color,
    pub text: Color,
    pub accent: Color,
    pub good: Color,
    pub warn: Color,
    pub bad: Color,
}

/// Each field of [`BaseColors`] and its plain label.
pub const KEYS: [(&str, &str); 7] = [
    ("bg", "Background"),
    ("surface", "Panels"),
    ("text", "Text"),
    ("accent", "Accent"),
    ("good", "Good"),
    ("warn", "Warning"),
    ("bad", "Bad"),
];

/// The base colours a theme already has.
pub fn base_of(t: &Theme) -> BaseColors {
    let c = &t.colors;
    BaseColors {
        bg: c.bg,
        surface: c.surface,
        text: c.text,
        accent: c.accent,
        good: c.success,
        warn: c.warning,
        bad: c.error,
    }
}

/// A linear blend in sRGB from `a` (at 0.0) to `b` (at 1.0), rounded.
pub fn mix(a: Color, b: Color, t: f64) -> Color {
    let ch = |i: usize| {
        let (x, y) = (f64::from(a.0[i]), f64::from(b.0[i]));
        (x + (y - x) * t).round().clamp(0.0, 255.0) as u8
    };
    Color([ch(0), ch(1), ch(2)])
}

fn passes(fg: Color, bg: Color) -> bool {
    contrast::contrast_ratio(fg, bg) >= AA
}

/// The muted text colour: 35% of the way from the text to the background, stepped back toward
/// the text by 5% until it reads on the background and on panels, or is the text itself.
fn muted(text: Color, bg: Color, surface: Color) -> Color {
    (0..=7)
        .rev()
        .map(|k| mix(text, bg, f64::from(k) * 0.05))
        .find(|c| passes(*c, bg) && passes(*c, surface))
        .unwrap_or(text)
}

/// Text on the accent: the base theme's if it still reads, else black or white, whichever reads
/// better.
fn accent_text(kept: Color, accent: Color) -> Color {
    if passes(kept, accent) {
        return kept;
    }
    let (black, white) = (Color([0, 0, 0]), Color([255, 255, 255]));
    if contrast::contrast_ratio(black, accent) >= contrast::contrast_ratio(white, accent) {
        black
    } else {
        white
    }
}

/// `base` with its palette rebuilt from `c`, named `name`. Fonts, icons, sizes and the terminal
/// colours are copied. The id is left for the caller.
pub fn derive(base: &Theme, c: &BaseColors, name: &str) -> Theme {
    let colors = Colors {
        bg: c.bg,
        surface: c.surface,
        surface_alt: mix(c.surface, c.text, 0.08),
        overlay: mix(c.surface, c.text, 0.16),
        text: c.text,
        text_muted: muted(c.text, c.bg, c.surface),
        accent: c.accent,
        accent_text: accent_text(base.colors.accent_text, c.accent),
        border: mix(c.surface, c.text, 0.28),
        success: c.good,
        warning: c.warn,
        error: c.bad,
    };
    Theme {
        name: name.to_string(),
        colors,
        ..base.clone()
    }
}

/// Dark needs a background darker than its text, light a lighter one.
pub fn brightness_ok(t: &Theme) -> bool {
    let (bg, text) = (
        contrast::luminance(t.colors.bg),
        contrast::luminance(t.colors.text),
    );
    match t.variant {
        Variant::Dark => bg < text,
        Variant::Light => bg > text,
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// One contrast failure as a plain sentence, for example "Muted text on panels is hard to read
/// in the dark version (3.1 to 1; it needs 4.5 to 1)."
pub fn problem_text(variant: Variant, f: &ContrastFailure) -> String {
    let pair = if f.fg == "accent_text" {
        contrast::plain_name(f.fg).to_string()
    } else {
        format!(
            "{} on {}",
            contrast::plain_name(f.fg),
            contrast::plain_name(f.bg)
        )
    };
    // Cut, not rounded, so "4.5 to 1" is never shown as a failure of a 4.5 requirement.
    let ratio = (f.ratio * 10.0).floor() / 10.0;
    format!(
        "{} is hard to read in the {variant} version ({ratio:.1} to 1; it needs {AA} to 1).",
        capital(&pair)
    )
}

/// Every problem a variant has, as plain lines: each contrast failure, then the brightness check.
pub fn problems(t: &Theme) -> Vec<String> {
    let mut out: Vec<String> = contrast::check_theme(t)
        .iter()
        .map(|f| problem_text(t.variant, f))
        .collect();
    if !brightness_ok(t) {
        out.push(match t.variant {
            Variant::Dark => "The dark version has a light background.".into(),
            Variant::Light => "The light version has a dark background.".into(),
        });
    }
    out
}

#[cfg(test)]
mod tests;
