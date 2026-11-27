use super::*;
use std::fs;

pub(crate) const FIXTURE: &str = r##"
name = "Fixture"

[colors]
bg = "#282828"
surface = "#32302f"
surface_alt = "#3c3836"
overlay = "#504945"
text = "#ebdbb2"
text_muted = "#bdae93"
accent = "#d79921"
accent_text = "#282828"
border = "#665c54"
success = "#98971a"
warning = "#d79921"
error = "#cc241d"

[ui]
radius = 10
gap_in = 5
gap_out = 12
border_width = 2

[fonts]
ui = "Inter"
mono = "JetBrains Mono"
size = 11

[icons]
theme = "Papirus-Dark"
cursor_theme = "Bibata-Modern-Classic"
cursor_size = 24

[terminal]
ansi = ["#282828","#cc241d","#98971a","#d79921","#458588","#b16286","#689d6a","#a89984",
    "#928374","#fb4934","#b8bb26","#fabd2f","#83a598","#d3869b","#8ec07c","#ebdbb2"]
"##;

#[test]
fn parses_minimal_theme() {
    let t = Theme::from_toml(FIXTURE, "fixture", Variant::Dark).unwrap();
    assert_eq!(t.colors.bg, Color([0x28, 0x28, 0x28]));
    assert_eq!(t.variant, Variant::Dark);
    assert_eq!(t.id, "fixture");
    assert_eq!(t.name, "Fixture");
    assert_eq!(t.terminal.ansi[15], Color([0xeb, 0xdb, 0xb2]));
}

#[test]
fn rejects_bad_hex() {
    let src = FIXTURE.replace("bg = \"#282828\"", "bg = \"#12345\"");
    let err = Theme::from_toml(&src, "fixture", Variant::Dark).unwrap_err();
    assert!(matches!(err, ThemeError::BadColor(_)), "got {err:?}");
}

#[test]
fn with_appearance_overrides_only_set_fields() {
    let t = Theme::from_toml(FIXTURE, "fixture", Variant::Dark).unwrap();
    assert_eq!(t.with_appearance(&Appearance::default()), t);
    let a = Appearance {
        mono_font: Some("Hack".into()),
        cursor_size: Some(32),
        icon_theme: Some("Adwaita".into()),
        ..Appearance::default()
    };
    let o = t.with_appearance(&a);
    assert_eq!(o.fonts.mono, "Hack");
    assert_eq!(o.fonts.ui, "Inter");
    assert_eq!(o.fonts.size, 11);
    assert_eq!(o.icons.cursor_size, 32);
    assert_eq!(o.icons.cursor_theme, "Bibata-Modern-Classic");
    assert_eq!(o.icons.theme, "Adwaita");
    let all = Appearance {
        ui_font: Some("Cantarell".into()),
        font_size: Some(14),
        cursor_theme: Some("Breeze".into()),
        ..Appearance::default()
    };
    let o = t.with_appearance(&all);
    assert_eq!(
        (
            o.fonts.ui.as_str(),
            o.fonts.size,
            o.icons.cursor_theme.as_str()
        ),
        ("Cantarell", 14, "Breeze")
    );
}

#[test]
fn rgba_format() {
    assert_eq!(Color([0x33, 0xcc, 0xff]).rgba(0xee), "rgba(33ccffee)");
    assert_eq!(Color([0x33, 0xcc, 0xff]).hex(), "#33ccff");
}

#[test]
fn variant_round_trip() {
    assert_eq!("light".parse::<Variant>().unwrap(), Variant::Light);
    assert_eq!(Variant::Dark.to_string(), "dark");
    assert!("dim".parse::<Variant>().is_err());
}

#[test]
fn registry_rejects_missing_variant() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("solo")).unwrap();
    fs::write(dir.path().join("solo/dark.toml"), FIXTURE).unwrap();
    let err = ThemeRegistry::load_dir(dir.path()).unwrap_err();
    assert!(
        matches!(&err, ThemeError::MissingVariant { id, variant: Variant::Light } if id == "solo"),
        "got {err:?}"
    );
}

#[test]
fn registry_loads_and_gets() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("solo")).unwrap();
    fs::write(dir.path().join("solo/dark.toml"), FIXTURE).unwrap();
    fs::write(dir.path().join("solo/light.toml"), FIXTURE).unwrap();
    let reg = ThemeRegistry::load_dir(dir.path()).unwrap();
    assert_eq!(reg.ids(), vec!["solo"]);
    assert_eq!(
        reg.get("solo", Variant::Light).unwrap().variant,
        Variant::Light
    );
    assert!(reg.get("nope", Variant::Dark).is_none());
}

#[test]
fn author_is_optional_in_theme_files() {
    let with = format!("author = \"Sam\"\n{FIXTURE}");
    let (t, a) = Theme::from_toml_with_author(&with, "f", Variant::Dark).unwrap();
    assert_eq!(a.as_deref(), Some("Sam"));
    assert_eq!(t.name, "Fixture");
    let (_, a) = Theme::from_toml_with_author(FIXTURE, "f", Variant::Dark).unwrap();
    assert_eq!(a, None);
    // `Theme` itself gains no field: tokens.json is its whole serialisation.
    let json = serde_json::to_value(&t).unwrap();
    assert!(json.get("author").is_none());
}

#[test]
fn load_dir_skips_user_prefixed_system_folders() {
    let dir = tempfile::tempdir().unwrap();
    for id in ["solo", "user-sneaky"] {
        fs::create_dir(dir.path().join(id)).unwrap();
        fs::write(dir.path().join(id).join("dark.toml"), FIXTURE).unwrap();
        fs::write(dir.path().join(id).join("light.toml"), FIXTURE).unwrap();
    }
    let reg = ThemeRegistry::load_dir(dir.path()).unwrap();
    assert_eq!(reg.ids(), vec!["solo"]);
    assert!(reg.info("user-sneaky").is_none());
}

fn user_scan(names: &[&str]) -> (tempfile::TempDir, user::UserScan) {
    let dir = tempfile::tempdir().unwrap();
    for n in names {
        let id = format!("user-{}", n.to_lowercase());
        fs::create_dir(dir.path().join(&id)).unwrap();
        for v in ["dark", "light"] {
            let text = if v == "dark" {
                FIXTURE.replacen("Fixture", n, 1)
            } else {
                fs::read_to_string(
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/gruvbox/light.toml"),
                )
                .unwrap()
                .replacen("Gruvbox", n, 1)
            };
            fs::write(dir.path().join(&id).join(format!("{v}.toml")), text).unwrap();
        }
    }
    let scan = user::scan(dir.path());
    (dir, scan)
}

#[test]
fn registry_infos_mark_user_and_missing() {
    let sys = tempfile::tempdir().unwrap();
    for (id, name) in [("zeta", "Zeta"), ("alpha", "alpha")] {
        fs::create_dir(sys.path().join(id)).unwrap();
        let text = FIXTURE.replacen("Fixture", name, 1);
        fs::write(sys.path().join(id).join("dark.toml"), &text).unwrap();
        fs::write(sys.path().join(id).join("light.toml"), &text).unwrap();
    }
    let mut reg = ThemeRegistry::load_dir(sys.path()).unwrap();
    let (_keep, scan) = user_scan(&["Beta", "Apple"]);
    assert_eq!(scan.themes.len(), 2, "{:?}", scan.skipped);
    reg.replace_user(scan, None);
    let order: Vec<_> = reg.infos().iter().map(|i| i.id.as_str()).collect();
    assert_eq!(order, ["alpha", "zeta", "user-apple", "user-beta"]);
    assert!(reg.is_user("user-beta") && !reg.is_user("zeta"));
    assert!(reg.get("user-beta", Variant::Light).is_some());
    assert!(!reg.info("user-beta").unwrap().missing);
    assert_eq!(reg.ids().len(), 4);

    // The folder vanishes: the active theme's last copy stays, marked missing.
    reg.replace_user(user::UserScan::default(), Some("user-beta"));
    let i = reg.info("user-beta").unwrap();
    assert!(i.missing && i.user);
    assert!(reg.get("user-beta", Variant::Dark).is_some());
    assert!(reg.get("user-beta", Variant::Light).is_some());
    assert!(reg.info("user-apple").is_none());
    assert!(reg.get("user-apple", Variant::Dark).is_none());
    // Keeping a system id or an unknown id changes nothing.
    reg.replace_user(user::UserScan::default(), Some("zeta"));
    assert!(reg.info("user-beta").is_none());
    assert_eq!(reg.ids(), ["alpha", "zeta"]);
    // A scan that has it again clears `missing`.
    let (_k2, scan) = user_scan(&["Beta"]);
    reg.replace_user(scan, Some("user-beta"));
    assert!(!reg.info("user-beta").unwrap().missing);
}

#[test]
fn registry_skips_the_rendered_dir() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("solo")).unwrap();
    fs::write(dir.path().join("solo/dark.toml"), FIXTURE).unwrap();
    fs::write(dir.path().join("solo/light.toml"), FIXTURE).unwrap();
    fs::create_dir_all(dir.path().join("rendered/solo-dark")).unwrap();
    fs::write(dir.path().join("rendered/solo-dark/tokens.json"), "{}").unwrap();
    let reg = ThemeRegistry::load_dir(dir.path()).unwrap();
    assert_eq!(reg.ids(), vec!["solo"]);
}
