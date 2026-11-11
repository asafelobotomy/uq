use super::*;
use crate::theme::tests::FIXTURE;
use crate::theme::{ThemeRegistry, uqtheme};
use std::path::PathBuf;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn registry() -> ThemeRegistry {
    ThemeRegistry::load_dir(&repo().join("themes")).unwrap()
}

fn fixture() -> Theme {
    Theme::from_toml(FIXTURE, "user-x", Variant::Dark).unwrap()
}

fn c(hex: &str) -> Color {
    hex.parse().unwrap()
}

#[test]
fn base_of_then_derive_keeps_system_themes_passing() {
    let reg = registry();
    let mut n = 0;
    for t in reg.iter() {
        let d = derive(t, &base_of(t), &t.name);
        assert!(
            contrast::check_theme(&d).is_empty(),
            "{} {}: {:?}",
            t.id,
            t.variant,
            contrast::check_theme(&d)
        );
        assert!(brightness_ok(&d), "{} {}", t.id, t.variant);
        assert!(problems(&d).is_empty());
        n += 1;
    }
    assert_eq!(n, 14);
}

#[test]
fn derive_is_deterministic() {
    let t = fixture();
    let b = base_of(&t);
    assert_eq!(derive(&t, &b, "A"), derive(&t, &b, "A"));
    let again = derive(&derive(&t, &b, "A"), &b, "A");
    assert_eq!(again, derive(&t, &b, "A"));
}

#[test]
fn mix_blends_in_srgb_and_rounds() {
    let (k, w) = (c("#000000"), c("#ffffff"));
    assert_eq!(mix(k, w, 0.0), k);
    assert_eq!(mix(k, w, 1.0), w);
    assert_eq!(mix(k, w, 0.5), c("#808080"));
    assert_eq!(mix(c("#102030"), c("#304050"), 0.5), c("#203040"));
    assert_eq!(mix(w, k, 0.08), c("#ebebeb"));
}

#[test]
fn palette_is_blended_from_the_seven() {
    let t = fixture();
    let mut b = base_of(&t);
    b.bg = c("#101820");
    b.surface = c("#182430");
    b.good = c("#00ff00");
    b.warn = c("#ffff00");
    b.bad = c("#ff0000");
    let d = derive(&t, &b, "Ocean");
    assert_eq!(d.name, "Ocean");
    assert_eq!(d.colors.surface_alt, mix(b.surface, b.text, 0.08));
    assert_eq!(d.colors.overlay, mix(b.surface, b.text, 0.16));
    assert_eq!(d.colors.border, mix(b.surface, b.text, 0.28));
    assert_eq!(
        (d.colors.success, d.colors.warning, d.colors.error),
        (b.good, b.warn, b.bad)
    );
    assert_eq!(
        (d.colors.bg, d.colors.text, d.colors.accent),
        (b.bg, b.text, b.accent)
    );
    assert_eq!(base_of(&d), b);
}

#[test]
fn muted_text_steps_until_it_passes() {
    let mut t = fixture();
    // The usual blend reads fine: it is used as is.
    let b = base_of(&t);
    let d = derive(&t, &b, "A");
    assert_eq!(d.colors.text_muted, mix(b.text, b.bg, 0.35));
    // Text that barely reads on the background: the blend fails, so it steps back.
    t.colors.text = c("#8a8a8a");
    let mut b = base_of(&t);
    b.bg = c("#000000");
    b.surface = c("#0a0a0a");
    assert!(!passes(mix(b.text, b.bg, 0.35), b.bg));
    let d = derive(&t, &b, "A");
    let m = d.colors.text_muted;
    assert!(
        m == b.text || (passes(m, b.bg) && passes(m, b.surface)),
        "{m:?}"
    );
    assert_ne!(m, mix(b.text, b.bg, 0.35));
    // Text that doesn't read at all: muted is the text itself.
    b.text = c("#050505");
    let d = derive(&t, &b, "A");
    assert_eq!(d.colors.text_muted, b.text);
}

#[test]
fn accent_text_picks_the_readable_one() {
    let t = fixture();
    let mut b = base_of(&t);
    b.accent = c("#ffee00");
    let mut base = t.clone();
    base.colors.accent_text = c("#ffffff"); // doesn't read on yellow
    assert_eq!(derive(&base, &b, "A").colors.accent_text, c("#000000"));
    b.accent = c("#001f5c");
    base.colors.accent_text = c("#282828"); // doesn't read on navy
    assert_eq!(derive(&base, &b, "A").colors.accent_text, c("#ffffff"));
    // The base theme's choice stays when it still reads.
    let same = base_of(&t);
    assert_eq!(
        derive(&t, &same, "A").colors.accent_text,
        t.colors.accent_text
    );
    // Whatever the accent, the result reads.
    for hex in [
        "#808080", "#767676", "#777777", "#ff0000", "#00ff00", "#0000ff",
    ] {
        let mut b = base_of(&t);
        b.accent = c(hex);
        let d = derive(&base, &b, "A");
        assert!(passes(d.colors.accent_text, b.accent), "{hex}");
    }
}

#[test]
fn copies_ui_fonts_icons_terminal() {
    let t = fixture();
    let mut b = base_of(&t);
    b.accent = c("#33aaff");
    let d = derive(&t, &b, "Other");
    assert_eq!(d.ui, t.ui);
    assert_eq!(d.fonts, t.fonts);
    assert_eq!(d.icons, t.icons);
    assert_eq!(d.terminal, t.terminal);
    assert_eq!(d.variant, t.variant);
    assert_eq!(d.id, t.id);
}

#[test]
fn derived_themes_import_cleanly() {
    let reg = registry();
    let (d0, l0) = (
        reg.get("gruvbox", Variant::Dark).unwrap(),
        reg.get("gruvbox", Variant::Light).unwrap(),
    );
    let mut bd = base_of(d0);
    bd.accent = c("#4aa3df");
    let mut bl = base_of(l0);
    bl.accent = c("#0b5c8e");
    let mut dark = derive(d0, &bd, "Ocean");
    let mut light = derive(l0, &bl, "Ocean");
    assert!(problems(&dark).is_empty() && problems(&light).is_empty());
    dark.id = "user-ocean".into();
    light.id = "user-ocean".into();
    let t = uqtheme::from_pair(&dark, &light, Some("Sam"), None);
    assert_eq!(t.slug, "ocean");
    let text = uqtheme::to_text(&t);
    assert_eq!(uqtheme::parse(&text), Ok(t));
}

#[test]
fn problems_are_plain_and_name_the_pair() {
    let mut t = fixture();
    assert!(problems(&t).is_empty());
    t.colors.text_muted = t.colors.surface;
    let p = problems(&t);
    assert!(
        p.iter()
            .any(|l| l.starts_with("Muted text on panels is hard to read")
                && l.contains("1.0 to 1")
                && l.contains("it needs 4.5 to 1")
                && l.contains("dark")),
        "{p:?}"
    );
    t.colors.accent_text = t.colors.accent;
    let p = problems(&t);
    assert!(
        p.iter()
            .any(|l| l.starts_with("Text on the accent is hard to read")),
        "{p:?}"
    );
    for l in &p {
        for bad in ["text_muted", "surface", "accent_text", "WCAG", "{", "["] {
            assert!(!l.contains(bad), "{l}");
        }
        assert!(l.ends_with('.'), "{l}");
    }
    let wrong = Theme::from_toml(
        &std::fs::read_to_string(repo().join("themes/gruvbox/light.toml")).unwrap(),
        "x",
        Variant::Dark,
    )
    .unwrap();
    assert_eq!(
        problems(&wrong),
        ["The dark version has a light background."]
    );
    let mut light = registry().get("gruvbox", Variant::Light).unwrap().clone();
    light.colors.bg = c("#000000");
    light.colors.surface = c("#000000");
    light.colors.surface_alt = c("#000000");
    light.colors.accent = c("#ffffff");
    assert!(problems(&light).contains(&"The light version has a dark background.".to_string()));
}

#[test]
fn problem_text_cuts_the_ratio_and_names_the_variant() {
    let f = ContrastFailure {
        fg: "text",
        bg: "surface_alt",
        ratio: 4.49,
    };
    let s = problem_text(Variant::Light, &f);
    assert_eq!(
        s,
        "Text on raised panels is hard to read in the light version (4.4 to 1; it needs 4.5 to 1)."
    );
}

#[test]
fn base_colors_serde_is_hex_and_strict() {
    let b = base_of(&fixture());
    let json = serde_json::to_value(b).unwrap();
    assert_eq!(json["bg"], "#282828");
    assert_eq!(json["good"], "#98971a");
    assert_eq!(
        serde_json::from_value::<BaseColors>(json.clone()).unwrap(),
        b
    );
    let mut extra = json.clone();
    extra["glow"] = "#000000".into();
    assert!(serde_json::from_value::<BaseColors>(extra).is_err());
    let mut bad = json;
    bad["bg"] = "#fff".into();
    assert!(serde_json::from_value::<BaseColors>(bad).is_err());
    assert_eq!(
        KEYS.map(|(k, _)| k),
        ["bg", "surface", "text", "accent", "good", "warn", "bad"]
    );
    let json = serde_json::to_value(b).unwrap();
    for (k, label) in KEYS {
        assert!(json.get(k).is_some(), "{k}");
        assert!(!label.is_empty());
    }
}
