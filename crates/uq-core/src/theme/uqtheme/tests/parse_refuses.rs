//! What parsing refuses.

use super::*;

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
