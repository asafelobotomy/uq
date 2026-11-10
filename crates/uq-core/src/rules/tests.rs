use super::*;
use crate::state::State;
use serde_json::{Value, json};
use std::collections::BTreeSet;

const SPEC: &str = r#"
[[rules.window]]
id = "r-7f2c"
name = "Pavucontrol floats"
enabled = true
match = { class = "^(org\\.pulseaudio\\.pavucontrol)$" }
effects = { float = true, size = [800, 600], center = true }

[[rules.layer]]
id = "r-0a91"
name = "Blur notifications"
enabled = true
match = { namespace = "^noti-fi$" }
effects = { blur = true, ignore_alpha = 0.3 }

[[rules.workspace]]
workspace = "3"
monitor = "DP-1"
default = false
persistent = true
gaps_in = 0
"#;

fn win(name: &str, m: Value, e: Value) -> Value {
    json!({"name": name, "enabled": true, "match": m, "effects": e})
}

fn section(window: Vec<Value>, layer: Vec<Value>, workspace: Vec<Value>) -> Value {
    json!({"window": window, "layer": layer, "workspace": workspace})
}

fn one(m: Value, e: Value) -> Value {
    section(vec![win("A rule", m, e)], vec![], vec![])
}

fn parse(v: Value) -> Result<Rules, SettingError> {
    Rules::from_section(&v, &Rules::default())
}

fn key_of(r: Result<Rules, SettingError>) -> String {
    match r {
        Err(SettingError::Invalid { key, .. }) => key,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn reason_of(r: Result<(), SettingError>) -> String {
    match r {
        Err(SettingError::Invalid { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn spec_example_parses() {
    let mut st = State::parse_strict(SPEC).unwrap();
    assert_eq!(st.rules.window.len(), 1);
    assert_eq!(st.rules.layer.len(), 1);
    assert_eq!(st.rules.workspace.len(), 1);
    let w = &st.rules.window[0];
    assert_eq!(w.id, "r-7f2c");
    assert_eq!(
        w.matches.class.as_deref(),
        Some(r"^(org\.pulseaudio\.pavucontrol)$")
    );
    assert_eq!(w.effects.size, Some([800, 600]));
    assert_eq!(st.rules.layer[0].effects.ignore_alpha, Some(0.3));
    assert_eq!(st.rules.workspace[0].gaps_in, Some(0));
    let v = st.section("rules").unwrap();
    assert_eq!(
        v["window"][0]["match"]["class"],
        json!(r"^(org\.pulseaudio\.pavucontrol)$")
    );
    assert!(v["window"][0]["effects"].get("move").is_none());
    st.set_section("rules", v.clone()).unwrap();
    assert_eq!(st.section("rules").unwrap(), v);
    // And through the file.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.toml");
    st.save(&path).unwrap();
    assert_eq!(State::load(&path).rules, st.rules);
}

#[test]
fn missing_ids_are_assigned_and_unique() {
    let r = parse(section(
        vec![
            json!({"id": "r-abcd", "name": "Kept", "enabled": true, "match": {"class": "a"}, "effects": {"float": true}}),
            win("One", json!({"class": "b"}), json!({"float": true})),
            json!({"id": "", "name": "Two", "enabled": true, "match": {"class": "c"}, "effects": {"float": true}}),
        ],
        vec![json!({"name": "L", "enabled": true, "match": {"namespace": "x"}, "effects": {"blur": true}})],
        vec![],
    ))
    .unwrap();
    assert_eq!(r.window[0].id, "r-abcd");
    let ids = r.ids();
    assert_eq!(ids.len(), 4);
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), 4);
    assert!(ids.iter().all(|i| valid_id(i)));
}

#[test]
fn fresh_id_avoids_taken() {
    let mut taken = BTreeSet::new();
    for _ in 0..300 {
        let id = fresh_id(&taken);
        assert!(valid_id(&id));
        assert!(taken.insert(id));
    }
}

#[test]
fn resending_the_section_is_accepted() {
    let mut st = State::parse_strict(SPEC).unwrap();
    let before = st.rules.clone();
    let v = st.section("rules").unwrap();
    st.set_section("rules", v.clone()).unwrap();
    assert_eq!(st.rules, before);
    let mut edited = v;
    edited["window"][0]["name"] = json!("Renamed");
    st.set_section("rules", edited).unwrap();
    assert_eq!(st.rules.window[0].name, "Renamed");
    assert_eq!(st.rules.window[0].id, "r-7f2c");
    assert_eq!(st.rules.layer, before.layer);
}

#[test]
fn refuses_bad_or_duplicate_ids() {
    for id in ["x", "r-ZZZZ", "r-abc", "r-123456789", "R-abcd"] {
        let v = section(
            vec![
                json!({"id": id, "name": "A", "enabled": true, "match": {"class": "a"}, "effects": {"float": true}}),
            ],
            vec![],
            vec![],
        );
        assert_eq!(key_of(parse(v)), "rules.window[0].id", "{id}");
    }
    let v = section(
        vec![
            json!({"id": "r-aaaa", "name": "A", "enabled": true, "match": {"class": "a"}, "effects": {"float": true}}),
        ],
        vec![
            json!({"id": "r-aaaa", "name": "B", "enabled": true, "match": {"namespace": "a"}, "effects": {"blur": true}}),
        ],
        vec![],
    );
    assert_eq!(key_of(parse(v)), "rules.layer[0].id");
}

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

#[test]
fn workspace_rule_selectors_normalised_and_unique() {
    let r = parse(section(
        vec![win(
            "A",
            json!({"workspace": "web"}),
            json!({"workspace": "web"}),
        )],
        vec![],
        vec![json!({"workspace": "web", "persistent": true})],
    ))
    .unwrap();
    assert_eq!(r.workspace[0].workspace, "name:web");
    assert_eq!(r.window[0].matches.workspace.as_deref(), Some("name:web"));
    assert_eq!(r.window[0].effects.workspace.as_deref(), Some("name:web"));
    let dup = section(
        vec![],
        vec![],
        vec![
            json!({"workspace": "3", "persistent": true}),
            json!({"workspace": "3", "default": true}),
        ],
    );
    assert_eq!(key_of(parse(dup)), "rules.workspace[1].workspace");
    let long = section(
        vec![],
        vec![],
        vec![json!({"workspace": "w".repeat(65), "persistent": true})],
    );
    assert_eq!(key_of(parse(long)), "rules.workspace[0].workspace");
    let m = one(json!({"workspace": "w".repeat(65)}), json!({"float": true}));
    assert_eq!(key_of(parse(m)), "rules.window[0].match.workspace");
    let e = one(json!({"class": "a"}), json!({"workspace": "w".repeat(65)}));
    assert_eq!(key_of(parse(e)), "rules.window[0].effects.workspace");
}

#[test]
fn lenient_load_drops_only_the_bad_rule() {
    let src = r#"
theme = "nord"

pinned = ["firefox.desktop"]

[[rules.window]]
id = "r-0001"
name = "Unknown effect"
enabled = true
match = { class = "a" }
effects = { bogus = true }

[[rules.window]]
id = "r-0002"
name = "Wrong type"
enabled = "yes"
match = { class = "a" }
effects = { float = true }

[[rules.window]]
id = "r-0003"
name = "Good"
enabled = true
match = { class = "a" }
effects = { float = true }
"#;
    let st = State::parse_strict(src).unwrap();
    assert_eq!(st.rules.window.len(), 1);
    assert_eq!(st.rules.window[0].id, "r-0003");
    assert_eq!(st.theme, "nord");
    assert_eq!(st.settings.pinned, ["firefox.desktop"]);
}

#[test]
fn lenient_load_repairs_the_rest() {
    let src = r#"
[[rules.window]]
id = "r-0001"
name = "Bad pattern"
enabled = true
match = { class = "(" }
effects = { float = true }

[[rules.window]]
id = "r-0002"
name = "First"
enabled = true
match = { class = "a" }
effects = { opacity = 0.333 }

[[rules.window]]
id = "r-0002"
name = "Same id"
enabled = true
match = { class = "a" }
effects = { float = true }

[[rules.window]]
name = "No id"
enabled = true
match = { class = "a" }
effects = { float = true }

[[rules.workspace]]
workspace = "3"
persistent = true

[[rules.workspace]]
workspace = "3"
default = true
"#;
    let st = State::parse_strict(src).unwrap();
    let names: Vec<_> = st.rules.window.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["First", "Same id", "No id"]);
    let ids: BTreeSet<_> = st.rules.window.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(st.rules.window[0].id, "r-0002");
    assert!(st.rules.window.iter().all(|r| valid_id(&r.id)));
    assert_eq!(st.rules.window[0].effects.opacity, Some(0.33));
    assert!(valid_id(&st.rules.window[1].id));
    assert_eq!(st.rules.workspace.len(), 1);
    // The repaired section is writable.
    let mut st = st;
    let v = st.section("rules").unwrap();
    st.set_section("rules", v).unwrap();
}

#[test]
fn non_array_lists_load_empty() {
    let st = State::parse_strict("[rules]\nwindow = 5\n").unwrap();
    assert!(st.rules.window.is_empty());
}

#[test]
fn exact_pattern_escapes() {
    assert_eq!(exact_pattern("org.kde.dolphin"), r"^org\.kde\.dolphin$");
    assert!(check_pattern("k", &exact_pattern("a(b)[c]+*?{1}|^$ \\")).is_ok());
}

#[test]
fn defaults_are_empty() {
    let st = State::default();
    assert_eq!(
        st.section("rules").unwrap(),
        json!({"window": [], "layer": [], "workspace": []})
    );
}

#[test]
fn load_clears_bad_ids_without_dropping_rules() {
    let rule = |id: &str, n: &str| {
        format!(
            "[[rules.window]]\nid = \"{id}\"\nname = \"{n}\"\nenabled = true\n\
             match = {{ class = \"a\" }}\neffects = {{ float = true }}\n\n"
        )
    };
    let src = [
        rule("R-7F2C", "Upper"),
        rule("x", "Short"),
        rule("r-0009", "Keeps"),
    ]
    .concat();
    let st = State::parse_strict(&src).unwrap();
    assert_eq!(st.rules.window.len(), 3);
    assert_eq!(st.rules.window[2].id, "r-0009");
    assert!(st.rules.window.iter().all(|r| valid_id(&r.id)));
    assert_ne!(st.rules.window[0].id, st.rules.window[1].id);
}

#[test]
fn silent_without_a_workspace() {
    let e = |silent: bool| {
        one(
            json!({"class": "a"}),
            json!({"float": true, "silent": silent}),
        )
    };
    assert!(parse(e(true)).is_err());
    assert_eq!(parse(e(false)).unwrap().window[0].effects.silent, None);
    let src = "[[rules.window]]\nname = \"Q\"\nenabled = true\nmatch = { class = \"a\" }\n\
               effects = { float = true, silent = true }\n";
    let st = State::parse_strict(src).unwrap();
    assert_eq!(st.rules.window.len(), 1);
    assert_eq!(st.rules.window[0].effects.silent, None);
}
