use std::path::PathBuf;
use uq_core::contrast::{check_theme, luminance};
use uq_core::theme::{ThemeRegistry, Variant};

fn themes_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes")
}

const IDS: [&str; 7] = ["dracula", "forest", "graphite", "gruvbox", "matcha", "rosewood", "velvet"];

#[test]
fn all_fourteen_variants_load() {
    let reg = ThemeRegistry::load_dir(&themes_dir()).unwrap();
    assert_eq!(reg.ids(), IDS.to_vec());
    for id in IDS {
        for v in Variant::ALL {
            assert!(reg.get(id, v).is_some(), "{id} {v}");
        }
    }
}

#[test]
fn all_variants_pass_contrast() {
    let reg = ThemeRegistry::load_dir(&themes_dir()).unwrap();
    let mut problems = Vec::new();
    for t in reg.iter() {
        for f in check_theme(t) {
            problems.push(format!("{} {}: {} on {} = {:.2}", t.id, t.variant, f.fg, f.bg, f.ratio));
        }
    }
    assert!(problems.is_empty(), "contrast failures:\n{}", problems.join("\n"));
}

#[test]
fn dark_variants_are_dark() {
    let reg = ThemeRegistry::load_dir(&themes_dir()).unwrap();
    for t in reg.iter() {
        let (bg, text) = (luminance(t.colors.bg), luminance(t.colors.text));
        match t.variant {
            Variant::Dark => assert!(bg < text, "{} dark has a light background", t.id),
            Variant::Light => assert!(bg > text, "{} light has a dark background", t.id),
        }
    }
}
