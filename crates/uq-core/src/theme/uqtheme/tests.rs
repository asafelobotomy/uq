use super::*;
use crate::theme::ThemeRegistry;
use crate::theme::tests::FIXTURE;
use serde_json::json;
use std::fs;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn light_text() -> String {
    fs::read_to_string(repo().join("themes/gruvbox/light.toml"))
        .unwrap()
        .replacen("name = \"Gruvbox\"", "name = \"Fixture\"", 1)
}

fn good() -> UqTheme {
    let d = Theme::from_toml(FIXTURE, "user-x", Variant::Dark).unwrap();
    let l = Theme::from_toml(&light_text(), "user-x", Variant::Light).unwrap();
    from_pair(&d, &l, Some("Sam"), None)
}

fn good_text() -> String {
    to_text(&good())
}

/// The good file as a table, changed by `f`, and written back.
fn edited(f: impl FnOnce(&mut toml::Table)) -> String {
    let mut t: toml::Table = good_text().parse().unwrap();
    f(&mut t);
    toml::to_string(&t).unwrap()
}

fn sub<'a>(t: &'a mut toml::Table, path: &[&str]) -> &'a mut toml::Table {
    let mut cur = t;
    for k in path {
        cur = cur.get_mut(*k).unwrap().as_table_mut().unwrap();
    }
    cur
}

fn val(v: impl Into<toml::Value>) -> toml::Value {
    v.into()
}

const HOSTILE_NAMES: [&str; 11] = [
    "X\nshell /bin/sh -c evil",
    "X */ * { color: red } /*",
    "{{ 7*7 }}",
    "{% if 1 %}",
    "{# x",
    "X\u{202e}",
    "X\u{200b}",
    "X\r",
    " X",
    "",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
];

fn dark_problem(e: ImportError) -> user::Problem {
    match e {
        ImportError::Variant {
            variant: Variant::Dark,
            problem,
        } => problem,
        other => panic!("expected a dark version problem, got {other:?}"),
    }
}

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
fn parse_refuses_too_big() {
    let pad = "#".repeat(65_537);
    assert_eq!(parse(&pad), Err(ImportError::TooBig));
    let padded = format!(
        "{}\n# {}\n",
        good_text(),
        "x".repeat(MAX_FILE_BYTES as usize)
    );
    assert_eq!(parse(&padded), Err(ImportError::TooBig));
    assert!(parse(&good_text()).is_ok());
}

#[test]
fn parse_refuses_nested() {
    let dotted = format!("format = 1\n[{}]\n", vec!["a"; 5000].join("."));
    assert!(parse(&dotted).is_err());
    let inline = format!(
        "format = 1\na = {}1{}\n",
        "{a=".repeat(5000),
        "}".repeat(5000)
    );
    assert!(parse(&inline).is_err());
    let arrays = format!("format = 1\na = {}{}\n", "[".repeat(5000), "]".repeat(5000));
    assert!(parse(&arrays).is_err());
    let under_dark = format!("{}\n[dark.{}]\n", good_text(), vec!["a"; 5000].join("."));
    assert!(parse(&under_dark).is_err());
}

#[test]
fn parse_refuses_unknown_keys() {
    let top = edited(|t| {
        t.insert("script".into(), val("x"));
    });
    assert_eq!(parse(&top), Err(ImportError::Unknown("script".into())));
    let nested = edited(|t| {
        sub(t, &["dark", "colors"]).insert("glow".into(), val("#000000"));
    });
    assert_eq!(
        dark_problem(parse(&nested).unwrap_err()),
        user::Problem::BadFile(Variant::Dark)
    );
    let shell = edited(|t| {
        sub(t, &["dark"]).insert("shell".into(), toml::Value::Table(toml::Table::new()));
    });
    assert_eq!(
        dark_problem(parse(&shell).unwrap_err()),
        user::Problem::BadFile(Variant::Dark)
    );
    // The shown key has no control characters and is cut short.
    let wild = edited(|t| {
        t.insert(format!("a\u{1b}[31m{}", "b".repeat(100)), val(1));
    });
    let Err(ImportError::Unknown(k)) = parse(&wild) else {
        panic!("expected Unknown")
    };
    assert!(
        k.chars().count() <= 40 && !k.chars().any(char::is_control),
        "{k:?}"
    );
}

#[test]
fn parse_refuses_bad_values() {
    let bad_file = |t: String| {
        assert_eq!(
            dark_problem(parse(&t).unwrap_err()),
            user::Problem::BadFile(Variant::Dark)
        )
    };
    bad_file(edited(|t| {
        let ansi = sub(t, &["dark", "terminal"]).get_mut("ansi").unwrap();
        ansi.as_array_mut().unwrap().push(val("#000000"));
    }));
    bad_file(edited(|t| {
        sub(t, &["dark", "ui"]).insert("radius".into(), val(300));
    }));
    bad_file(edited(|t| {
        sub(t, &["dark", "colors"]).insert("bg".into(), val("#fff"));
    }));
    bad_file(edited(|t| {
        sub(t, &["dark", "colors"]).insert("bg".into(), val("rgb(1,2,3)"));
    }));
    bad_file(edited(|t| {
        sub(t, &["dark", "fonts"]).insert("size".into(), val("11"));
    }));
    // In range for the type but not for a theme.
    let e = parse(&edited(|t| {
        sub(t, &["dark", "ui"]).insert("radius".into(), val(41));
    }))
    .unwrap_err();
    assert_eq!(dark_problem(e), user::Problem::OutOfRange("corner radius"));
}

#[test]
fn parse_refuses_format() {
    let no_format = edited(|t| {
        t.remove("format");
    });
    assert_eq!(parse(&no_format), Err(ImportError::NotATheme));
    let text = edited(|t| {
        t.insert("format".into(), val("1"));
    });
    assert_eq!(parse(&text), Err(ImportError::NotATheme));
    let newer = edited(|t| {
        t.insert("format".into(), val(2));
    });
    assert_eq!(parse(&newer), Err(ImportError::NewerFormat(2)));
    assert_eq!(parse("not toml at all ["), Err(ImportError::NotATheme));
}

#[test]
fn parse_refuses_hostile_text() {
    for name in HOSTILE_NAMES {
        let as_name = edited(|t| {
            t.insert("name".into(), val(name));
        });
        assert_eq!(parse(&as_name), Err(ImportError::BadName), "{name:?}");
        if name.is_empty() {
            continue; // an empty author means none
        }
        let as_author = edited(|t| {
            t.insert("author".into(), val(name));
        });
        assert_eq!(parse(&as_author), Err(ImportError::BadAuthor), "{name:?}");
    }
    let not_text = edited(|t| {
        t.insert("name".into(), val(5));
    });
    assert_eq!(parse(&not_text), Err(ImportError::BadName));
    let not_text = edited(|t| {
        t.insert("author".into(), val(5));
    });
    assert_eq!(parse(&not_text), Err(ImportError::BadAuthor));
}

#[test]
fn parse_refuses_traversal_ids() {
    let long = "a".repeat(41);
    for id in [
        "../x",
        "user-../x",
        "user-",
        "user-A",
        "user-a/b",
        "user-a\u{0}",
        "user-user-",
        "../../.config/hypr",
        "",
        "-x",
        long.as_str(),
    ] {
        let r = parse(&edited(|t| {
            t.insert("id".into(), val(id));
        }));
        assert_eq!(r, Err(ImportError::BadId), "{id:?}");
    }
    let r = parse(&edited(|t| {
        t.insert("id".into(), val(5));
    }));
    assert_eq!(r, Err(ImportError::BadId));
}

#[test]
fn parse_refuses_missing_variant() {
    let no_light = edited(|t| {
        t.remove("light");
    });
    assert_eq!(
        parse(&no_light),
        Err(ImportError::Variant {
            variant: Variant::Light,
            problem: user::Problem::MissingVariant(Variant::Light)
        })
    );
    let no_dark = edited(|t| {
        t.insert("dark".into(), val(3));
    });
    assert_eq!(
        dark_problem(parse(&no_dark).unwrap_err()),
        user::Problem::BadFile(Variant::Dark)
    );
    let partial = edited(|t| {
        sub(t, &["dark"]).remove("icons");
    });
    assert_eq!(
        dark_problem(parse(&partial).unwrap_err()),
        user::Problem::BadFile(Variant::Dark)
    );
}

#[test]
fn parse_refuses_low_contrast() {
    let e = parse(&edited(|t| {
        let surface = sub(t, &["dark", "colors"])["surface"].clone();
        sub(t, &["dark", "colors"]).insert("text_muted".into(), surface);
    }))
    .unwrap_err();
    assert!(e.to_string().contains("dark"), "{e}");
    assert!(
        !e.to_string().starts_with("The dark version: The dark"),
        "{e}"
    );
    assert!(matches!(dark_problem(e), user::Problem::HardToRead { .. }));
    let e = parse(&edited(|t| {
        let light = sub(t, &["light", "colors"]).clone();
        *sub(t, &["dark", "colors"]) = light;
    }))
    .unwrap_err();
    assert_eq!(
        dark_problem(e),
        user::Problem::WrongBrightness(Variant::Dark)
    );
}

#[test]
fn read_file_refuses_big_binary_and_fifo() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    fs::write(p.join("ok.uqtheme"), good_text()).unwrap();
    assert_eq!(read_file(&p.join("ok.uqtheme")).unwrap(), good_text());
    fs::write(p.join("big"), vec![b' '; 65_537]).unwrap();
    assert_eq!(read_file(&p.join("big")), Err(ImportError::TooBig));
    fs::write(p.join("bin"), [0xff, 0xfe, 0, 1, 2]).unwrap();
    assert_eq!(read_file(&p.join("bin")), Err(ImportError::NotATheme));
    assert!(matches!(
        read_file(&p.join("missing")),
        Err(ImportError::Unreadable(_))
    ));
    std::os::unix::fs::symlink(p.join("ok.uqtheme"), p.join("link")).unwrap();
    assert!(
        read_file(&p.join("link")).is_ok(),
        "a file the user picked may be a link"
    );
    assert!(matches!(read_file(p), Err(ImportError::Unreadable(_))));
    assert!(
        std::process::Command::new("mkfifo")
            .arg(p.join("pipe"))
            .status()
            .unwrap()
            .success()
    );
    let (tx, rx) = std::sync::mpsc::channel();
    let pipe = p.join("pipe");
    std::thread::spawn(move || tx.send(read_file(&pipe)).ok());
    let r = rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("read_file waited on a pipe");
    assert!(matches!(r, Err(ImportError::Unreadable(_))), "{r:?}");
}

/// xorshift64, so the fuzz run is the same every time.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

#[test]
fn fuzz_parse_never_panics() {
    let mut base = good();
    base.look = json!({"gaps_in": 4, "blur": false}).as_object().cloned();
    let base = to_text(&base).into_bytes();
    let snippets: [&[u8]; 8] = [
        b"\n",
        b"*/",
        b"{{",
        b"\"\"\"",
        b"[[",
        "\u{202e}".as_bytes(),
        b"../",
        b"\0",
    ];
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let (mut ok, mut err) = (0, 0);
    for _ in 0..5000 {
        let mut b = base.clone();
        for _ in 0..=rng.below(3) {
            match rng.below(5) {
                0 => {
                    let i = rng.below(b.len());
                    b[i] ^= 1 << rng.below(8);
                }
                1 => {
                    let n = rng.below(b.len());
                    b.truncate(n.max(1));
                }
                2 => {
                    let lines: Vec<&[u8]> = b.split(|&c| c == b'\n').collect();
                    let l = lines[rng.below(lines.len())].to_vec();
                    let at = rng.below(b.len());
                    b.splice(at..at, l.into_iter().chain(*b"\n"));
                }
                3 => {
                    let s = snippets[rng.below(snippets.len())];
                    let at = rng.below(b.len());
                    b.splice(at..at, s.iter().copied());
                }
                _ => {
                    let lines: Vec<&[u8]> = b.split(|&c| c == b'\n').collect();
                    let skip = rng.below(lines.len());
                    b = lines
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != skip)
                        .flat_map(|(_, l)| l.iter().copied().chain(*b"\n"))
                        .collect();
                }
            }
            if b.is_empty() {
                b.push(b'#');
            }
        }
        let text = String::from_utf8_lossy(&b);
        match parse(&text) {
            Ok(t) => {
                ok += 1;
                assert_eq!(
                    user::check_pair(&t.dark, &t.light, t.author.as_deref()),
                    Ok(())
                );
                assert!(user::valid_slug(&t.slug), "{:?}", t.slug);
                assert!(user::valid_display_text(&t.name), "{:?}", t.name);
                assert!(t.author.as_deref().is_none_or(user::valid_display_text));
                assert_eq!(t.dark.id, format!("user-{}", t.slug));
                assert_eq!(parse(&to_text(&t)), Ok(t));
            }
            Err(_) => err += 1,
        }
    }
    assert!(ok > 0 && err > 0, "ok {ok}, err {err}");
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
