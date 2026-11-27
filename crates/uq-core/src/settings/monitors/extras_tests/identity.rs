//! Identity, layout equality and summaries.

use super::*;

#[test]
fn identity_check() {
    let ok = |m: &str| MonitorId {
        make: "a".into(),
        model: m.into(),
        serial: "c".into(),
    };
    assert!(ok(&"m".repeat(64)).check("k").is_ok());
    assert!(ok(&"m".repeat(65)).check("k").is_err());
    assert!(ok("x\u{7}y").check("k").is_err());
    assert!(MonitorId::default().check("k").is_ok());
    assert_eq!(MonitorId::default().label(), "A display");
    assert_eq!(ok("U2720Q").label(), "a U2720Q");
    let dell = MonitorId {
        make: "Dell Inc.".into(),
        model: "DELL U2720Q".into(),
        serial: String::new(),
    };
    assert_eq!(dell.label(), "DELL U2720Q");
    assert!(counts_in_sets("DP-1"));
    assert!(!counts_in_sets("FALLBACK"));
    assert!(!counts_in_sets("HEADLESS-2"));
    let live = LiveOutput {
        name: "DP-1".into(),
        make: "Dell Inc.".into(),
        model: "M".repeat(100),
        serial: "S".into(),
        ..LiveOutput::default()
    };
    let of = MonitorId::of(&live);
    assert_eq!(of.model.chars().count(), 64);
    assert!(of.check("k").is_ok());
}

#[test]
fn layout_eq_ignores_extras() {
    let a = section(full()).unwrap();
    let mut b = a.clone();
    b.primary = None;
    b.profiles.clear();
    b.bar = BarChoice::default();
    b.workspaces = WorkspacePlan::default();
    b.auto_profiles = true;
    assert!(a.layout_eq(&b));
    b.outputs.insert("DP-2".into(), OutputConfig::default());
    assert!(!a.layout_eq(&b));
    let c = a.with_layout(b.outputs.clone());
    assert_eq!(c.outputs, b.outputs);
    assert_eq!(c.primary, a.primary);
    assert_eq!(c.profiles, a.profiles);
}

#[test]
fn profile_set_is_a_sorted_multiset() {
    let p: MonitorProfile = serde_json::from_value(json!({"id": "m-0001", "name": "x",
        "outputs": [
            {"monitor": id("b", "b", "2"), "hint": "DP-1", "config": {}},
            {"monitor": id("a", "a", "1"), "hint": "DP-2", "config": {}},
            {"monitor": id("a", "a", "1"), "hint": "DP-3", "config": {}}]}))
    .unwrap();
    let serials: Vec<_> = p.set().into_iter().map(|m| m.serial).collect();
    assert_eq!(serials, ["1", "1", "2"]);
    assert!(valid_profile_id("m-0001") && valid_profile_id("m-0123abcd"));
}

#[test]
fn summaries_are_plain() {
    let none = json!({"outputs": {}});
    let main = |s: &str| json!({"outputs": {}, "primary": id("Dell Inc.", "DELL U2720Q", s)});
    assert_eq!(
        lines(none.clone(), main("A")),
        "Main display set to DELL U2720Q"
    );
    assert_eq!(lines(main("A"), none.clone()), "Main display cleared");
    let with_p =
        |name: &str| json!({"outputs": {}, "profiles": [arrangement("m-7f2c", name, "1")]});
    assert_eq!(
        lines(none.clone(), with_p("Desk")),
        "Display arrangement “Desk” saved"
    );
    assert_eq!(
        lines(with_p("Desk"), with_p("Home")),
        "Display arrangement “Desk” renamed to “Home”"
    );
    assert_eq!(
        lines(with_p("Desk"), none.clone()),
        "Display arrangement “Desk” removed"
    );
    let mut changed = with_p("Desk");
    changed["profiles"][0]["outputs"][0]["hint"] = json!("DP-2");
    assert_eq!(
        lines(with_p("Desk"), changed),
        "Display arrangement “Desk” updated"
    );
    let bar = |v: Value| json!({"outputs": {}, "bar": v});
    assert_eq!(
        lines(none.clone(), bar(json!({"show": "main", "on": []}))),
        "Bar shown on the main display only"
    );
    assert_eq!(
        lines(
            none.clone(),
            bar(json!({"show": "chosen", "on": [id("a", "b", "1"), id("a", "b", "2")]}))
        ),
        "Bar shown on 2 chosen screens"
    );
    assert_eq!(
        lines(bar(json!({"show": "main", "on": []})), none.clone()),
        "Bar shown on every screen"
    );
    let ws = |s: &str| json!({"outputs": {}, "workspaces": {"strategy": s, "count": 10}});
    assert_eq!(
        lines(none.clone(), ws("sequential")),
        "Workspaces across screens: one after another"
    );
    assert_eq!(
        lines(ws("sequential"), ws("interleave")),
        "Workspaces across screens: in turn"
    );
    assert_eq!(
        lines(ws("sequential"), ws("manual")),
        "Workspaces across screens: by hand"
    );
    assert_eq!(
        lines(ws("sequential"), none.clone()),
        "Workspaces across screens: off"
    );
    // Other leaves fall back to the generic lines, with plain labels.
    let off = json!({"outputs": {}, "auto_profiles": false});
    assert_eq!(
        lines(none.clone(), off),
        "Switch arrangements when screens change: default → false"
    );
    // Never raw JSON.
    let all = lines(none, full());
    for bad in ["{", "[", "m-7f", "make"] {
        assert!(!all.contains(bad), "{all}");
    }
}
