//! Round trips and messages.

use super::*;

#[test]
fn every_system_theme_round_trips() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    assert_eq!(reg.ids().len(), 7);
    for id in reg.ids() {
        let d = reg.get(id, Variant::Dark).unwrap();
        let l = reg.get(id, Variant::Light).unwrap();
        let text = to_text(&from_pair(d, l, None, None));
        assert!(text.starts_with(HEADER));
        let back = parse(&text).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(back.slug, id);
        assert_eq!(back.dark.id, format!("user-{id}"));
        for (a, b) in [(&back.dark, d), (&back.light, l)] {
            assert_eq!(a.colors, b.colors, "{id}");
            assert_eq!(a.ui, b.ui);
            assert_eq!(a.fonts, b.fonts);
            assert_eq!(a.icons, b.icons);
            assert_eq!(a.terminal, b.terminal);
            assert_eq!(a.name, b.name);
        }
        assert_eq!(to_text(&back), text, "{id}: a second export differs");
    }
}

#[test]
fn author_and_files_round_trip() {
    let t = good();
    let back = parse(&to_text(&t)).unwrap();
    assert_eq!(back, t);
    assert_eq!(back.author.as_deref(), Some("Sam"));
    let files = install_files(&t);
    let names: Vec<_> = files.0.keys().map(|p| p.to_str().unwrap()).collect();
    assert_eq!(names, ["dark.toml", "light.toml"]);
    let (d, a) = Theme::from_toml_with_author(
        &files.0[&PathBuf::from("dark.toml")],
        "user-x",
        Variant::Dark,
    )
    .unwrap();
    assert_eq!((d, a.as_deref()), (t.dark.clone(), Some("Sam")));
    // An empty author means none.
    let none = parse(&edited(|t| {
        t.insert("author".into(), val(""));
    }))
    .unwrap();
    assert_eq!(none.author, None);
}

#[test]
fn look_round_trips_and_is_checked() {
    let look = json!({"gaps_in": 4, "blur": false, "animation_speed": 1.5});
    let mut t = good();
    t.look = look.as_object().cloned();
    let text = to_text(&t);
    assert!(text.contains("[look]"), "{text}");
    assert_eq!(parse(&text).unwrap(), t);
    assert_eq!(to_text(&parse(&text).unwrap()), text);

    let with_look = |k: &str, v: toml::Value| {
        edited(|t| {
            let mut l = toml::Table::new();
            l.insert(k.into(), v);
            t.insert("look".into(), toml::Value::Table(l));
        })
    };
    assert!(matches!(
        parse(&with_look("gaps_in", val(99))),
        Err(ImportError::BadLook(m)) if m.contains("Gaps between windows") && m.contains("50")
    ));
    assert!(matches!(
        parse(&with_look("wallpaper", val("x"))),
        Err(ImportError::Unknown(k)) if k == "look.wallpaper"
    ));
    assert!(matches!(
        parse(&with_look("blur", val("yes"))),
        Err(ImportError::BadLook(_))
    ));
    assert!(matches!(
        parse(&edited(|t| {
            t.insert("look".into(), val(3));
        })),
        Err(ImportError::BadLook(_))
    ));
    assert_eq!(
        parse(&with_look("gaps_in", val(4))).unwrap().look.unwrap()["gaps_in"],
        4
    );
    // An empty table is no look at all.
    let empty = edited(|t| {
        t.insert("look".into(), toml::Value::Table(toml::Table::new()));
    });
    assert_eq!(parse(&empty).unwrap().look, None);
}

#[test]
fn id_prefix_is_stripped_once() {
    let with_id = |id: &str| {
        parse(&edited(|t| {
            t.insert("id".into(), val(id));
        }))
    };
    let a = with_id("user-ocean").unwrap();
    assert_eq!(
        (a.slug.as_str(), a.dark.id.as_str()),
        ("ocean", "user-ocean")
    );
    let b = with_id("user-user-x").unwrap();
    assert_eq!(b.slug, "user-x");
    assert_eq!(b.light.id, "user-user-x");
    let c = with_id("ocean").unwrap();
    assert_eq!(c.dark.id, "user-ocean");
    // Re-importing an export is stable.
    assert!(to_text(&b).contains("id = \"user-user-x\""));
    assert_eq!(parse(&to_text(&b)).unwrap(), b);
}

#[test]
fn messages_are_plain() {
    let v = Variant::Dark;
    let all = [
        ImportError::TooBig,
        ImportError::Unreadable("No such file or directory (os error 2)".into()),
        ImportError::NotATheme,
        ImportError::NewerFormat(2),
        ImportError::BadId,
        ImportError::BadName,
        ImportError::BadAuthor,
        ImportError::Unknown("script".into()),
        ImportError::Variant {
            variant: v,
            problem: user::Problem::OutOfRange("corner radius"),
        },
        ImportError::Variant {
            variant: v,
            problem: user::Problem::BadFile(v),
        },
        ImportError::BadLook("Gaps between windows must be between 0 and 50.".into()),
    ];
    for e in all {
        let m = e.to_string();
        for bad in ["toml", "TOML", "serde", "uqd", "{", "[", "ThemeError"] {
            assert!(!m.contains(bad), "{m:?} has {bad:?}");
        }
        assert!(m.chars().next().unwrap().is_uppercase(), "{m}");
    }
    assert_eq!(
        ImportError::Variant {
            variant: v,
            problem: user::Problem::OutOfRange("corner radius")
        }
        .to_string(),
        "The dark version: The corner radius is out of range."
    );
    assert_eq!(
        ImportError::TooBig.to_string(),
        "The file is too big to be a theme."
    );
}
