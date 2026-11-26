//! Ids, display text, variant checks and messages.

use super::*;

#[test]
fn ids_and_slugs() {
    for ok in ["user-ocean", "user-a1-b2", "user-a", "user-0"] {
        assert!(valid_user_id(ok), "{ok}");
    }
    let long = format!("user-{}", "a".repeat(41));
    assert_eq!(long.len(), 46);
    for bad in [
        "user-",
        "ocean",
        "user-Ocean",
        "user--x",
        "user-x-",
        "user-../x",
        "user-a/b",
        "user-a\0",
        long.as_str(),
    ] {
        assert!(!valid_user_id(bad), "{bad:?}");
    }
    assert!(valid_user_id(&format!("user-{}", "a".repeat(40))));
    assert!(valid_slug("a-b") && !valid_slug("") && !valid_slug("a b"));
    assert_eq!(slug_for("My Ocean!", &[]), "my-ocean");
    assert_eq!(slug_for("My Ocean", &["my-ocean".into()]), "my-ocean-2");
    assert_eq!(
        slug_for("My Ocean", &["my-ocean".into(), "my-ocean-2".into()]),
        "my-ocean-3"
    );
    assert_eq!(slug_for("ëë", &[]), "theme");
    assert_eq!(slug_for(&"a".repeat(60), &[]).len(), 40);
    let full = "a".repeat(40);
    let next = slug_for(&"a".repeat(60), &[full]);
    assert_eq!(next.len(), 40);
    assert!(next.ends_with("-2") && valid_slug(&next), "{next}");
    assert!(valid_slug(&slug_for(
        "a-b-c-d-e-f-g-h-i-j-k-l-m-n-o-p-q-r-s-t-u",
        &[]
    )));
}

#[test]
fn display_text_rules() {
    for bad in HOSTILE_NAMES {
        assert!(!valid_display_text(bad), "{bad:?}");
    }
    for ok in ["Ocean", "Café Noir", "Rosé–Pine"] {
        assert!(valid_display_text(ok), "{ok}");
    }
    assert!(valid_display_text(&"a".repeat(64)));
}

#[test]
fn check_variant_ranges() {
    type Set = fn(&mut Theme, u8);
    let cases: [(&str, u8, u8, Set); 6] = [
        ("corner radius", 0, 40, |t, n| t.ui.radius = n),
        ("gap between windows", 0, 50, |t, n| t.ui.gap_in = n),
        ("gap to the screen edge", 0, 100, |t, n| t.ui.gap_out = n),
        ("border width", 0, 20, |t, n| t.ui.border_width = n),
        ("font size", 6, 32, |t, n| t.fonts.size = n),
        ("cursor size", 16, 96, |t, n| t.icons.cursor_size = n),
    ];
    for (what, lo, hi, set) in cases {
        for n in [lo, hi] {
            let mut t = dark();
            set(&mut t, n);
            assert_eq!(check_variant(&t, None), Ok(()), "{what} {n}");
        }
        for n in [hi + 1].into_iter().chain((lo > 0).then(|| lo - 1)) {
            let mut t = dark();
            set(&mut t, n);
            let e = check_variant(&t, None).unwrap_err();
            assert_eq!(e, Problem::OutOfRange(what));
            assert!(e.to_string().contains(what), "{e}");
        }
    }
    let mut t = dark();
    t.fonts.ui = "x".repeat(101);
    assert_eq!(check_variant(&t, None), Err(Problem::Font));
    t.fonts.ui = "Inter\n".into();
    assert_eq!(check_variant(&t, None), Err(Problem::Font));
    let mut t = dark();
    t.icons.theme = "../x".into();
    assert_eq!(check_variant(&t, None), Err(Problem::IconName));
    let mut t = dark();
    t.icons.cursor_theme = String::new();
    assert_eq!(check_variant(&t, None), Err(Problem::IconName));
    let mut t = dark();
    t.name = "X */".into();
    assert_eq!(check_variant(&t, None), Err(Problem::Name));
    assert_eq!(check_variant(&dark(), Some("a\nb")), Err(Problem::Author));
    assert_eq!(check_variant(&dark(), Some("Sam")), Ok(()));
}

#[test]
fn check_pair_contrast_and_brightness() {
    assert_eq!(check_pair(&dark(), &light(), None), Ok(()));
    let mut d = dark();
    d.colors.text_muted = d.colors.surface;
    let e = check_pair(&d, &light(), None).unwrap_err();
    assert!(
        matches!(
            &e,
            Problem::HardToRead {
                variant: Variant::Dark,
                ..
            }
        ),
        "{e:?}"
    );
    assert!(e.to_string().contains("dark"), "{e}");
    // A light palette in the dark slot passes contrast but not the brightness check.
    let wrong = Theme::from_toml(&light_text(), "user-x", Variant::Dark).unwrap();
    assert_eq!(
        check_pair(&wrong, &light(), None),
        Err(Problem::WrongBrightness(Variant::Dark))
    );
    let mut l = light();
    l.name = "Other".into();
    assert_eq!(check_pair(&dark(), &l, None), Err(Problem::NamesDiffer));
}

#[test]
fn variant_toml_round_trips() {
    let reg = crate::theme::ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    assert_eq!(reg.ids().len(), 7);
    for t in reg.iter() {
        for author in [None, Some("Sam \"the\" Maker")] {
            let text = variant_toml(t, author);
            let (back, a) = Theme::from_toml_with_author(&text, &t.id, t.variant).unwrap();
            assert_eq!(&back, t, "{} {}", t.id, t.variant);
            assert_eq!(a.as_deref(), author);
        }
    }
    let mut odd = dark();
    odd.name = "a#b\"c\\d'e".into();
    let back = Theme::from_toml(&variant_toml(&odd, None), "user-x", Variant::Dark).unwrap();
    assert_eq!(back, odd);
}

#[test]
fn hostile_names_never_leave_template_comments() {
    let templates = repo().join("templates");
    let comment_files = |t: &Theme| {
        let out = crate::render::render(t, &templates, Some("KvGnome")).unwrap();
        out.0
            .into_iter()
            .filter(|(p, _)| p.extension().is_some_and(|e| e != "json"))
            .collect::<Vec<_>>()
    };
    let plain = comment_files(&dark());
    assert!(plain.len() >= 6);
    let edge = [
        "a".repeat(64),
        "a#b".into(),
        "a/*b".into(),
        "a\"b".into(),
        "a\\b".into(),
        "a'b".into(),
        "é".into(),
    ];
    for name in edge {
        assert!(valid_display_text(&name), "{name:?} should be accepted");
        let mut t = dark();
        t.name = name.clone();
        let out = comment_files(&t);
        assert_eq!(out.len(), plain.len());
        for ((path, text), (_, base)) in out.iter().zip(&plain) {
            assert_eq!(
                text.lines().count(),
                base.lines().count(),
                "{path:?} {name}"
            );
            if text.contains("Generated by uqd from") {
                let first = text
                    .lines()
                    .find(|l| l.contains("Generated by uqd from"))
                    .unwrap();
                assert!(first.contains(&name), "{path:?}: {first}");
            }
            if path.extension().is_some_and(|e| e == "css") {
                let masked = text.replace(&name, "NAME");
                assert_eq!(
                    masked.matches("/*").count(),
                    masked.matches("*/").count(),
                    "{path:?} {name}"
                );
            }
        }
    }
}

#[test]
fn messages_are_plain() {
    let v = Variant::Dark;
    let all = [
        Problem::FolderName,
        Problem::Link,
        Problem::NotAFolder,
        Problem::MissingVariant(v),
        Problem::Unreadable(v),
        Problem::BadFile(v),
        Problem::NamesDiffer,
        Problem::Name,
        Problem::Author,
        Problem::Font,
        Problem::IconName,
        Problem::OutOfRange("corner radius"),
        Problem::HardToRead {
            variant: v,
            text: "Muted text on panels is hard to read.".into(),
        },
        Problem::WrongBrightness(v),
        Problem::TooMany,
    ];
    for p in all {
        let m = p.to_string();
        for bad in ["toml", "TOML", "serde", "uqd", "{", "[", "ThemeError"] {
            assert!(!m.contains(bad), "{m:?} has {bad:?}");
        }
        assert!(
            m.ends_with('.') && m.chars().next().unwrap().is_uppercase(),
            "{m}"
        );
    }
}

#[test]
fn empty_author_in_a_folder_theme_means_none() {
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    let with = |t: String| format!("author = \"\"\n{t}");
    put(r, "user-x", &with(dark_text()), &with(light_text()));
    let s = scan(r);
    assert_eq!(s.themes.len(), 1, "{:?}", s.skipped);
    assert_eq!(s.themes[0].author, None);
}
