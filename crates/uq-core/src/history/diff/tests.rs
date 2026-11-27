use super::*;

fn labels(d: &str, k: &str) -> Option<String> {
    (d == "look" && k == "gaps_in").then(|| "Gaps between windows".into())
}

#[test]
fn a_mouse_change_is_recorded_once() {
    let old = State::default();
    let mut new = old.clone();
    new.settings.mouse.sensitivity = 0.4;
    let domains: Vec<_> = diff(&old, &new, &labels)
        .into_iter()
        .map(|c| c.domain)
        .collect();
    assert_eq!(domains, ["input"]);
    new.settings.idle.dim = Some(30);
    let domains: Vec<_> = diff(&old, &new, &labels)
        .into_iter()
        .map(|c| c.domain)
        .collect();
    assert_eq!(domains, ["input", "settings"]);
}

#[test]
fn diff_reports_only_changed_domains() {
    let old = State::default();
    let mut new = old.clone();
    new.settings.look.gaps_in = Some(8);
    let mut old = old;
    old.settings.look.gaps_in = Some(5);
    let changes = diff(&old, &new, &labels);
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(changes[0].domain, "look");
    assert_eq!(changes[0].summary, "Gaps between windows: 5 → 8");
    assert_eq!(changes[0].before["gaps_in"], 5);
    assert_eq!(changes[0].after["gaps_in"], 8);
}

#[test]
fn diff_ignores_launch_counts() {
    let old = State::default();
    let mut new = old.clone();
    new.launch_counts.insert("firefox.desktop".into(), 4);
    assert!(diff(&old, &new, &|_, _| None).is_empty());
}

/// Welcome's bookkeeping is not a setting: finishing or resetting Welcome writes no history
/// entry (so there's nothing to label), and Undo never brings Welcome back.
#[test]
fn diff_ignores_welcome() {
    let old = State::default();
    let mut new = old.clone();
    new.welcome.done = true;
    new.welcome.version = crate::welcome::CURRENT_WELCOME_VERSION;
    assert!(diff(&old, &new, &|_, _| None).is_empty());
}

#[test]
fn diff_theme_pseudo_domain() {
    let old = State::default();
    let mut new = old.clone();
    new.theme = "rosewood".into();
    let changes = diff(&old, &new, &|_, _| None);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].domain, "theme");
    assert_eq!(changes[0].summary, "theme: graphite → rosewood");
    assert_eq!(changes[0].after["theme"], "rosewood");
}

#[test]
fn diff_settings_and_layouts_domains() {
    let old = State::default();
    let mut new = old.clone();
    new.settings.pinned = vec!["a.desktop".into()];
    new.layouts
        .insert("2".into(), crate::lua::Layout::Scrolling);
    let names: Vec<_> = diff(&old, &new, &|_, _| None)
        .into_iter()
        .map(|c| c.domain)
        .collect();
    assert_eq!(names, ["layouts", "settings"]);
}

#[test]
fn revert_changed_touches_only_the_entry_s_leaves() {
    let before = json!({"gaps_in": 5, "rounding": 3});
    let after = json!({"gaps_in": 8, "rounding": 3});
    let current = json!({"gaps_in": 8, "rounding": 6, "blur": true});
    assert_eq!(
        revert_changed(&current, &before, &after),
        json!({"gaps_in": 5, "rounding": 6, "blur": true})
    );
}

#[test]
fn revert_changed_removes_added_leaves_and_empty_parents() {
    // Output names may hold dots; paths are walked by segment, never re-split.
    let before = json!({"outputs": {}});
    let after = json!({"outputs": {"HDMI.A": {"image": "/a.png", "fit": "fill"}}});
    let current = json!({"outputs": {"HDMI.A": {"image": "/a.png", "fit": "fill"}, "DP-1": {"image": "/b.png"}}});
    assert_eq!(
        revert_changed(&current, &before, &after),
        json!({"outputs": {"DP-1": {"image": "/b.png"}}})
    );
    // Keys the entry changed go back even if they changed again since (the conflict
    // prompt covers that); an object left empty goes too, which reads as "nothing set".
    let current = json!({"outputs": {"HDMI.A": {"image": "/c.png", "fit": "fit"}}});
    assert_eq!(revert_changed(&current, &before, &after), json!({}));
}

#[test]
fn revert_changed_restores_removed_leaves() {
    let before = json!({"workspaces": {"2": "master"}, "default": "dwindle"});
    let after = json!({"workspaces": {}, "default": "dwindle"});
    let current = json!({"workspaces": {"3": "scrolling"}, "default": "master"});
    assert_eq!(
        revert_changed(&current, &before, &after),
        json!({"workspaces": {"3": "scrolling", "2": "master"}, "default": "master"})
    );
}

#[test]
fn summary_caps_at_three_leaves() {
    let before = json!({});
    let after = json!({"gaps_in": 1, "gaps_out": 2, "rounding": 3, "blur": true, "shadow": false});
    assert_eq!(
        summarize("look", &before, &after, &|_, _| None),
        "blur: default → true, gaps_in: default → 1, gaps_out: default → 2, and 2 more"
    );
}
fn env_json(vars: &[(&str, &str)]) -> Value {
    json!({"vars": vars.iter().map(|(n, v)| json!({"name": n, "value": v})).collect::<Vec<_>>()})
}

#[test]
fn env_summary_names_never_values() {
    let none = env_json(&[]);
    let one = env_json(&[("GDK_BACKEND", "s3cret")]);
    let s = summarize("env", &none, &one, &|_, _| None);
    assert_eq!(s, "Environment variable GDK_BACKEND added");
    let changed = env_json(&[("GDK_BACKEND", "other")]);
    let s = summarize("env", &one, &changed, &|_, _| None);
    assert_eq!(s, "Environment variable GDK_BACKEND changed");
    let s = summarize("env", &changed, &none, &|_, _| None);
    assert_eq!(s, "Environment variable GDK_BACKEND removed");
    let many = env_json(&[
        ("A", "s3cret"),
        ("B", "s3cret"),
        ("C", "s3cret"),
        ("D", "s3cret"),
        ("E", "s3cret"),
    ]);
    let s = summarize("env", &none, &many, &|_, _| None);
    assert_eq!(
        s,
        "Environment variable A added, Environment variable B added, \
         Environment variable C added, and 2 more"
    );
    for line in [&s, &summarize("env", &many, &one, &|_, _| None)] {
        assert!(!line.contains("s3cret"), "{line}");
    }
}

#[test]
fn env_summaries_are_plain() {
    let a = env_json(&[("A", "1"), ("B", "2")]);
    let b = env_json(&[("B", "3"), ("C", "4")]);
    let c = env_json(&[("B", "2"), ("A", "1")]);
    for (x, y) in [(&a, &b), (&b, &a), (&a, &c), (&env_json(&[]), &a)] {
        let s = summarize("env", x, y, &|_, _| None);
        assert!(!s.contains(['{', '[']) && !s.contains("env."), "{s}");
    }
}

fn compat_json(sharp: bool, apps: Value) -> Value {
    json!({"sharp_x11": sharp, "electron_wayland": false, "chromium_wayland": false,
        "qt_wayland": false, "steam_scale": false, "apps": apps})
}

fn app(id: &str, platform: &str) -> Value {
    json!({"id": id, "platform": platform, "scale": null, "env": [], "args": [],
        "elsewhere": false})
}

#[test]
fn compat_summary() {
    let off = compat_json(false, json!([]));
    let on = compat_json(true, json!([]));
    let s = |b: &Value, a: &Value| summarize("compat", b, a, &|_, _| None);
    assert_eq!(s(&off, &on), "Sharp X11 apps turned on");
    assert_eq!(s(&on, &off), "Sharp X11 apps turned off");
    let mut both = on.clone();
    both["steam_scale"] = json!(true);
    assert_eq!(
        s(&off, &both),
        "Sharp X11 apps turned on, Steam UI scale turned on"
    );
    let code = compat_json(false, json!([app("code.desktop", "x11")]));
    assert_eq!(s(&off, &code), "Fix for code added");
    let changed = compat_json(false, json!([app("code.desktop", "wayland")]));
    assert_eq!(s(&code, &changed), "Fix for code changed");
    assert_eq!(s(&changed, &off), "Fix for code removed");
    let many = compat_json(
        true,
        json!([
            app("a.desktop", "x11"),
            app("b.desktop", "x11"),
            app("c.desktop", "x11")
        ]),
    );
    assert_eq!(
        s(&off, &many),
        "Sharp X11 apps turned on, Fix for a added, Fix for b added, and 1 more"
    );
    let reordered = compat_json(
        true,
        json!([
            app("c.desktop", "x11"),
            app("b.desktop", "x11"),
            app("a.desktop", "x11")
        ]),
    );
    assert_eq!(s(&many, &reordered), "App fixes reordered");
}

#[test]
fn compat_summaries_are_plain() {
    let a = compat_json(
        true,
        json!([app("code.desktop", "x11"), app("x.y.desktop", "auto")]),
    );
    let b = compat_json(false, json!([app("code.desktop", "wayland")]));
    let c = compat_json(false, json!([]));
    let hostile = compat_json(false, json!([app("[x]{y}.desktop", "x11")]));
    for (x, y) in [
        (&a, &b),
        (&b, &a),
        (&c, &a),
        (&a, &c),
        (&c, &c),
        (&c, &hostile),
    ] {
        let s = summarize("compat", x, y, &|_, _| None);
        assert!(!s.is_empty());
        assert!(
            !s.contains(['{', '[']) && !s.contains(".desktop") && !s.contains("compat."),
            "{s}"
        );
    }
}
