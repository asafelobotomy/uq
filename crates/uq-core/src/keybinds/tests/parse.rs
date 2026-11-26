//! Parsing, overrides and ids.

use super::*;

#[test]
fn spec_shape_parses_and_round_trips() {
    let v = json!({
        "overrides": [{"id": "uq.lookseedo", "combo": "SUPER + K"},
                      {"id": "uq.close", "enabled": false}],
        "custom": [{"id": "k-3f9c", "name": "Screenshot", "combo": "SUPER + SHIFT + P",
                    "enabled": true, "locked": false, "repeating": false,
                    "action": {"kind": "command", "command": "grim -g \"$(slurp)\" ~/shot.png"}}]
    });
    let mut st = State::default();
    write(&mut st, v.clone()).unwrap();
    assert_eq!(st.section("keybinds").unwrap(), v);
    let toml = toml::to_string(&st).unwrap();
    assert!(toml.contains("[[keybinds.overrides]]") && toml.contains("[[keybinds.custom]]"));
    let back = State::parse_strict(&toml).unwrap();
    assert_eq!(back.keybinds, st.keybinds);
}

#[test]
fn ids_assigned_and_kept() {
    let mut st = State::default();
    write(
        &mut st,
        json!({"custom": [custom("A", "SUPER + F1", term()), custom("B", "SUPER + F2", term())]}),
    )
    .unwrap();
    let ids = st
        .keybinds
        .ids()
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    assert!(ids.iter().all(|i| valid_custom_id(i)) && ids[0] != ids[1]);
    for bad in ["x-1234", "k-12", "k-GGGG", "k-123456789"] {
        let mut b = custom("A", "SUPER + F1", term());
        b["id"] = json!(bad);
        assert!(
            err(json!({"custom": [b]})).contains("looks like k-"),
            "{bad}"
        );
    }
    let (mut a, mut b) = (
        custom("A", "SUPER + F1", term()),
        custom("B", "SUPER + F2", term()),
    );
    a["id"] = json!("k-0001");
    b["id"] = json!("k-0001");
    assert!(err(json!({"custom": [a, b]})).contains("another shortcut"));
}

#[test]
fn resending_the_section_is_accepted() {
    let mut st = State::default();
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.close", "combo": "SUPER + X"}],
               "custom": [custom("A", "SUPER + F1", term())]}),
    )
    .unwrap();
    let again = st.section("keybinds").unwrap();
    let before = st.keybinds.clone();
    write(&mut st, again).unwrap();
    assert_eq!(st.keybinds, before);
}

#[test]
fn override_equal_to_default_is_dropped() {
    let mut st = State::default();
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.lookseedo", "combo": "WIN + space"},
                             {"id": "uq.close", "enabled": true}]}),
    )
    .unwrap();
    assert!(st.keybinds.overrides.is_empty());
    assert_eq!(
        st.section("keybinds").unwrap(),
        json!({"overrides": [], "custom": []})
    );
}

#[test]
fn override_kind_must_match() {
    let mut st = State::default();
    assert!(
        write(
            &mut st,
            json!({"overrides": [{"id": "uq.window.drag", "combo": "SUPER + Q"}]})
        )
        .is_err()
    );
    assert!(
        write(
            &mut st,
            json!({"overrides": [{"id": "uq.close", "combo": "SUPER + mouse:272"}]})
        )
        .is_err()
    );
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.window.drag", "combo": "SUPER + mouse:274"},
                             {"id": "uq.ws.next", "combo": "SUPER + mouse_left"}]}),
    )
    .unwrap();
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.appmenu", "combo": "SUPER + D"}]}),
    )
    .unwrap();
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.appmenu", "combo": "SUPER + SUPER_R"}]}),
    )
    .unwrap();
    assert_eq!(
        st.keybinds.overrides[0].combo.as_deref(),
        Some("SUPER + SUPER_R")
    );
}

#[test]
fn unknown_default_id_refused_on_write_dropped_on_load() {
    assert!(
        err(json!({"overrides": [{"id": "uq.nope", "enabled": false}]}))
            .contains("isn't one of uQ's shortcuts")
    );
    let src = r#"
[[keybinds.overrides]]
id = "uq.nope"
enabled = false
[[keybinds.overrides]]
id = "uq.close"
enabled = false
"#;
    let st = State::parse_strict(src).unwrap();
    assert_eq!(st.keybinds.overrides.len(), 1);
    assert_eq!(st.keybinds.overrides[0].id, "uq.close");
}

#[test]
fn lenient_load_drops_only_the_bad_entry() {
    let src = r#"
theme = "rosewood"
[settings]
pinned = ["a.desktop"]

[[keybinds.custom]]
id = "k-0001"
name = "Bad"
combo = "SUPER + F1"
exec = "nope"
[keybinds.custom.action]
kind = "uq"
name = "terminal"

[[keybinds.custom]]
id = "k-0002"
name = "Conflicting"
combo = "SUPER + Space"
[keybinds.custom.action]
kind = "uq"
name = "terminal"

[[keybinds.custom]]
id = "k-0003"
name = "Good"
combo = "SUPER + F3"
[keybinds.custom.action]
kind = "uq"
name = "terminal"
"#;
    let st = State::parse_strict(src).unwrap();
    assert_eq!(st.keybinds.custom.len(), 1);
    assert_eq!(st.keybinds.custom[0].name, "Good");
    assert_eq!(st.theme, "rosewood");
}

#[test]
fn effective_lists_defaults_then_custom() {
    let mut st = State::default();
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.lookseedo", "combo": "SUPER + K"}, {"id": "uq.close", "enabled": false}],
               "custom": [custom("Mine", "SUPER + F1", term())]}),
    )
    .unwrap();
    let eff = st.keybinds.effective();
    assert_eq!(eff.len(), 69);
    let by = |id: &str| eff.iter().find(|e| e.id == id).unwrap();
    assert_eq!(
        combo::canonical(by("uq.lookseedo").combo.unwrap()),
        "SUPER + K"
    );
    assert!(!by("uq.close").enabled && by("uq.terminal").enabled);
    assert!(by("uq.appmenu").release && by("uq.window.drag").mouse);
    assert!(!eff.last().unwrap().is_default);
}

#[test]
fn spotlight_names_migrate_to_lookseedo() {
    let kb: Keybinds = serde_json::from_value(serde_json::json!({
        "overrides": [{"id": "uq.spotlight", "combo": "SUPER + K"}],
        "custom": [{"id": "k-abcd", "name": "Mine", "combo": "SUPER + F9",
                    "action": {"kind": "shell", "name": "spotlight"}}],
    }))
    .unwrap();
    assert_eq!(kb.overrides[0].id, "uq.lookseedo");
    assert_eq!(
        kb.custom[0].action,
        Action::Shell {
            name: "lookseedo".into()
        }
    );
}
