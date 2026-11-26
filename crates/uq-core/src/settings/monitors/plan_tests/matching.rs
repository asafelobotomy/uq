//! Matching a set of screens and arranging it.

use super::*;

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
