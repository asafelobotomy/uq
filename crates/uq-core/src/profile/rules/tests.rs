use super::*;
use crate::profile::{KeyMemo, read_leaf, write_leaves};
use serde_json::json;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn win(id: &str, name: &str, on: bool) -> Value {
    json!({"id": id, "name": name, "enabled": on, "match": {"class": "a"},
        "effects": {"float": true}})
}

fn panel(id: &str, name: &str, on: bool) -> Value {
    json!({"id": id, "name": name, "enabled": on, "match": {"namespace": "n"},
        "effects": {"blur": true}})
}

fn state() -> State {
    let mut st = State::default();
    st.set_section(
        "rules",
        json!({"window": [win("r-000a", "Float A", true), win("r-000b", "B", false)],
            "layer": [panel("r-00c0", "Blur notifications", true)],
            "workspace": [{"workspace": "3", "persistent": true}]}),
    )
    .unwrap();
    st
}

fn with(body: &str) -> String {
    format!("format = 1\nid = \"x\"\nname = \"X\"\n{body}")
}

#[test]
fn rules_tree_validation() {
    let p = Profile::parse(&with("[rules]\n\"r-7f2c\" = false\n")).unwrap();
    assert_eq!(p.values["rules"], json!({"r-7f2c": false}));
    let text = p.to_toml();
    assert!(text.contains("[rules]"), "{text}");
    assert_eq!(Profile::parse(&text), Ok(p));
    for (body, key, reason) in [
        ("[rules]\n\"x\" = true\n", "rules.x", NOT_AN_ID),
        ("[rules]\n\"r-ZZZZ\" = true\n", "rules.r-ZZZZ", NOT_AN_ID),
        (
            "[rules]\n\"r-7f2c\" = \"off\"\n",
            "rules.r-7f2c",
            ONLY_ON_OFF,
        ),
        (
            "[rules]\n\"r-7f2c\" = [true]\n",
            "rules.r-7f2c",
            ONLY_ON_OFF,
        ),
        (
            "[rules.enabled]\n\"r-7f2c\" = true\n",
            "rules.enabled",
            NOT_AN_ID,
        ),
        ("[rules.r-7f2c]\non = true\n", "rules.r-7f2c", ONLY_ON_OFF),
        (
            "[rules]\n\"r-\\u001b7f2c\" = true\n",
            "rules.r-7f2c",
            NOT_AN_ID,
        ),
    ] {
        let e = Profile::parse(&with(body)).unwrap_err();
        assert_eq!(
            e,
            ProfileError::Invalid {
                key: key.into(),
                reason: reason.into()
            },
            "{body:?}"
        );
        let text = e.to_string();
        assert!(
            !text.chars().any(char::is_control),
            "{body:?} gave {text:?}"
        );
        assert!(!text.contains('\n'), "{text:?}");
        assert_eq!(text, format!("A rule: {reason}"), "{body:?}");
    }
    assert_eq!(
        Profile::parse(&with("[keybinds]\n\"uq.spotlight\" = false\n")),
        Err(ProfileError::NeedsNewer("keybinds".into()))
    );
}

#[test]
fn read_and_write_rule_leaves() {
    let mut st = state();
    assert_eq!(read_leaf(&st, "rules", &s(&["r-000a"])), Some(json!(true)));
    assert_eq!(read_leaf(&st, "rules", &s(&["r-00c0"])), Some(json!(true)));
    assert_eq!(read_leaf(&st, "rules", &s(&["r-ffff"])), None);
    assert_eq!(read_leaf(&st, "rules", &s(&["r-000a", "x"])), None);
    let before = st.rules.clone();
    write_leaves(
        &mut st,
        "rules",
        &[
            (s(&["r-000a"]), Some(json!(false))),
            (s(&["r-00c0"]), Some(json!(false))),
        ],
    )
    .unwrap();
    assert_eq!(enabled_of(&st, "r-000a"), Some(false));
    assert_eq!(enabled_of(&st, "r-00c0"), Some(false));
    // Only the switches changed.
    let mut back = st.rules.clone();
    back.window[0].enabled = true;
    back.layer[0].enabled = true;
    assert_eq!(back, before);
    let now = st.clone();
    for leaf in [
        (s(&["r-000b"]), Some(json!("on"))),
        (s(&["r-000b"]), None),
        (s(&["r-ffff"]), Some(json!(true))),
        (s(&["r-000b", "x"]), Some(json!(true))),
    ] {
        // The good leaf before the bad one isn't written either.
        let both = [(s(&["r-000a"]), Some(json!(true))), leaf.clone()];
        assert!(write_leaves(&mut st, "rules", &both).is_err(), "{leaf:?}");
        assert_eq!(st, now, "{leaf:?}");
    }
    assert!(set_enabled(&mut st, "r-000b", true));
    assert!(!set_enabled(&mut st, "r-ffff", true));
    assert_eq!(enabled_of(&st, "r-000b"), Some(true));
}

#[test]
fn rule_keys_and_labels() {
    let st = state();
    let keys = rule_keys(&st.rules);
    let got: Vec<_> = keys
        .iter()
        .map(|k| (k.path.clone(), k.value.clone()))
        .collect();
    assert_eq!(
        got,
        vec![
            (s(&["r-000a"]), json!(true)),
            (s(&["r-000b"]), json!(false)),
            (s(&["r-00c0"]), json!(true)),
        ]
    );
    assert!(keys.iter().all(|k| k.domain == "rules"));
    assert_eq!(rule_label(&st.rules, "r-000a"), "Window rule “Float A”");
    assert_eq!(
        rule_label(&st.rules, "r-00c0"),
        "Panel rule “Blur notifications”"
    );
    assert_eq!(
        rule_label(&st.rules, "r-ffff"),
        "A rule that isn't on this computer"
    );
    assert_eq!(
        crate::profile::key_label("rules", &s(&["r-000a"])),
        "A rule"
    );
    assert_eq!(
        missing_rules_notice(1),
        "1 rule in this profile isn't on this computer, so it was left out."
    );
    assert_eq!(
        missing_rules_notice(3),
        "3 rules in this profile aren't on this computer, so they were left out."
    );
}

#[test]
fn rule_toggles_classifies() {
    let sect = |w: Vec<Value>, ws: Value| json!({"window": w, "layer": [], "workspace": ws});
    let ws3 = json!([{"workspace": "3", "persistent": true}]);
    let before = sect(
        vec![
            win("r-000a", "A", true),
            win("r-000b", "B", true),
            win("r-000d", "D", true),
        ],
        ws3.clone(),
    );
    // A toggle.
    let after = sect(
        vec![
            win("r-000a", "A", false),
            win("r-000b", "B", true),
            win("r-000d", "D", true),
        ],
        ws3.clone(),
    );
    assert_eq!(rule_toggles(&before, &after), (s(&["r-000a"]), vec![]));
    // A toggle plus a rename is still a toggle.
    let after = sect(
        vec![
            win("r-000a", "A2", false),
            win("r-000b", "B", true),
            win("r-000d", "D", true),
        ],
        ws3.clone(),
    );
    assert_eq!(rule_toggles(&before, &after), (s(&["r-000a"]), vec![]));
    // Added (r-000e), removed (r-000d), edited (r-000b), and A and B moved: each once.
    let after = sect(
        vec![
            win("r-000b", "B2", true),
            win("r-000a", "A", true),
            win("r-000e", "E", true),
        ],
        json!([{"workspace": "3", "persistent": false}]),
    );
    let (toggles, mut other) = rule_toggles(&before, &after);
    other.sort();
    assert!(toggles.is_empty());
    assert_eq!(
        other,
        s(&["order:window", "r-000b", "r-000d", "r-000e", "workspace:3"])
    );
    // A panel rule turned on.
    let b = json!({"layer": [panel("r-00c0", "P", false)]});
    let a = json!({"layer": [panel("r-00c0", "P", true)]});
    assert_eq!(rule_toggles(&b, &a), (s(&["r-00c0"]), vec![]));
}

#[test]
fn mark_rules_only_overrides_uq() {
    let mut st = state();
    let p = Profile::parse(&with(
        "[rules]\n\"r-000a\" = false\n\"r-000b\" = true\n\"r-00c0\" = false\n",
    ))
    .unwrap();
    let active = ActiveProfile {
        id: "x".into(),
        name: "Gaming".into(),
        since: 0,
        keys: p
            .keys()
            .into_iter()
            .map(|k| KeyMemo {
                before: None,
                set: k.value,
                domain: k.domain,
                path: k.path,
            })
            .collect(),
        power: None,
    };
    for k in p.keys() {
        set_enabled(&mut st, &k.path[0], k.value == json!(true));
    }
    // By hand since: B switched off again.
    set_enabled(&mut st, "r-000b", false);
    let lua = "hl.window_rule({ name = \"uq:r-000a\", enabled = true })\n";
    let mut rows = crate::rules::explain(&st.rules, Some(lua), None, "/u.lua");
    mark_rules(&mut rows, &st.rules, &active);
    assert_eq!(rows["window.r-000a"]["source"], "user_lua");
    assert_eq!(rows["window.r-000b"]["source"], "uq");
    assert_eq!(rows["layer.r-00c0"]["source"], "profile");
    assert_eq!(rows["layer.r-00c0"]["detail"], json!({"name": "Gaming"}));
    assert_eq!(rows["workspace.3"]["source"], "uq");
    let mut rows = crate::rules::explain(&st.rules, None, Some("bad"), "/u.lua");
    mark_rules(&mut rows, &st.rules, &active);
    for k in ["rules", "window.r-000a", "layer.r-00c0"] {
        assert_eq!(rows[k]["source"], "error", "{k}");
    }
}
