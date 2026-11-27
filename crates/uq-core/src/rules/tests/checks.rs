//! What a rule check refuses.

use super::*;

#[test]
fn check_pattern_refuses_what_re2_would_not_take() {
    let long = "a".repeat(257);
    let refused = [
        "negative:foo",
        "[a&&b]",
        "[a--b]",
        "[a~~b]",
        "(",
        ")",
        "*a",
        &long,
        "a\u{7}",
        "(?:a{1000}){1000}",
        "(?x)a b",
        r"\<word",
        r"\b{start}a",
        "a{1001}",
        "a{2,1001}",
        r"\p{Greek}",
        r"[\p{Greek}]",
        r"(\w)\1",
        "(?=a)",
        "",
    ];
    for p in refused {
        match check_pattern("rules.window[0].match.class", p) {
            Err(SettingError::Invalid { key, .. }) => {
                assert_eq!(key, "rules.window[0].match.class")
            }
            other => panic!("{p:?} should be refused, got {other:?}"),
        }
    }
    let ok = [
        r"^(org\.pulseaudio\.pavucontrol)$",
        "firefox",
        "(?i)fire.*",
        "a{1000}",
        r".*\.pdf - Viewer",
        r"\bfoo\B",
        r"[a-z0-9_]+\d\w",
    ];
    for p in ok {
        assert!(check_pattern("k", p).is_ok(), "{p:?}");
    }
}

#[test]
fn refusals_are_plain() {
    let long = "a".repeat(257);
    let refused = [
        "negative:foo",
        "[a&&b]",
        "(",
        "*a",
        &long,
        "a\u{7}",
        "(?:a{1000}){1000}",
        "(?x)a b",
        r"\<word",
        "a{1001}",
        r"\p{Greek}",
        r"(\w)\1",
        "",
    ];
    for p in refused {
        let reason = reason_of(check_pattern("k", p));
        assert!(!reason.contains('\n'), "{reason:?}");
        assert!(!reason.to_lowercase().contains("regex"), "{reason:?}");
        assert!(
            reason.starts_with("this pattern can't be used: "),
            "{reason:?}"
        );
        if p.len() > 2 && !p.contains('\u{7}') {
            assert!(!reason.contains(p), "{reason:?} repeats the pattern");
        }
    }
    assert!(reason_of(check_pattern("k", "(")).contains("isn't closed"));
    assert!(reason_of(check_pattern("k", "*a")).contains("has nothing before it"));
    assert!(reason_of(check_pattern("k", "(?:a{1000}){1000}")).contains("too complex"));
    // The length limit keeps nesting under the parser's, so this mapping is checked directly.
    let deep = regex_syntax::ast::ErrorKind::NestLimitExceeded(250);
    assert!(check::parse_reason(&deep).contains("too complex"));
}

#[test]
fn needs_a_match_and_an_effect() {
    assert_eq!(
        key_of(parse(one(json!({}), json!({"float": true})))),
        "rules.window[0].match"
    );
    assert_eq!(
        key_of(parse(one(json!({"class": "a"}), json!({})))),
        "rules.window[0].effects"
    );
    let r = Rules::from_section(&one(json!({"class": "a"}), json!({})), &Rules::default());
    let Err(SettingError::Invalid { reason, .. }) = r else {
        panic!()
    };
    assert_eq!(reason, "a rule needs at least one effect");
    let r = parse(section(
        vec![],
        vec![json!({"name": "L", "enabled": true, "match": {"namespace": "x"}, "effects": {}})],
        vec![],
    ));
    assert_eq!(key_of(r), "rules.layer[0].effects");
    let r = parse(section(vec![], vec![], vec![json!({"workspace": "3"})]));
    assert_eq!(key_of(r), "rules.workspace[0]");
}

#[test]
fn refuses_contradictions() {
    assert_eq!(
        key_of(parse(one(
            json!({"class": "a"}),
            json!({"float": true, "tile": true})
        ))),
        "rules.window[0].effects"
    );
    assert!(
        parse(one(
            json!({"class": "a"}),
            json!({"float": true, "tile": false})
        ))
        .is_ok()
    );
    assert!(
        parse(one(
            json!({"class": "a"}),
            json!({"move": [1, 2], "center": true})
        ))
        .is_err()
    );
    assert!(
        parse(one(
            json!({"class": "a"}),
            json!({"move": [1, 2], "center": false})
        ))
        .is_ok()
    );
    assert!(
        parse(one(
            json!({"class": "a"}),
            json!({"float": true, "silent": true})
        ))
        .is_err()
    );
    assert!(
        parse(one(
            json!({"class": "a"}),
            json!({"workspace": "3", "silent": true})
        ))
        .is_ok()
    );
}

#[test]
fn ranges() {
    let bad = [
        json!({"opacity": 0.05}),
        json!({"opacity": 1.2}),
        json!({"rounding": 21}),
        json!({"border_size": 21}),
        json!({"size": [0, 10]}),
        json!({"size": [10, 16385]}),
        json!({"move": [20000, 0]}),
        json!({"move": [0, -20000]}),
        json!({"monitor": "DP 1"}),
    ];
    for e in bad {
        assert!(parse(one(json!({"class": "a"}), e.clone())).is_err(), "{e}");
    }
    let good = [
        json!({"opacity": 0.1}),
        json!({"opacity": 1.0}),
        json!({"rounding": 20}),
        json!({"border_size": 0}),
        json!({"size": [1, 16384]}),
        json!({"move": [-16384, 16384]}),
        json!({"monitor": "DP-1"}),
    ];
    for e in good {
        assert!(parse(one(json!({"class": "a"}), e.clone())).is_ok(), "{e}");
    }
    let r = parse(one(json!({"class": "a"}), json!({"opacity": 0.333}))).unwrap();
    assert_eq!(r.window[0].effects.opacity, Some(0.33));
    let l = |e: Value| {
        parse(section(
            vec![],
            vec![json!({"name": "L", "enabled": true, "match": {"namespace": "x"}, "effects": e})],
            vec![],
        ))
    };
    assert!(l(json!({"ignore_alpha": 1.5})).is_err());
    assert!(l(json!({"ignore_alpha": -0.1})).is_err());
    assert_eq!(
        l(json!({"ignore_alpha": 0.456})).unwrap().layer[0]
            .effects
            .ignore_alpha,
        Some(0.46)
    );
    let w = |f: &str, n: u32| {
        parse(section(
            vec![],
            vec![],
            vec![json!({"workspace": "3", f: n})],
        ))
    };
    assert!(w("gaps_in", 101).is_err());
    assert!(w("gaps_out", 101).is_err());
    assert!(w("border_size", 21).is_err());
    assert!(w("gaps_in", 100).is_ok());
}

#[test]
fn closed_values() {
    assert!(
        parse(one(
            json!({"class": "a"}),
            json!({"idle_inhibit": "sometimes"})
        ))
        .is_err()
    );
    assert!(parse(one(json!({"class": "a"}), json!({"idle_inhibit": "focus"}))).is_ok());
    let l = |e: Value| {
        parse(section(
            vec![],
            vec![json!({"name": "L", "enabled": true, "match": {"namespace": "x"}, "effects": e})],
            vec![],
        ))
    };
    assert!(l(json!({"animation": "spin"})).is_err());
    assert!(l(json!({"animation": "slide"})).is_ok());
    for tag in ["a b", "uq_ws_3", "", &"t".repeat(33)] {
        assert!(
            parse(one(json!({"tag": tag}), json!({"float": true}))).is_err(),
            "{tag:?}"
        );
        assert!(
            parse(one(json!({"class": "a"}), json!({"tag": tag}))).is_err(),
            "{tag:?}"
        );
    }
    assert!(parse(one(json!({"tag": "media.player-1"}), json!({"tag": "x_y"}))).is_ok());
}

#[test]
fn limits() {
    let w = |n: usize| {
        (0..n)
            .map(|i| {
                win(
                    &format!("R{i}"),
                    json!({"class": "a"}),
                    json!({"float": true}),
                )
            })
            .collect::<Vec<_>>()
    };
    assert!(parse(section(w(200), vec![], vec![])).is_ok());
    let r = parse(section(w(201), vec![], vec![]));
    let Err(SettingError::Invalid { key, reason }) = r else {
        panic!()
    };
    assert_eq!(
        (key.as_str(), reason.as_str()),
        ("rules.window", "at most 200 window rules")
    );
    let l = |n: usize| {
        (0..n)
            .map(|i| json!({"name": format!("L{i}"), "enabled": true, "match": {"namespace": "x"}, "effects": {"blur": true}}))
            .collect::<Vec<_>>()
    };
    assert!(parse(section(vec![], l(50), vec![])).is_ok());
    assert!(parse(section(vec![], l(51), vec![])).is_err());
    let ws = |n: usize| {
        (0..n)
            .map(|i| json!({"workspace": format!("{}", i + 1), "persistent": true}))
            .collect::<Vec<_>>()
    };
    assert!(parse(section(vec![], vec![], ws(64))).is_ok());
    assert!(parse(section(vec![], vec![], ws(65))).is_err());
    let named = |n: &str| {
        section(
            vec![win(n, json!({"class": "a"}), json!({"float": true}))],
            vec![],
            vec![],
        )
    };
    assert!(parse(named(&"n".repeat(100))).is_ok());
    assert_eq!(
        key_of(parse(named(&"n".repeat(101)))),
        "rules.window[0].name"
    );
    assert!(parse(named("a\nb")).is_err());
    assert!(parse(named("")).is_err());
}

#[test]
fn refuses_hostile_rules() {
    let bad = [
        one(json!({"class": "a", "bogus": 1}), json!({"float": true})),
        one(json!({"class": "a"}), json!({"float": true, "exec": "x"})),
        json!({"window": [], "layer": [], "workspace": [], "bogus": []}),
        section(
            vec![
                json!({"name": "A", "enabled": true, "match": {"class": "a"}, "effects": {"float": true}, "extra": 1}),
            ],
            vec![],
            vec![],
        ),
        section(
            vec![],
            vec![],
            vec![json!({"workspace": "3", "persistent": true, "x": 1})],
        ),
        section(
            vec![
                json!({"name": "A", "enabled": "yes", "match": {"class": "a"}, "effects": {"float": true}}),
            ],
            vec![],
            vec![],
        ),
        one(json!({"class": "a"}), json!({"size": [1]})),
        one(json!({"class": 5}), json!({"float": true})),
        one(json!({"class": "a"}), json!({"float": "yes"})),
        json!({"window": {"a": 1}}),
        json!([]),
        json!({"window": [5]}),
    ];
    for v in bad {
        assert!(parse(v.clone()).is_err(), "{v}");
    }
    assert_eq!(
        key_of(parse(one(
            json!({"class": "a", "bogus": 1}),
            json!({"float": true})
        ))),
        "rules.window[0].match"
    );
}
