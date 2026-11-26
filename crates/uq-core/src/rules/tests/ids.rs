//! Parsing and rule ids.

use super::*;

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
