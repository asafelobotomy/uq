//! The settings, layouts, rules and keybinds every variant is rendered with.

use super::*;

/// Every mouse key away from its default, so the emitted `input` block is fully exercised.
pub(crate) fn mouse() -> MouseSettings {
    MouseSettings {
        sensitivity: -0.25,
        accel_profile: AccelProfile::Flat,
        natural_scroll: true,
        left_handed: true,
        scroll_factor: 1.5,
    }
}

/// Every look and layout option away from "unset", so CI's `--verify-config` checks each name.
pub(crate) fn settings() -> Settings {
    let mut s = Settings {
        mouse: mouse(),
        ..Settings::default()
    };
    s.look.gaps_in = Some(7);
    s.look.gaps_out = Some(18);
    s.look.border_width = Some(3);
    s.look.rounding = Some(12);
    s.look.blur = Some(true);
    s.look.blur_size = Some(5);
    s.look.blur_passes = Some(3);
    s.look.shadow = Some(false);
    s.look.animations = Some(true);
    s.look.animation_speed = Some(1.5);
    let lo = &mut s.layout_options;
    lo.default = Some(Layout::Scrolling);
    lo.dwindle.preserve_split = Some(true);
    lo.dwindle.smart_split = Some(true);
    lo.dwindle.force_split = Some(ForceSplit::Left);
    lo.master.mfact = Some(0.65);
    lo.master.new_status = Some(NewStatus::Slave);
    lo.master.orientation = Some(Orientation::Right);
    lo.scrolling.column_width = Some(0.6);
    lo.scrolling.fullscreen_on_one_column = Some(true);
    // Every monitor, keyboard, touchpad and device option away from "unset".
    s.monitors.outputs.insert(
        "DP-1".into(),
        OutputConfig {
            mode: Some("2560x1440@144".into()),
            position: Some(Position { x: 0, y: 0 }),
            scale: Some(1.25),
            transform: Some(1),
            enabled: Some(true),
            vrr: Some(1),
            ..Default::default()
        },
    );
    s.monitors.outputs.insert(
        "HDMI-A-1".into(),
        OutputConfig {
            mirror_of: Some("DP-1".into()),
            ..Default::default()
        },
    );
    s.monitors.outputs.insert(
        "eDP-1".into(),
        OutputConfig {
            enabled: Some(false),
            ..Default::default()
        },
    );
    let k = &mut s.input.keyboard;
    k.layouts = vec![
        KbLayout {
            layout: "us".into(),
            variant: None,
        },
        KbLayout {
            layout: "de".into(),
            variant: Some("nodeadkeys".into()),
        },
    ];
    k.switch_combo = Some("grp:alt_shift_toggle".into());
    k.repeat_rate = Some(35);
    k.repeat_delay = Some(250);
    let t = &mut s.input.touchpad;
    t.natural_scroll = Some(true);
    t.tap_to_click = Some(false);
    t.disable_while_typing = Some(true);
    t.scroll_factor = Some(0.8);
    s.input.devices.insert(
        "logitech-g-pro".into(),
        DeviceOverride {
            sensitivity: Some(-0.4),
            accel_profile: Some(AccelProfile::Flat),
            natural_scroll: Some(false),
            left_handed: Some(true),
            scroll_factor: Some(1.5),
        },
    );
    s
}

pub(crate) fn layouts() -> BTreeMap<String, Layout> {
    BTreeMap::from([
        ("1".into(), Layout::Master),
        ("2".into(), Layout::Scrolling),
        ("name:web".into(), Layout::Dwindle),
    ])
}

/// One window rule using every effect (split where they contradict), one panel rule with every
/// effect and one workspace rule with every field.
pub(crate) fn rules() -> Rules {
    let m = |class: &str| WindowMatch {
        class: Some(class.into()),
        ..Default::default()
    };
    let all_match = WindowMatch {
        class: Some("^(a|b)$".into()),
        title: Some("t.*".into()),
        initial_class: Some("c".into()),
        initial_title: Some("d".into()),
        tag: Some("web".into()),
        workspace: Some("2".into()),
        xwayland: Some(false),
        floating: Some(true),
        fullscreen: Some(false),
        pinned: Some(false),
        modal: Some(false),
    };
    let rule = |id: &str, enabled, matches, effects| WindowRule {
        id: id.into(),
        name: format!("Rule {id}"),
        enabled,
        matches,
        effects,
    };
    Rules {
        window: vec![
            rule(
                "r-0001",
                true,
                all_match,
                WindowEffects {
                    float: Some(true),
                    fullscreen: Some(false),
                    maximize: Some(false),
                    workspace: Some("name:web".into()),
                    silent: Some(true),
                    monitor: Some("DP-1".into()),
                    size: Some([800, 600]),
                    pin: Some(true),
                    no_initial_focus: Some(true),
                    opacity: Some(0.9),
                    no_blur: Some(true),
                    no_shadow: Some(true),
                    no_anim: Some(true),
                    rounding: Some(8),
                    border_size: Some(1),
                    idle_inhibit: Some("focus".into()),
                    keep_aspect_ratio: Some(true),
                    tag: Some("pinned".into()),
                    ..Default::default()
                },
            ),
            rule(
                "r-0002",
                false,
                m("other"),
                WindowEffects {
                    tile: Some(true),
                    position: Some([-10, 20]),
                    ..Default::default()
                },
            ),
            rule(
                "r-0003",
                true,
                m("centered"),
                WindowEffects {
                    center: Some(true),
                    ..Default::default()
                },
            ),
        ],
        layer: vec![LayerRule {
            id: "r-0a1b".into(),
            name: "Panel".into(),
            enabled: true,
            matches: LayerMatch {
                namespace: "^(noti-fi)$".into(),
            },
            effects: LayerEffects {
                blur: Some(true),
                ignore_alpha: Some(0.3),
                no_anim: Some(true),
                animation: Some("slide".into()),
            },
        }],
        workspace: vec![
            WorkspaceRule {
                workspace: "2".into(),
                monitor: Some("DP-1".into()),
                default: Some(true),
                persistent: Some(true),
                gaps_in: Some(3),
                gaps_out: Some(6),
                border_size: Some(2),
            },
            WorkspaceRule {
                workspace: "name:chat".into(),
                monitor: None,
                default: None,
                persistent: Some(true),
                gaps_in: None,
                gaps_out: None,
                border_size: None,
            },
        ],
    }
}

/// Rules whose strings try to break out of their Lua string (built directly, past validation).
pub(crate) fn hostile_rules() -> Rules {
    let bad = "a\"b\\c]]d\ne\") hl.exec_cmd(\"x";
    Rules {
        window: (0..3)
            .map(|i| WindowRule {
                id: format!("r-000{i}"),
                name: bad.into(),
                enabled: true,
                matches: WindowMatch {
                    class: Some(bad.into()),
                    title: Some(bad.into()),
                    ..Default::default()
                },
                effects: WindowEffects {
                    monitor: Some(bad.into()),
                    tag: Some(bad.into()),
                    ..Default::default()
                },
            })
            .collect(),
        layer: (0..2)
            .map(|i| LayerRule {
                id: format!("r-001{i}"),
                name: bad.into(),
                enabled: true,
                matches: LayerMatch {
                    namespace: bad.into(),
                },
                effects: LayerEffects {
                    animation: Some(bad.into()),
                    ..Default::default()
                },
            })
            .collect(),
        workspace: vec![WorkspaceRule {
            workspace: bad.into(),
            monitor: Some(bad.into()),
            default: None,
            persistent: None,
            gaps_in: None,
            gaps_out: None,
            border_size: None,
        }],
    }
}

/// A combo no default uses: three modifiers and the `i`th key of letters, digits and F1-F24.
pub(crate) fn free_combo(i: usize) -> String {
    let keys: Vec<&str> = KEYS
        .iter()
        .filter(|k| matches!(k.group, Group::Letter | Group::Digit | Group::Function))
        .map(|k| k.spelling)
        .collect();
    format!("SUPER + CTRL + ALT + {}", keys[i])
}

pub(crate) fn custom_bind(i: usize, name: &str, action: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"id": format!("k-{:04x}", i + 1), "name": name,
                       "combo": free_combo(i), "action": action})
}

/// Every default overridden or kept (the launcher and sound keys stay, as the checks below
/// look for them), `uq.profile.next` bound, and one shortcut of your own per action kind and
/// per value of each closed list.
pub(crate) fn keybinds() -> Keybinds {
    let mut custom = Vec::new();
    let mut add = |name: &str, action: serde_json::Value| {
        custom.push(custom_bind(custom.len(), name, action));
    };
    for n in SHELL_NAMES {
        add(n, serde_json::json!({"kind": "shell", "name": n}));
    }
    for n in ["close", "float", "pin"] {
        add(n, serde_json::json!({"kind": "window", "name": n}));
    }
    for d in ["left", "right", "up", "down"] {
        add(
            d,
            serde_json::json!({"kind": "window", "name": "focus", "direction": d}),
        );
    }
    for (n, w) in [("focus", 1), ("focus", 10), ("move", 1), ("move", 10)] {
        add(
            n,
            serde_json::json!({"kind": "workspace", "name": n, "workspace": w}),
        );
    }
    for n in ["toggle_special", "move_special", "next", "prev"] {
        add(n, serde_json::json!({"kind": "workspace", "name": n}));
    }
    for n in ["terminal", "exit", "profile-next", "settings"] {
        add(n, serde_json::json!({"kind": "uq", "name": n}));
    }
    for l in ["dwindle", "master", "scrolling"] {
        add(
            l,
            serde_json::json!({"kind": "uq", "name": "layout", "layout": l}),
        );
    }
    for m in ["next", "play-pause", "previous"] {
        add(
            m,
            serde_json::json!({"kind": "uq", "name": "media", "media": m}),
        );
    }
    add(
        "app",
        serde_json::json!({"kind": "launch", "app": "org.mozilla.firefox"}),
    );
    add(
        "cmd",
        serde_json::json!({"kind": "command", "command": "grim -g \"$(slurp)\" ~/it's.png"}),
    );
    // Your own volume key: locked and repeating, like the default.
    let mut vol = custom_bind(
        custom.len(),
        "Louder",
        serde_json::json!({"kind": "shell", "name": "vol-up"}),
    );
    vol["combo"] = "SUPER + F7".into();
    vol["locked"] = true.into();
    vol["repeating"] = true.into();
    custom.push(vol);
    let v = serde_json::json!({
        "overrides": [
            {"id": "uq.terminal", "combo": "SUPER + T"},
            {"id": "uq.close", "enabled": false},
            {"id": "uq.float", "combo": "SUPER + SHIFT + F"},
            {"id": "uq.ws.next", "combo": "SUPER + SHIFT + mouse_down"},
            {"id": "uq.window.drag", "combo": "ALT + mouse:272"},
            {"id": "uq.window.resize", "combo": "ALT + mouse:273"},
            {"id": "uq.profile.next", "combo": "SUPER + ALT + P"},
        ],
        "custom": custom,
    });
    Keybinds::from_section(&v, &Keybinds::default()).expect("the fixture shortcuts are valid")
}

/// One shortcut of your own per `KEYS` row a shortcut may use, each on its own free combo with
/// all four modifiers. The table is longer than the limit of 100, so it comes in chunks.
pub(crate) fn keybinds_keys() -> Vec<Keybinds> {
    let rows: Vec<_> = KEYS
        .iter()
        .filter(|k| matches!(k.kind, KeyKind::Key | KeyKind::Wheel))
        .collect();
    rows.chunks(60)
        .map(|chunk| {
            let custom: Vec<serde_json::Value> = chunk
                .iter()
                .enumerate()
                .map(|(i, k)| {
                    serde_json::json!({"id": format!("k-{:04x}", i + 1), "name": k.label,
                        "combo": format!("SUPER + SHIFT + CTRL + ALT + {}", k.spelling),
                        "action": {"kind": "window", "name": "close"}})
                })
                .collect();
            Keybinds::from_section(
                &serde_json::json!({ "custom": custom }),
                &Keybinds::default(),
            )
            .expect("every key is valid")
        })
        .collect()
}

/// The fixtures every variant is rendered with. The mouse buttons and Super keys only exist as
/// defaults; `keybinds()` moves them.
pub(crate) fn fixtures() -> Vec<Keybinds> {
    let mut v = vec![keybinds()];
    v.extend(keybinds_keys());
    v
}
