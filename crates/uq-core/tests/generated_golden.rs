use std::collections::BTreeMap;
use std::path::PathBuf;
use uq_core::keybinds::Keybinds;
use uq_core::lua::{Layout, StartupRender, emit_generated};
use uq_core::rules::{Rules, WorkspaceRule};
use uq_core::settings::{
    AccelProfile, DeviceOverride, ForceSplit, KbLayout, NewStatus, Orientation, OutputConfig,
    Position, Settings,
};
use uq_core::theme::{ThemeRegistry, Variant};

/// Two overrides (a rebind and an off switch) and one shortcut of your own of each action kind.
fn all_set_binds() -> Keybinds {
    let custom = |id: &str, name: &str, combo: &str, action: serde_json::Value| serde_json::json!({"id": id, "name": name, "combo": combo, "action": action});
    let mut vol = custom(
        "k-0007",
        "Louder",
        "SUPER + F7",
        serde_json::json!({"kind": "shell", "name": "vol-up"}),
    );
    vol["locked"] = true.into();
    vol["repeating"] = true.into();
    let v = serde_json::json!({
        "overrides": [
            {"id": "uq.spotlight", "combo": "SUPER + K"},
            {"id": "uq.close", "enabled": false},
        ],
        "custom": [
            custom("k-0001", "Spotlight too", "SUPER + F1",
                serde_json::json!({"kind": "shell", "name": "spotlight"})),
            custom("k-0002", "Float", "SUPER + F2",
                serde_json::json!({"kind": "window", "name": "float"})),
            custom("k-0003", "Workspace nine", "SUPER + F3",
                serde_json::json!({"kind": "workspace", "name": "focus", "workspace": 9})),
            custom("k-0004", "Master layout", "SUPER + F4",
                serde_json::json!({"kind": "uq", "name": "layout", "layout": "master"})),
            custom("k-0005", "Firefox", "SUPER + F5",
                serde_json::json!({"kind": "launch", "app": "org.mozilla.firefox"})),
            custom("k-0006", "Screenshot", "SUPER + SHIFT + P",
                serde_json::json!({"kind": "command",
                    "command": "grim -g \"$(slurp)\" ~/it's a shot.png"})),
            vol,
        ],
    });
    Keybinds::from_section(&v, &Keybinds::default()).expect("the golden shortcuts are valid")
}

/// Every look and layout option away from "unset".
fn all_set() -> Settings {
    let mut s = Settings::default();
    s.look.gaps_in = Some(8);
    s.look.gaps_out = Some(20);
    s.look.border_width = Some(3);
    s.look.rounding = Some(14);
    s.look.blur = Some(true);
    s.look.blur_size = Some(6);
    s.look.blur_passes = Some(3);
    s.look.shadow = Some(false);
    s.look.animations = Some(true);
    s.look.animation_speed = Some(1.5);
    let lo = &mut s.layout_options;
    lo.default = Some(Layout::Master);
    lo.dwindle.preserve_split = Some(true);
    lo.dwindle.smart_split = Some(false);
    lo.dwindle.force_split = Some(ForceSplit::Right);
    lo.master.mfact = Some(0.6);
    lo.master.new_status = Some(NewStatus::Inherit);
    lo.master.orientation = Some(Orientation::Right);
    lo.scrolling.column_width = Some(0.7);
    lo.scrolling.fullscreen_on_one_column = Some(true);
    s.monitors.outputs.insert(
        "DP-1".into(),
        OutputConfig {
            mode: Some("2560x1440@144".into()),
            position: Some(Position { x: 0, y: 0 }),
            scale: Some(1.25),
            transform: Some(1),
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

#[test]
fn golden_default_and_all_set() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let reg = ThemeRegistry::load_dir(&root.join("../../themes")).unwrap();
    let theme = reg.get("graphite", Variant::Dark).unwrap();
    let layouts = BTreeMap::from([("2".to_string(), Layout::Scrolling)]);
    let bless = std::env::var_os("UQ_BLESS").is_some();
    let mut bad = Vec::new();
    // `all-set` carries a workspace rule for `2`, so the merged call is in the golden file.
    let ws_rules = Rules {
        workspace: vec![WorkspaceRule {
            workspace: "2".into(),
            monitor: Some("DP-1".into()),
            default: Some(true),
            persistent: Some(true),
            gaps_in: Some(4),
            gaps_out: Some(10),
            border_size: Some(2),
        }],
        ..Rules::default()
    };
    // Sharp X11 apps on, and three session variables (one hostile) through `hl.env`.
    let startup = StartupRender {
        zero_scaling: true,
        env: vec![
            ("GDK_SCALE".into(), "2".into()),
            ("MOZ_ENABLE_WAYLAND".into(), "1".into()),
            ("QUOTED".into(), "say \"hi\") hl.exec_cmd(\"x".into()),
        ],
        export: true,
    };
    for (name, settings, rules, binds, startup) in [
        (
            "default",
            Settings::default(),
            Rules::default(),
            Keybinds::default(),
            StartupRender::default(),
        ),
        (
            "all-set",
            all_set(),
            ws_rules,
            all_set_binds(),
            StartupRender::default(),
        ),
        (
            "startup-all",
            Settings::default(),
            Rules::default(),
            Keybinds::default(),
            startup,
        ),
    ] {
        let out = emit_generated(theme, &layouts, &rules, &binds, &settings, &startup);
        let path = root.join(format!("tests/golden/generated/{name}.lua"));
        if bless {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &out).unwrap();
        } else if std::fs::read_to_string(&path).ok().as_deref() != Some(out.as_str()) {
            bad.push(path.display().to_string());
        }
    }
    assert!(
        bad.is_empty(),
        "golden mismatch (UQ_BLESS=1 to update):\n{}",
        bad.join("\n")
    );
}
