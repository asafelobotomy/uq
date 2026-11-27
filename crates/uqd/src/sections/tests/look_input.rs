//! Look, layouts, input and monitors sections.

use super::*;

#[tokio::test]
async fn set_look_renders_and_reloads() {
    let mut r = rig();
    r.desktop.startup().await;
    r.comp.calls.lock().unwrap().clear();
    r.sys.applied.lock().unwrap().clear();
    drain(&mut r.events);
    r.desktop
        .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
        .await
        .unwrap();
    assert!(generated(&r).contains("gaps_in = 9"), "{}", generated(&r));
    let calls = r.comp.calls();
    assert!(calls.contains(&"reload".to_string()), "{calls:?}");
    assert!(
        !calls.iter().any(|c| c.starts_with("setcursor")),
        "{calls:?}"
    );
    assert!(r.sys.applied.lock().unwrap().is_empty());
    let evs = drain(&mut r.events);
    assert!(!evs.iter().any(|e| matches!(e, Event::ThemeChanged { .. })));
    assert!(
        evs.iter()
            .any(|e| matches!(e, Event::SectionChanged { domain, .. } if domain == "look"))
    );
    assert!(evs.iter().any(|e| matches!(e, Event::SettingsChanged(_))));
    let saved = State::load(&r.desktop.paths().state_file);
    assert_eq!(saved.settings.look.gaps_in, Some(9));
}

#[tokio::test]
async fn rejected_look_keeps_previous_config() {
    let mut r = rig();
    r.desktop.startup().await;
    let gen_before = std::fs::read(&r.desktop.paths().generated_lua).unwrap();
    let state_before = std::fs::read(&r.desktop.paths().state_file).unwrap();
    drain(&mut r.events);
    *r.comp.verify_error.lock().unwrap() = Some("bad option".into());
    let res = r
        .desktop
        .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
        .await;
    assert_eq!(res, Err(DesktopError::ConfigInvalid("bad option".into())));
    assert_eq!(
        std::fs::read(&r.desktop.paths().generated_lua).unwrap(),
        gen_before
    );
    // startup already wrote state.toml (with an empty `[look]` table); it must be untouched.
    let state = std::fs::read(&r.desktop.paths().state_file).unwrap();
    assert_eq!(state, state_before);
    assert!(!String::from_utf8(state).unwrap().contains("gaps_in"));
    let evs = drain(&mut r.events);
    assert!(
        evs.iter().all(|e| matches!(e, Event::ConfigError(_))),
        "{evs:?}"
    );
    let look: serde_json::Value =
        serde_json::from_str(&r.desktop.get_section("look").unwrap()).unwrap();
    assert!(look["gaps_in"].is_null(), "{look}");
}

#[tokio::test]
async fn set_layouts_replaces_rules() {
    let mut r = rig();
    r.desktop
        .set_layout("2", "scrolling", Origin::Settings)
        .await
        .unwrap();
    r.comp.calls.lock().unwrap().clear();
    drain(&mut r.events);
    r.desktop
        .set_section(
            "layouts",
            r#"{"default":"master","workspaces":{"3":"master"}}"#,
            Origin::Settings,
        )
        .await
        .unwrap();
    let g = generated(&r);
    assert!(g.contains(r#"workspace = "3", layout = "master""#), "{g}");
    assert!(!g.contains(r#"workspace = "2""#), "{g}");
    assert!(r.comp.calls().contains(&"reload".to_string()));
    let saved = State::load(&r.desktop.paths().state_file);
    assert_eq!(
        saved.layouts.into_iter().collect::<Vec<_>>(),
        vec![("3".to_string(), uq_core::lua::Layout::Master)]
    );
    let evs = drain(&mut r.events);
    assert!(evs.contains(&Event::LayoutChanged {
        workspace: "3".into(),
        layout: uq_core::lua::Layout::Master
    }));
    assert!(
        evs.iter()
            .any(|e| matches!(e, Event::SectionChanged { domain, .. } if domain == "layouts"))
    );
    // The removed rule "2" reports the new default.
    assert!(evs.contains(&Event::LayoutChanged {
        workspace: "2".into(),
        layout: uq_core::lua::Layout::Master
    }));
    assert!(!evs.iter().any(|e| matches!(e, Event::ThemeChanged { .. })));
}

#[tokio::test]
async fn set_layout_emits_layouts_section() {
    let mut r = rig();
    r.desktop
        .set_layout("2", "scrolling", Origin::Settings)
        .await
        .unwrap();
    let evs = drain(&mut r.events);
    let json = evs
        .iter()
        .find_map(|e| match e {
            Event::SectionChanged { domain, json } if domain == "layouts" => Some(json),
            _ => None,
        })
        .expect("SectionChanged layouts");
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(v["workspaces"]["2"], "scrolling");
}

#[tokio::test]
async fn set_input_renders_and_reloads() {
    let mut r = rig();
    r.desktop.startup().await;
    r.comp.calls.lock().unwrap().clear();
    drain(&mut r.events);
    r.desktop
        .set_section(
            "input",
            r#"{"keyboard":{"layouts":[{"layout":"us"},{"layout":"de"}]}}"#,
            Origin::Settings,
        )
        .await
        .unwrap();
    assert!(generated(&r).contains(r#"kb_layout = "us,de""#));
    assert!(r.comp.calls().contains(&"reload".to_string()));
    let evs = drain(&mut r.events);
    assert!(
        evs.iter()
            .any(|e| matches!(e, Event::SectionChanged { domain, .. } if domain == "input"))
    );
    let saved = State::load(&r.desktop.paths().state_file);
    assert_eq!(saved.settings.input.keyboard.layouts.len(), 2);
}

#[tokio::test]
async fn rejected_input_keeps_previous_config() {
    let mut r = rig();
    r.desktop.startup().await;
    let gen_before = std::fs::read(&r.desktop.paths().generated_lua).unwrap();
    let state_before = std::fs::read(&r.desktop.paths().state_file).unwrap();
    drain(&mut r.events);
    *r.comp.verify_error.lock().unwrap() = Some("bad option".into());
    let res = r
        .desktop
        .set_section(
            "input",
            r#"{"keyboard":{"layouts":[{"layout":"de"}]}}"#,
            Origin::Settings,
        )
        .await;
    assert_eq!(res, Err(DesktopError::ConfigInvalid("bad option".into())));
    assert_eq!(
        std::fs::read(&r.desktop.paths().generated_lua).unwrap(),
        gen_before
    );
    assert_eq!(
        std::fs::read(&r.desktop.paths().state_file).unwrap(),
        state_before
    );
    let evs = drain(&mut r.events);
    assert!(
        evs.iter().all(|e| matches!(e, Event::ConfigError(_))),
        "{evs:?}"
    );
    let input: serde_json::Value =
        serde_json::from_str(&r.desktop.get_section("input").unwrap()).unwrap();
    assert_eq!(input["keyboard"]["layouts"], serde_json::json!([]));
}

#[tokio::test]
async fn monitors_section_write_refused() {
    let mut r = rig();
    r.desktop.startup().await;
    let state_before = std::fs::read(&r.desktop.paths().state_file).unwrap();
    drain(&mut r.events);
    let res = r
        .desktop
        .set_section(
            "monitors",
            r#"{"outputs":{"DP-1":{"scale":1.5}}}"#,
            Origin::Settings,
        )
        .await;
    assert_eq!(
        res,
        Err(DesktopError::InvalidSetting(
            "displays change through ApplyMonitors".into()
        ))
    );
    assert_eq!(
        std::fs::read(&r.desktop.paths().state_file).unwrap(),
        state_before
    );
    assert!(drain(&mut r.events).is_empty());
    assert!(r.desktop.history(100).is_empty());
}

#[tokio::test]
async fn input_devices_from_hyprland() {
    let r = rig();
    let long = "x".repeat(300);
    *r.comp.devices_json.lock().unwrap() = format!(
        r#"{{"mice":[{{"address":"0x1","name":"logitech-mx","defaultSpeed":0.5}},
                    {{"address":"0x2","name":"{long}","defaultSpeed":0.0}}],
            "keyboards":[{{"address":"0x3","name":"at-keyboard","main":true}}],
            "touch":[],"tablets":[{{"name":"ignored"}}]}}"#
    );
    let v: serde_json::Value =
        serde_json::from_str(&r.desktop.list_input_devices().await.unwrap()).unwrap();
    assert_eq!(v["keyboards"], serde_json::json!([{"name": "at-keyboard"}]));
    assert_eq!(v["touch"], serde_json::json!([]));
    let p = v["pointers"].as_array().unwrap();
    assert_eq!(p.len(), 2);
    assert_eq!(p[0]["name"], "logitech-mx");
    assert_eq!(p[0]["default_speed"], 0.5);
    assert_eq!(p[1]["name"].as_str().unwrap().chars().count(), 128);
}

#[tokio::test]
async fn input_devices_empty_when_hyprland_is_down() {
    let r = rig();
    *r.comp.running.lock().unwrap() = false;
    assert_eq!(
        r.desktop.list_input_devices().await.unwrap(),
        r#"{"keyboards":[],"pointers":[],"touch":[]}"#
    );
}
