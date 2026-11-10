use std::collections::BTreeMap;
use std::path::PathBuf;
use uq_core::lua::{Layout, emit_generated};
use uq_core::rules::{Rules, WorkspaceRule};
use uq_core::settings::{
    AccelProfile, DeviceOverride, ForceSplit, KbLayout, NewStatus, Orientation, OutputConfig,
    Position, Settings,
};
use uq_core::theme::{ThemeRegistry, Variant};

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
    for (name, settings, rules) in [
        ("default", Settings::default(), Rules::default()),
        ("all-set", all_set(), ws_rules),
    ] {
        let out = emit_generated(theme, &layouts, &rules, &settings);
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
