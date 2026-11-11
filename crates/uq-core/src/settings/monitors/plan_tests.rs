#![allow(clippy::field_reassign_with_default)]

use super::*;
use crate::compat::launch::main_scale;
use crate::settings::{
    BarChoice, BarShow, ManualSlot, MonitorId, MonitorProfile, Monitors, OutputConfig, Position,
    SavedOutput, Strategy, WorkspacePlan,
};
use std::collections::BTreeSet;

fn out(name: &str, serial: &str, x: i32, y: i32) -> LiveOutput {
    LiveOutput {
        name: name.into(),
        description: format!("Dell Inc. DELL U2720Q {serial}"),
        make: "Dell Inc.".into(),
        model: "DELL U2720Q".into(),
        serial: serial.into(),
        width: 2560,
        height: 1440,
        refresh_rate: 143.912,
        x,
        y,
        scale: 1.0,
        ..LiveOutput::default()
    }
}

fn mid(serial: &str) -> MonitorId {
    MonitorId {
        make: "Dell Inc.".into(),
        model: "DELL U2720Q".into(),
        serial: serial.into(),
    }
}

fn saved(serial: &str, hint: &str, x: i32) -> SavedOutput {
    SavedOutput {
        monitor: mid(serial),
        hint: hint.into(),
        config: OutputConfig {
            position: Some(Position { x, y: 0 }),
            scale: Some(1.5),
            ..OutputConfig::default()
        },
    }
}

fn profile(id: &str, name: &str, outputs: Vec<SavedOutput>) -> MonitorProfile {
    MonitorProfile {
        id: id.into(),
        name: name.into(),
        outputs,
        workspaces: None,
    }
}

fn with_profiles(p: Vec<MonitorProfile>) -> Monitors {
    Monitors {
        profiles: p,
        ..Monitors::default()
    }
}

fn names(r: &[(String, OutputConfig)]) -> Vec<&str> {
    r.iter().map(|(n, _)| n.as_str()).collect()
}

#[test]
fn match_survives_port_swap() {
    let m = with_profiles(vec![profile(
        "m-0001",
        "Desk",
        vec![saved("A", "DP-1", 0), saved("B", "DP-2", 2560)],
    )]);
    let live = [out("DP-3", "A", 0, 0), out("HDMI-A-1", "B", 2560, 0)];
    let p = find_arrangement(&m, &live).expect("found");
    assert_eq!(p.name, "Desk");
    let r = resolve(&p.outputs, &live).unwrap();
    assert_eq!(names(&r), ["DP-3", "HDMI-A-1"]);
    assert_eq!(r[1].1.position, Some(Position { x: 2560, y: 0 }));
}

#[test]
fn twins_resolve_to_distinct_outputs() {
    // Two identical screens with the same serial: still two entries, two connectors.
    let two = vec![saved("", "DP-1", 0), saved("", "DP-2", 1920)];
    let live = [out("DP-5", "", 0, 0), out("DP-6", "", 0, 0)];
    let r = resolve(&two, &live).unwrap();
    assert_eq!(names(&r), ["DP-5", "DP-6"]);
    // Three saved, two live: no match.
    let three = vec![saved("", "", 0), saved("", "", 1), saved("", "", 2)];
    assert!(resolve(&three, &live).is_none());
}

#[test]
fn twins_prefer_their_old_port() {
    let two = vec![saved("S", "DP-1", 0), saved("S", "DP-2", 1920)];
    let live = [out("DP-2", "S", 0, 0), out("DP-1", "S", 0, 0)];
    let r = resolve(&two, &live).unwrap();
    assert_eq!(names(&r), ["DP-1", "DP-2"], "each keeps its own port");
    // One port gone: the second twin takes what is left.
    let live = [out("DP-1", "S", 0, 0), out("DP-7", "S", 0, 0)];
    let r = resolve(&two, &live).unwrap();
    assert_eq!(names(&r), ["DP-1", "DP-7"]);
}

#[test]
fn subset_or_superset_is_no_match() {
    let m = with_profiles(vec![profile(
        "m-0001",
        "Desk",
        vec![saved("A", "DP-1", 0), saved("B", "DP-2", 2560)],
    )]);
    assert!(find_arrangement(&m, &[out("DP-1", "A", 0, 0)]).is_none());
    let three = [
        out("DP-1", "A", 0, 0),
        out("DP-2", "B", 0, 0),
        out("DP-3", "C", 0, 0),
    ];
    assert!(find_arrangement(&m, &three).is_none());
    assert!(find_arrangement(&m, &[]).is_none());
}

#[test]
fn disabled_screens_still_count() {
    let m = with_profiles(vec![profile(
        "m-0001",
        "Desk",
        vec![saved("A", "DP-1", 0), saved("B", "DP-2", 2560)],
    )]);
    let mut b = out("DP-2", "B", 2560, 0);
    b.disabled = true;
    assert!(find_arrangement(&m, &[out("DP-1", "A", 0, 0), b.clone()]).is_some());
    assert_eq!(connected_set(&[b]).len(), 1);
}

#[test]
fn fallback_and_headless_never_count() {
    let live = [
        out("DP-1", "A", 0, 0),
        out("FALLBACK", "", 0, 0),
        out("HEADLESS-2", "", 0, 0),
    ];
    assert_eq!(connected_set(&live), vec![mid("A")]);
    let m = with_profiles(vec![profile("m-0001", "Solo", vec![saved("A", "DP-1", 0)])]);
    assert!(find_arrangement(&m, &live).is_some());
}

#[test]
fn arranged_keeps_other_connectors_and_takes_plan() {
    let mut m = with_profiles(vec![]);
    m.outputs.insert(
        "DP-9".into(),
        OutputConfig {
            scale: Some(2.0),
            ..OutputConfig::default()
        },
    );
    m.outputs.insert(
        "DP-3".into(),
        OutputConfig {
            scale: Some(1.0),
            ..OutputConfig::default()
        },
    );
    let mut p = profile(
        "m-0001",
        "Desk",
        vec![saved("A", "DP-1", 0), saved("B", "DP-2", 2560)],
    );
    p.workspaces = Some(WorkspacePlan {
        strategy: Strategy::Interleave,
        ..WorkspacePlan::default()
    });
    let live = [out("DP-3", "A", 0, 0), out("DP-4", "B", 0, 0)];
    let got = arranged(&m, &p, &live).unwrap();
    assert_eq!(
        got.outputs["DP-9"].scale,
        Some(2.0),
        "absent connector kept"
    );
    assert_eq!(
        got.outputs["DP-3"].scale,
        Some(1.5),
        "plan replaces the entry"
    );
    assert_eq!(
        got.outputs["DP-4"].position,
        Some(Position { x: 2560, y: 0 })
    );
    assert_eq!(
        got.outputs["DP-4"].description.as_deref(),
        Some("Dell Inc. DELL U2720Q B"),
        "refreshed from live"
    );
    assert_eq!(got.workspaces.strategy, Strategy::Interleave);
    // No plan of its own: the stored planner stays.
    p.workspaces = None;
    m.workspaces.strategy = Strategy::Sequential;
    assert_eq!(
        arranged(&m, &p, &live).unwrap().workspaces.strategy,
        Strategy::Sequential
    );
    // A screen that can't be placed gives nothing.
    assert!(arranged(&m, &p, &[out("DP-3", "A", 0, 0)]).is_none());
}

#[test]
fn arranged_follows_mirror_targets_to_new_ports() {
    let mut second = saved("B", "DP-2", 0);
    second.config.mirror_of = Some("DP-1".into());
    let p = profile("m-0001", "Mirror", vec![saved("A", "DP-1", 0), second]);
    let live = [out("DP-7", "A", 0, 0), out("DP-8", "B", 0, 0)];
    let got = arranged(&Monitors::default(), &p, &live).unwrap();
    assert_eq!(got.outputs["DP-8"].mirror_of.as_deref(), Some("DP-7"));
}

#[test]
fn remember_uses_stored_then_live() {
    let mut m = Monitors::default();
    m.outputs.insert(
        "DP-1".into(),
        OutputConfig {
            mode: Some("1920x1080@60".into()),
            scale: Some(2.0),
            ..OutputConfig::default()
        },
    );
    let live = [out("DP-1", "A", 0, 0), out("DP-2", "B", 2560, 0)];
    let p = remember(&m, &live, "  Desk ", None, &BTreeSet::new()).unwrap();
    assert_eq!(p.name, "Desk");
    assert!(valid_profile_id(&p.id));
    assert_eq!(p.outputs.len(), 2);
    assert_eq!(p.outputs[0].hint, "DP-1");
    assert_eq!(p.outputs[0].config.mode.as_deref(), Some("1920x1080@60"));
    assert_eq!(p.outputs[0].config.scale, Some(2.0));
    let live_cfg = &p.outputs[1].config;
    assert_eq!(live_cfg.mode.as_deref(), Some("2560x1440@143.91"));
    assert_eq!(live_cfg.position, Some(Position { x: 2560, y: 0 }));
    assert_eq!(live_cfg.scale, Some(1.0));
    // A screen with no stored entry and no live mode can't be remembered.
    let mut blank = out("DP-3", "C", 0, 0);
    blank.width = 0;
    blank.height = 0;
    let err = remember(&m, &[blank], "x", None, &BTreeSet::new()).unwrap_err();
    assert!(
        err.to_string().contains("arrange these screens first"),
        "{err}"
    );
    // An empty name is refused for a new arrangement.
    assert!(remember(&m, &live, "  ", None, &BTreeSet::new()).is_err());
    // Past 32 arrangements.
    let many = Monitors {
        profiles: (0..32)
            .map(|i| {
                profile(
                    &format!("m-{i:04x}"),
                    "n",
                    vec![saved(&i.to_string(), "", 0)],
                )
            })
            .collect(),
        ..Monitors::default()
    };
    assert!(remember(&many, &live, "x", None, &BTreeSet::new()).is_err());
}

#[test]
fn remember_same_set_keeps_id_and_name() {
    let live = [out("DP-1", "A", 0, 0), out("DP-2", "B", 2560, 0)];
    let existing = profile(
        "m-7f2c",
        "Desk",
        vec![saved("B", "DP-9", 0), saved("A", "DP-8", 0)],
    );
    let m = with_profiles(vec![existing]);
    let taken: BTreeSet<String> = ["m-7f2c".to_string()].into();
    let p = remember(&m, &live, "", Some("m-1111".into()), &taken).unwrap();
    assert_eq!((p.id.as_str(), p.name.as_str()), ("m-7f2c", "Desk"));
    assert_eq!(p.outputs.len(), 2);
    assert_eq!(p.outputs[0].hint, "DP-1", "hints are refreshed");
    let renamed = remember(&m, &live, "Home", None, &taken).unwrap();
    assert_eq!(
        (renamed.id.as_str(), renamed.name.as_str()),
        ("m-7f2c", "Home")
    );
}

fn screens_n(n: usize) -> Vec<LiveOutput> {
    (0..n)
        .map(|i| out(&format!("DP-{}", i + 1), &i.to_string(), i as i32 * 2560, 0))
        .collect()
}

fn plan_of(strategy: Strategy, count: u8, n: usize) -> Vec<(u8, String, bool)> {
    let p = WorkspacePlan {
        strategy,
        count,
        ..WorkspacePlan::default()
    };
    plan(&p, &screens_n(n))
        .into_iter()
        .map(|p| (p.workspace, p.output, p.default))
        .collect()
}

fn share(planned: &[(u8, String, bool)], output: &str) -> Vec<u8> {
    planned
        .iter()
        .filter(|p| p.1 == output)
        .map(|p| p.0)
        .collect()
}

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
