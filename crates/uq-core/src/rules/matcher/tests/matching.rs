//! Patterns, matching and window lists.

use super::*;

#[test]
fn full_match_semantics() {
    let w = win("firefox-esr");
    assert_eq!(
        m(|x| x.class = Some("firefox".into())).test(&w),
        Verdict::No
    );
    assert_eq!(
        m(|x| x.class = Some("fire.*".into())).test(&w),
        Verdict::Yes
    );
    assert_eq!(
        m(|x| x.class = Some("^firefox-esr$".into())).test(&w),
        Verdict::Yes
    );
    assert_eq!(m(|x| x.class = Some("^fire$".into())).test(&w), Verdict::No);
}

#[test]
fn compiled_refuses_bad_patterns() {
    let x = WindowMatch {
        title: Some("(".into()),
        ..Default::default()
    };
    assert!(Compiled::new(&x).is_err());
}

#[test]
fn every_match_field() {
    let mut w = win("a");
    w.title = "Hello".into();
    w.initial_class = "ic".into();
    w.initial_title = "it".into();
    w.tags = vec!["music".into()];
    w.xwayland = true;
    w.floating = true;
    w.fullscreen = true;
    w.pinned = true;
    let yes = |c: Compiled| assert_eq!(c.test(&w), Verdict::Yes);
    let no = |c: Compiled| assert_eq!(c.test(&w), Verdict::No);
    yes(m(|x| x.title = Some("Hel+o".into())));
    no(m(|x| x.title = Some("Bye".into())));
    yes(m(|x| x.initial_class = Some("ic".into())));
    no(m(|x| x.initial_class = Some("a".into())));
    yes(m(|x| x.initial_title = Some("it".into())));
    no(m(|x| x.initial_title = Some("T".into())));
    yes(m(|x| x.tag = Some("music".into())));
    no(m(|x| x.tag = Some("mus".into())));
    yes(m(|x| x.xwayland = Some(true)));
    no(m(|x| x.xwayland = Some(false)));
    yes(m(|x| x.floating = Some(true)));
    no(m(|x| x.floating = Some(false)));
    yes(m(|x| x.fullscreen = Some(true)));
    no(m(|x| x.fullscreen = Some(false)));
    yes(m(|x| x.pinned = Some(true)));
    no(m(|x| x.pinned = Some(false)));
    // all fields must hold
    no(m(|x| {
        x.class = Some("a".into());
        x.title = Some("Bye".into());
    }));
}

#[test]
fn workspace_match_forms() {
    let mut w = win("a");
    w.workspace_id = 3;
    w.workspace_name = "3".into();
    let ws = |s: &str, w: &WindowInfo| m(|x| x.workspace = Some(s.into())).test(w);
    assert_eq!(ws("3", &w), Verdict::Yes);
    assert_eq!(ws("4", &w), Verdict::No);
    let mut web = win("a");
    web.workspace_id = 5;
    web.workspace_name = "web".into();
    let mut webby = web.clone();
    webby.workspace_name = "webby".into();
    assert_eq!(ws("name:web", &web), Verdict::Yes);
    assert_eq!(ws("name:web", &webby), Verdict::No);
    let mut sp = win("a");
    sp.workspace_id = -98;
    sp.workspace_name = "special:magic".into();
    assert_eq!(ws("special:magic", &sp), Verdict::Yes);
    assert_eq!(ws("special:other", &sp), Verdict::No);
    assert_eq!(ws("w[tv1]", &w), Verdict::Unknown);
    let mut def = win("a");
    def.workspace_id = -99;
    def.workspace_name = "special:special".into();
    assert_eq!(ws("special", &def), Verdict::Yes);
    assert_eq!(ws("special", &sp), Verdict::No);
    assert_eq!(ws("special:special", &def), Verdict::Yes);
    assert_eq!(ws("special", &w), Verdict::No);
}

#[test]
fn modal_gives_if_dialog() {
    let w = win("a");
    let c = m(|x| {
        x.class = Some("a".into());
        x.modal = Some(true);
    });
    assert_eq!(c.test(&w), Verdict::IfDialog);
    let c = m(|x| {
        x.class = Some("b".into());
        x.modal = Some(true);
    });
    assert_eq!(c.test(&w), Verdict::No);
    // Unknown wins over IfDialog; No wins over both.
    let c = m(|x| {
        x.workspace = Some("w[1]".into());
        x.modal = Some(true);
    });
    assert_eq!(c.test(&w), Verdict::Unknown);
    let c = m(|x| {
        x.workspace = Some("w[1]".into());
        x.modal = Some(true);
        x.class = Some("b".into());
    });
    assert_eq!(c.test(&w), Verdict::No);
}

#[test]
fn matching_keeps_order_and_disabled() {
    let rule = |name: &str, class: &str, enabled: bool| WindowRule {
        id: String::new(),
        name: name.into(),
        enabled,
        matches: WindowMatch {
            class: Some(class.into()),
            ..Default::default()
        },
        effects: WindowEffects {
            float: Some(true),
            ..Default::default()
        },
    };
    let rules = vec![
        rule("a", "x", true),
        rule("b", "x", false),
        rule("c", "y", true),
        rule("d", "x.*", true),
    ];
    let got = matching(&rules, &win("x"));
    assert_eq!(
        got,
        vec![(0, Verdict::Yes), (1, Verdict::Yes), (3, Verdict::Yes)]
    );
}

#[test]
fn verdict_serialises_snake_case() {
    assert_eq!(
        serde_json::to_string(&Verdict::IfDialog).unwrap(),
        "\"if_dialog\""
    );
    assert_eq!(
        serde_json::to_string(&Verdict::Unknown).unwrap(),
        "\"unknown\""
    );
    assert_eq!(serde_json::to_string(&Verdict::Yes).unwrap(), "\"yes\"");
    assert_eq!(serde_json::to_string(&Verdict::No).unwrap(), "\"no\"");
}

#[test]
fn windows_from_clients_maps_and_cleans() {
    let j = json!([
        {"address": "0x1", "mapped": true, "hidden": false, "at": [0, 0], "size": [10, 10],
         "workspace": {"id": 5, "name": "web"}, "floating": true, "monitor": 0,
         "class": "firefox", "title": "a\u{1b}[2J", "initialClass": "Firefox",
         "initialTitle": "init", "xwayland": true, "pinned": false, "fullscreen": 2,
         "tags": ["x", "y\u{7}"], "focusHistoryID": 0, "pid": 12},
        {"mapped": false, "class": "ghost", "workspace": {"id": 1, "name": "1"}},
        {"mapped": true, "class": "plain", "title": "", "fullscreen": 0,
         "workspace": {"id": 1, "name": "1"}}
    ])
    .to_string();
    let got = windows_from_clients(&j);
    assert_eq!(got.len(), 2);
    let w = &got[0];
    assert_eq!(w.class, "firefox");
    assert_eq!(w.title, "a [2J");
    assert_eq!(w.initial_class, "Firefox");
    assert_eq!(w.initial_title, "init");
    assert_eq!((w.workspace_id, w.workspace_name.as_str()), (5, "web"));
    assert!(w.fullscreen && w.xwayland && w.floating && !w.pinned);
    assert_eq!(w.tags, vec!["x".to_string(), "y ".to_string()]);
    assert!(!got[1].fullscreen);
    assert!(windows_from_clients("not json").is_empty());
    assert!(windows_from_clients("{}").is_empty());
}

#[test]
fn windows_from_clients_caps_the_list() {
    let items: Vec<_> = (0..600)
        .map(|i| json!({"mapped": true, "class": format!("c{i}")}))
        .collect();
    assert_eq!(windows_from_clients(&json!(items).to_string()).len(), 500);
}

#[test]
fn effects_text_is_plain() {
    let e = WindowEffects {
        float: Some(true),
        tile: Some(false),
        fullscreen: Some(true),
        maximize: Some(true),
        center: Some(true),
        pin: Some(true),
        no_initial_focus: Some(true),
        no_blur: Some(true),
        no_shadow: Some(true),
        no_anim: Some(true),
        keep_aspect_ratio: Some(true),
        workspace: Some("3".into()),
        silent: Some(true),
        monitor: Some("DP-1".into()),
        size: Some([800, 600]),
        position: Some([10, -20]),
        opacity: Some(0.9),
        rounding: Some(8),
        border_size: Some(2),
        idle_inhibit: Some("fullscreen".into()),
        tag: Some("music".into()),
    };
    let lines = effects_text(&e);
    assert!(lines.contains(&"Floats".to_string()));
    assert!(lines.contains(&"Opens on workspace 3, without switching to it".to_string()));
    assert!(lines.contains(&"Opacity: 90 %".to_string()));
    assert!(lines.contains(&"Keeps the screen awake while fullscreen".to_string()));
    let others = [
        layer_effects_text(&LayerEffects {
            blur: Some(true),
            no_anim: Some(true),
            ignore_alpha: Some(0.3),
            animation: Some("fade".into()),
        }),
        workspace_text(&WorkspaceRule {
            workspace: "name:web".into(),
            monitor: Some("DP-1".into()),
            default: Some(true),
            persistent: Some(true),
            gaps_in: Some(1),
            gaps_out: Some(2),
            border_size: Some(3),
        }),
        match_text(&WindowMatch {
            class: Some("^a$".into()),
            title: Some("t".into()),
            initial_class: Some("a".into()),
            initial_title: Some("t".into()),
            tag: Some("x".into()),
            workspace: Some("name:web".into()),
            xwayland: Some(true),
            floating: Some(true),
            fullscreen: Some(true),
            pinned: Some(true),
            modal: Some(true),
        }),
    ];
    assert_eq!(
        effects_text(&WindowEffects::default()),
        Vec::<String>::new()
    );
    let mut bad: Vec<&str> = vec!["hl.", "=", "{", "\"", "_"];
    let lua: Vec<&str> = hypr::WINDOW_EFFECTS.iter().map(|e| e.field.lua).collect();
    bad.extend(lua.iter().filter(|l| l.contains('_')));
    for line in lines.iter().chain(others.iter().flatten()) {
        assert!(!line.is_empty());
        for b in &bad {
            assert!(!line.contains(b), "{line:?} holds {b:?}");
        }
    }
    assert!(others[2].contains(&"Is on workspace web".to_string()));
    assert!(others[0].contains(&"Animation style: fade".to_string()));
    assert!(has_static(&e));
    assert!(!has_static(&WindowEffects {
        opacity: Some(0.5),
        no_blur: Some(true),
        ..Default::default()
    }));
}

#[test]
fn tag_match_accepts_effect_tags() {
    let mut w = win("a");
    w.tags = vec!["music*".into()];
    assert_eq!(m(|x| x.tag = Some("music".into())).test(&w), Verdict::Yes);
    assert_eq!(m(|x| x.tag = Some("mus".into())).test(&w), Verdict::No);
}
