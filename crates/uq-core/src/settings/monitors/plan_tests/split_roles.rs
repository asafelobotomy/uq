//! Workspace splits, screen order and roles.

use super::*;

#[test]
fn sequential_split() {
    let two = plan_of(Strategy::Sequential, 10, 2);
    assert_eq!(share(&two, "DP-1"), [1, 2, 3, 4, 5]);
    assert_eq!(share(&two, "DP-2"), [6, 7, 8, 9, 10]);
    let three = plan_of(Strategy::Sequential, 10, 3);
    assert_eq!(share(&three, "DP-1"), [1, 2, 3, 4]);
    assert_eq!(share(&three, "DP-2"), [5, 6, 7]);
    assert_eq!(share(&three, "DP-3"), [8, 9, 10]);
    let odd = plan_of(Strategy::Sequential, 3, 2);
    assert_eq!(share(&odd, "DP-1"), [1, 2]);
    assert_eq!(share(&odd, "DP-2"), [3]);
    let one = plan_of(Strategy::Sequential, 1, 2);
    assert_eq!(one, [(1, "DP-1".to_string(), true)]);
}

#[test]
fn interleave_split() {
    let two = plan_of(Strategy::Interleave, 6, 2);
    assert_eq!(share(&two, "DP-1"), [1, 3, 5]);
    assert_eq!(share(&two, "DP-2"), [2, 4, 6]);
    let three = plan_of(Strategy::Interleave, 7, 3);
    assert_eq!(share(&three, "DP-1"), [1, 4, 7]);
    assert_eq!(share(&three, "DP-3"), [3, 6]);
}

#[test]
fn defaults_are_first_of_each_share() {
    for s in [Strategy::Sequential, Strategy::Interleave] {
        let p = plan_of(s, 10, 2);
        let defaults: Vec<u8> = p.iter().filter(|x| x.2).map(|x| x.0).collect();
        let expect = if s == Strategy::Sequential {
            vec![1, 6]
        } else {
            vec![1, 2]
        };
        assert_eq!(defaults, expect, "{s:?}");
    }
    let manual = WorkspacePlan {
        strategy: Strategy::Manual,
        count: 4,
        manual: vec![ManualSlot {
            workspace: 1,
            monitor: mid("0"),
        }],
        ..WorkspacePlan::default()
    };
    assert!(plan(&manual, &screens_n(2)).iter().all(|p| !p.default));
}

#[test]
fn manual_skips_absent_screens() {
    let p = WorkspacePlan {
        strategy: Strategy::Manual,
        count: 5,
        manual: vec![
            ManualSlot {
                workspace: 4,
                monitor: mid("1"),
            },
            ManualSlot {
                workspace: 2,
                monitor: mid("0"),
            },
            ManualSlot {
                workspace: 3,
                monitor: mid("gone"),
            },
        ],
        move_back: true,
    };
    let got: Vec<(u8, String)> = plan(&p, &screens_n(2))
        .into_iter()
        .map(|p| (p.workspace, p.output))
        .collect();
    assert_eq!(got, [(2, "DP-1".into()), (4, "DP-2".into())]);
}

#[test]
fn one_screen_plans_nothing() {
    for s in [Strategy::Sequential, Strategy::Interleave, Strategy::Manual] {
        assert!(plan_of(s, 10, 1).is_empty(), "{s:?}");
        assert!(plan_of(s, 10, 0).is_empty(), "{s:?}");
    }
    assert!(plan_of(Strategy::Off, 10, 3).is_empty());
}

#[test]
fn screens_order_left_to_right_then_top() {
    let mut off = out("DP-4", "off", 0, 0);
    off.disabled = true;
    let mut mirror = out("DP-5", "mirror", 0, 0);
    mirror.mirror_of = Some("DP-1".into());
    let live = [
        out("DP-3", "c", 1920, 0),
        out("DP-2", "b", 0, 1080),
        out("DP-1", "a", 0, 0),
        out("HEADLESS-1", "", 0, 0),
        off,
        mirror,
    ];
    let s = screens(&Monitors::default(), &live);
    let got: Vec<&str> = s.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(got, ["DP-1", "DP-2", "DP-3"]);
    // The stored layout is laid over the live one first.
    let mut m = Monitors::default();
    m.outputs.insert(
        "DP-3".into(),
        OutputConfig {
            position: Some(Position { x: -1920, y: 0 }),
            ..OutputConfig::default()
        },
    );
    m.outputs.insert(
        "DP-2".into(),
        OutputConfig {
            enabled: Some(false),
            ..OutputConfig::default()
        },
    );
    let got: Vec<String> = screens(&m, &live).into_iter().map(|o| o.name).collect();
    assert_eq!(got, ["DP-3", "DP-1"]);
}

#[test]
fn main_output_prefers_primary_then_fallback() {
    let live = [out("DP-1", "A", 0, 0), out("DP-2", "B", 2560, 0)];
    let mut m = Monitors::default();
    assert_eq!(main_output(&m, &live).unwrap().name, "DP-1");
    m.primary = Some(mid("B"));
    assert_eq!(main_output(&m, &live).unwrap().name, "DP-2");
    // Primary disabled: fallback.
    let mut off = live.clone();
    off[1].disabled = true;
    assert_eq!(main_output(&m, &off).unwrap().name, "DP-1");
    // Primary mirroring: fallback.
    let mut mir = live.clone();
    mir[1].mirror_of = Some("DP-1".into());
    assert_eq!(main_output(&m, &mir).unwrap().name, "DP-1");
    // Primary not connected: fallback.
    m.primary = Some(mid("gone"));
    assert_eq!(main_output(&m, &live).unwrap().name, "DP-1");
    assert!(main_output(&m, &[]).is_none());
}

#[test]
fn roles_primary_empty_unless_chosen() {
    let live = [out("DP-1", "A", 0, 0), out("DP-2", "B", 2560, 0)];
    let mut m = Monitors::default();
    let r = roles(&m, &live);
    assert_eq!((r.primary.as_str(), r.main.as_str()), ("", "DP-1"));
    assert_eq!(r.bar, None);
    m.primary = Some(mid("B"));
    let r = roles(&m, &live);
    assert_eq!((r.primary.as_str(), r.main.as_str()), ("DP-2", "DP-2"));
    m.primary = Some(mid("gone"));
    let r = roles(&m, &live);
    assert_eq!((r.primary.as_str(), r.main.as_str()), ("", "DP-1"));
}

#[test]
fn roles_bar_never_empty() {
    let live = [out("DP-1", "A", 0, 0), out("DP-2", "B", 2560, 0)];
    let mut m = Monitors::default();
    m.bar = BarChoice {
        show: BarShow::Main,
        on: vec![],
    };
    assert_eq!(roles(&m, &live).bar, Some(vec!["DP-1".to_string()]));
    assert_eq!(roles(&m, &[]).bar, None, "main with no screen");
    m.bar = BarChoice {
        show: BarShow::Chosen,
        on: vec![mid("B")],
    };
    assert_eq!(roles(&m, &live).bar, Some(vec!["DP-2".to_string()]));
    m.bar.on = vec![mid("gone")];
    assert_eq!(roles(&m, &live).bar, None, "chosen all absent");
    m.bar.on = vec![];
    assert_eq!(roles(&m, &live).bar, None);
    m.bar.on = vec![mid("A"), mid("B")];
    assert_eq!(
        roles(&m, &live).bar,
        Some(vec!["DP-1".to_string(), "DP-2".to_string()])
    );
}

#[test]
fn main_scale_unchanged_without_primary() {
    let mut a = out("DP-1", "A", 0, 0);
    a.scale = 1.5;
    let mut b = out("DP-2", "B", 2560, 0);
    b.scale = 2.0;
    assert_eq!(main_scale(&[a.clone(), b.clone()]), Some(1.5));
    let mut moved = a.clone();
    moved.x = 100;
    let mut at_origin = b.clone();
    at_origin.x = 0;
    assert_eq!(main_scale(&[moved.clone(), at_origin]), Some(2.0));
    assert_eq!(
        main_scale(&[moved, b.clone()]),
        Some(1.5),
        "none at 0,0: first by name"
    );
    assert_eq!(main_scale(&[]), None);
    assert_eq!(fallback_main(&[a, b]).unwrap().name, "DP-1");
}

#[test]
fn screens_follow_stored_mirror() {
    let mut mirrored_live = out("DP-2", "B", 2560, 0);
    mirrored_live.mirror_of = Some("DP-1".into());
    let live = [out("DP-1", "A", 0, 0), out("DP-2", "B", 2560, 0)];
    // A candidate that turns mirroring on: the mirror leaves the plan, the cursor and the bar.
    let mut m = Monitors {
        primary: Some(mid("B")),
        workspaces: WorkspacePlan {
            strategy: Strategy::Sequential,
            count: 4,
            ..WorkspacePlan::default()
        },
        ..Monitors::default()
    };
    m.outputs.insert(
        "DP-2".into(),
        OutputConfig {
            mirror_of: Some("DP-1".into()),
            ..OutputConfig::default()
        },
    );
    let lay = overlay(&live, &m);
    let shown = screens(&m, &live);
    let names: Vec<&str> = shown.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, ["DP-1"]);
    assert!(
        plan(&m.workspaces, &screens(&m, &live)).is_empty(),
        "one screen plans nothing"
    );
    let r = roles(&m, &lay);
    assert_eq!((r.primary.as_str(), r.main.as_str()), ("", "DP-1"));
    let render = crate::lua::monitor_render(&m, &lay);
    assert_eq!(render.default_monitor, None);
    // Three screens: the mirror gets no workspace.
    let three = [live[0].clone(), live[1].clone(), out("DP-3", "C", 5120, 0)];
    let got = plan(&m.workspaces, &screens(&m, &three));
    assert!(got.iter().all(|p| p.output != "DP-2"), "{got:?}");
    // A stored entry without a mirror un-mirrors a live mirror.
    let mut plain = Monitors::default();
    plain.outputs.insert("DP-2".into(), OutputConfig::default());
    let names: Vec<String> = screens(&plain, &[live[0].clone(), mirrored_live])
        .into_iter()
        .map(|o| o.name)
        .collect();
    assert_eq!(names, ["DP-1", "DP-2"]);
}

#[test]
fn roles_chosen_bar_skips_mirrors_and_headless() {
    let mut mir = out("DP-2", "B", 2560, 0);
    mir.mirror_of = Some("DP-1".into());
    let live = [out("DP-1", "A", 0, 0), mir, out("HEADLESS-1", "A", 0, 0)];
    let m = Monitors {
        bar: BarChoice {
            show: BarShow::Chosen,
            on: vec![mid("A"), mid("B")],
        },
        ..Monitors::default()
    };
    assert_eq!(roles(&m, &live).bar, Some(vec!["DP-1".to_string()]));
}

#[test]
fn invalid_connector_names_are_never_planned() {
    let live = [
        out("DP-1", "A", 0, 0),
        out("DP 2", "B", 2560, 0),
        out("DP-3", "C", 5120, 0),
    ];
    let names: Vec<String> = screens(&Monitors::default(), &live)
        .into_iter()
        .map(|o| o.name)
        .collect();
    assert_eq!(names, ["DP-1", "DP-3"]);
}
