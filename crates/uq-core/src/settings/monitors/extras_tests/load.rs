//! Loading and repairing the extras.

use super::*;

#[test]
fn defaults_serialise_to_nothing() {
    let d = Monitors::default();
    assert_eq!(serde_json::to_value(&d).unwrap(), json!({"outputs": {}}));
    assert!(d.auto_profiles);
    assert_eq!(d.workspaces.count, 10);
    assert!(d.workspaces.move_back);
    assert_eq!(d.workspaces.strategy, Strategy::Off);
    assert_eq!(d.bar.show, BarShow::All);
}

#[test]
fn section_round_trips() {
    let m = section(full()).unwrap();
    assert_eq!(m.primary.as_ref().unwrap().serial, "ABC123");
    assert!(!m.auto_profiles);
    assert_eq!(m.bar.show, BarShow::Chosen);
    assert_eq!(m.workspaces.strategy, Strategy::Manual);
    assert_eq!(m.profiles[0].id, "m-7f2c");
    let json = serde_json::to_value(&m).unwrap();
    assert_eq!(section(json.clone()).unwrap(), m);
    assert_eq!(json["workspaces"]["strategy"], "manual");
    let mut s = State::default();
    s.settings.monitors = m.clone();
    let back: State = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.settings.monitors, m);
}

#[test]
fn strict_refusals() {
    let with = |k: &str, v: Value| {
        let mut f = full();
        f[k] = v;
        f
    };
    let arr = |a: Value| with("profiles", json!([a]));
    refused(with("primary", json!("DP-1")), "monitors.primary");
    refused(
        with(
            "primary",
            json!({"make": "a", "model": "b", "serial": "c", "x": 1}),
        ),
        "monitors.primary",
    );
    refused(
        with("primary", id("a", &"m".repeat(65), "c")),
        "monitors.primary",
    );
    refused(with("primary", id("a\nb", "m", "c")), "monitors.primary");
    refused(
        with("auto_profiles", json!("yes")),
        "monitors.auto_profiles",
    );
    refused(with("bar", json!({"show": "some"})), "monitors.bar");
    refused(
        with("bar", json!({"show": "all", "extra": 1})),
        "monitors.bar",
    );
    let many: Vec<Value> = (0..17).map(|i| id("a", "b", &i.to_string())).collect();
    refused(
        with("bar", json!({"show": "chosen", "on": many})),
        "monitors.bar",
    );
    for count in [0, 21] {
        refused(
            with("workspaces", json!({"strategy": "off", "count": count})),
            "monitors.workspaces",
        );
    }
    refused(
        with("workspaces", json!({"strategy": "later"})),
        "monitors.workspaces",
    );
    let slot = |w: u64| json!({"workspace": w, "monitor": id("a", "b", "c")});
    refused(
        with(
            "workspaces",
            json!({"strategy": "manual", "count": 4, "manual": [slot(0)]}),
        ),
        "monitors.workspaces",
    );
    refused(
        with(
            "workspaces",
            json!({"strategy": "manual", "count": 4, "manual": [slot(5)]}),
        ),
        "monitors.workspaces",
    );
    refused(
        with(
            "workspaces",
            json!({"strategy": "manual", "count": 4, "manual": [slot(2), slot(2)]}),
        ),
        "monitors.workspaces",
    );
    for bad in ["bogus", "m-12", "m-123456789", "M-7f2c", "m-zzzz"] {
        refused(
            arr(arrangement(bad, "Desk", "A")),
            "monitors.profiles[0].id",
        );
    }
    refused(
        with(
            "profiles",
            json!([
                arrangement("m-7f2c", "A", "1"),
                arrangement("m-7f2c", "B", "2")
            ]),
        ),
        "monitors.profiles[1].id",
    );
    refused(
        with(
            "profiles",
            json!([
                arrangement("m-7f2c", "A", "1"),
                arrangement("m-7f2d", "B", "1")
            ]),
        ),
        "monitors.profiles[1]",
    );
    let thirty_three: Vec<Value> = (0..33)
        .map(|i| arrangement(&format!("m-{i:04x}"), "A", &i.to_string()))
        .collect();
    refused(with("profiles", json!(thirty_three)), "monitors.profiles");
    for name in ["", &"n".repeat(61), "a\tb"] {
        refused(
            arr(arrangement("m-7f2c", name, "1")),
            "monitors.profiles[0].name",
        );
    }
    let mut seventeen = arrangement("m-7f2c", "A", "1");
    seventeen["outputs"] =
        json!((0..17)
        .map(|i| json!({"monitor": id("a", "b", &i.to_string()), "hint": "DP-1", "config": {}}))
        .collect::<Vec<_>>());
    refused(arr(seventeen), "monitors.profiles[0].outputs");
    let mut bad_cfg = arrangement("m-7f2c", "A", "1");
    bad_cfg["outputs"][0]["config"] = json!({"scale": 9.0});
    refused(arr(bad_cfg), "monitors.profiles[0].outputs[0]");
    let mut unknown = arrangement("m-7f2c", "A", "1");
    unknown["bogus"] = json!(1);
    refused(arr(unknown), "monitors.profiles[0]");
    let mut bad_hint = arrangement("m-7f2c", "A", "1");
    bad_hint["outputs"][0]["hint"] = json!("DP 1");
    refused(arr(bad_hint), "monitors.profiles[0].outputs[0].hint");
}

#[test]
fn lenient_load_costs_only_the_bad_key() {
    let toml = r#"
theme = "rosewood"
[monitors]
primary = 3
bar = "everywhere"
[monitors.outputs.DP-1]
mode = "preferred"
[[monitors.profiles]]
id = "bogus"
name = "Bad"
[[monitors.profiles]]
id = "m-7f2c"
name = "Desk"
[[monitors.profiles.outputs]]
hint = "DP-1"
[monitors.profiles.outputs.monitor]
make = "Dell"
model = "U2720Q"
serial = "A"
[monitors.profiles.outputs.config]
mode = "2560x1440@144"
[monitors.workspaces]
strategy = "manual"
count = 4
[[monitors.workspaces.manual]]
workspace = 9
[monitors.workspaces.manual.monitor]
make = "a"
model = "b"
serial = "c"
"#;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(&p, toml).unwrap();
    let s = State::load(&p);
    let m = &s.settings.monitors;
    assert_eq!(s.theme, "rosewood");
    assert_eq!(m.primary, None);
    assert_eq!(m.bar, BarChoice::default());
    assert_eq!(m.outputs["DP-1"].mode.as_deref(), Some("preferred"));
    assert_eq!(m.profiles.len(), 1, "{:?}", m.profiles);
    assert_eq!(m.profiles[0].id, "m-7f2c");
    assert_eq!(m.workspaces.strategy, Strategy::Manual);
    assert!(
        m.workspaces.manual.is_empty(),
        "slot outside 1..=count dropped"
    );
}

#[test]
fn repaired_gives_duplicate_ids_new_ones() {
    let p = |aid: &str, serial: &str| -> MonitorProfile {
        serde_json::from_value(arrangement(aid, "A", serial)).unwrap()
    };
    let m = Monitors {
        profiles: vec![p("m-7f2c", "1"), p("m-7f2c", "2"), p("m-7f2c", "3")],
        ..Monitors::default()
    };
    let (m, errs) = m.sanitised();
    let ids: BTreeSet<_> = m.profiles.iter().map(|p| p.id.clone()).collect();
    assert_eq!(m.profiles.len(), 3);
    assert_eq!(ids.len(), 3, "{ids:?}");
    assert_eq!(m.profiles[0].id, "m-7f2c", "first holder keeps it");
    assert!(ids.iter().all(|i| valid_profile_id(i)));
    assert!(!errs.is_empty());
}

#[test]
fn repaired_cuts_past_32() {
    let m = Monitors {
        profiles: (0..40)
            .map(|i| {
                serde_json::from_value(arrangement(&format!("m-{i:04x}"), "A", &i.to_string()))
                    .unwrap()
            })
            .collect(),
        ..Monitors::default()
    };
    let (m, errs) = m.sanitised();
    assert_eq!(m.profiles.len(), 32);
    assert!(!errs.is_empty());
    assert_eq!(m.profiles[0].id, "m-0000");
}
