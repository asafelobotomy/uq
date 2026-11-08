//! WCAG 2.x contrast checks for theme tokens.

use crate::theme::{Color, Theme};

/// Minimum ratio for normal text (WCAG AA).
pub const AA: f64 = 4.5;

fn channel(c: u8) -> f64 {
    let s = f64::from(c) / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance, 0.0 (black) to 1.0 (white).
pub fn luminance(c: Color) -> f64 {
    0.2126 * channel(c.0[0]) + 0.7152 * channel(c.0[1]) + 0.0722 * channel(c.0[2])
}

pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContrastFailure {
    pub fg: &'static str,
    pub bg: &'static str,
    pub ratio: f64,
}

/// Every text/surface pair that falls below WCAG AA.
pub fn check_theme(t: &Theme) -> Vec<ContrastFailure> {
    let c = &t.colors;
    let pairs = [
        ("text", c.text, "bg", c.bg),
        ("text", c.text, "surface", c.surface),
        ("text", c.text, "surface_alt", c.surface_alt),
        ("text_muted", c.text_muted, "bg", c.bg),
        ("text_muted", c.text_muted, "surface", c.surface),
        ("accent_text", c.accent_text, "accent", c.accent),
    ];
    pairs
        .into_iter()
        .filter_map(|(fg, fc, bg, bc)| {
            let ratio = contrast_ratio(fc, bc);
            (ratio < AA).then_some(ContrastFailure { fg, bg, ratio })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Color, Theme, Variant, tests::FIXTURE};

    #[test]
    fn black_on_white_is_21() {
        let r = contrast_ratio(Color([0, 0, 0]), Color([255, 255, 255]));
        assert!((r - 21.0).abs() < 0.01, "{r}");
    }

    #[test]
    fn same_color_is_1() {
        let c = Color([0x80, 0x40, 0x20]);
        assert!((contrast_ratio(c, c) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn ratio_is_symmetric() {
        let (a, b) = (Color([0x28, 0x28, 0x28]), Color([0xeb, 0xdb, 0xb2]));
        assert!((contrast_ratio(a, b) - contrast_ratio(b, a)).abs() < 1e-9);
    }

    #[test]
    fn fixture_passes() {
        let t = Theme::from_toml(FIXTURE, "f", Variant::Dark).unwrap();
        assert!(check_theme(&t).is_empty(), "{:?}", check_theme(&t));
    }

    #[test]
    fn check_theme_flags_low_muted_text() {
        let mut t = Theme::from_toml(FIXTURE, "f", Variant::Dark).unwrap();
        t.colors.text_muted = t.colors.surface;
        let fails = check_theme(&t);
        assert!(
            fails
                .iter()
                .any(|f| (f.fg, f.bg) == ("text_muted", "surface")),
            "{fails:?}"
        );
    }
}
