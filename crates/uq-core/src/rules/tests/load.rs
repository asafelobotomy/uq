//! Lenient loading, workspace selectors and patterns.

use super::*;

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
