use super::*;
use crate::gestures::{Action, Direction, Gesture, Mods, SwipeOptions};
use crate::history::diff::{journal_domains, leaf_keys};
use crate::lua::Layout;
use crate::schedule::Schedule;
use crate::settings::{
    AccelProfile, BarChoice, BarShow, DeviceOverride, ForceSplit, KbLayout, Location, ManualSlot,
    MonitorId, MonitorProfile, NewStatus, Orientation, OutputConfig, Position, ProviderPref,
    SavedOutput, Strategy, WorkspacePlan,
};
use crate::state::State;
use std::path::PathBuf;

/// Keys that may stay unnamed in summaries. Keep this empty unless there is a reason.
const UNLABELLED: &[(&str, &str)] = &[];

/// A state with every optional value set, every map non-empty, and each schedule mode.
fn populated() -> Vec<State> {
    let mut st = State::default();
    let s = &mut st.settings;
    s.pinned = vec!["firefox.desktop".into()];
    s.weather_location = Some(Location {
        name: "London".into(),
        lat: 51.5,
        lon: -0.13,
    });
    s.idle.dim = Some(60);
    s.idle.lock = Some(120);
    s.idle.screen_off = Some(180);
    s.idle.suspend_battery = Some(900);
    let l = &mut s.look;
    l.gaps_in = Some(5);
    l.gaps_out = Some(10);
    l.border_width = Some(2);
    l.rounding = Some(3);
    l.blur = Some(true);
    l.blur_size = Some(4);
    l.blur_passes = Some(2);
    l.shadow = Some(true);
    l.animations = Some(true);
    l.animation_speed = Some(1.5);
    let a = &mut s.appearance;
    a.ui_font = Some("Inter".into());
    a.mono_font = Some("Iosevka".into());
    a.font_size = Some(11);
    a.cursor_theme = Some("Adwaita".into());
    a.cursor_size = Some(24);
    a.icon_theme = Some("Papirus".into());
    for name in ["*", "DP-1", "HDMI-A.1"] {
        let e = s.wallpaper.outputs.entry(name.into()).or_default();
        e.image = Some(PathBuf::from("/a.png"));
        e.folder = Some(PathBuf::from("/walls"));
        e.color = Some("#112233".into());
    }
    s.wallpaper.outputs.get_mut("*").unwrap().fit = crate::settings::Fit::Span;
    let lo = &mut s.layout_options;
    lo.default = Some(Layout::Master);
    lo.dwindle.preserve_split = Some(true);
    lo.dwindle.smart_split = Some(true);
    lo.dwindle.force_split = Some(ForceSplit::Left);
    lo.master.mfact = Some(0.6);
    lo.master.new_status = Some(NewStatus::Master);
    lo.master.orientation = Some(Orientation::Top);
    lo.scrolling.column_width = Some(0.5);
    lo.scrolling.fullscreen_on_one_column = Some(true);
    s.shell.providers = vec![ProviderPref {
        id: "calculator".into(),
        enabled: true,
    }];
    s.shell.terminal = Some("kitty.desktop".into());
    s.shell.edge_indicators = false;
    s.shell.edge_titles = true;
    s.shell.edge_style = crate::settings::EdgeStyle::Peek;
    s.shell.date_format = crate::settings::DateFormat::Us;
    s.shell.temp_unit = crate::settings::TempUnit::F;
    s.shell.fit_windows = false;
    s.mouse.sensitivity = 0.2;
    let kb = &mut s.input.keyboard;
    kb.layouts = vec![KbLayout {
        layout: "us".into(),
        variant: Some("intl".into()),
    }];
    kb.switch_combo = Some("grp:alt_shift_toggle".into());
    kb.repeat_rate = Some(30);
    kb.repeat_delay = Some(400);
    let tp = &mut s.input.touchpad;
    tp.natural_scroll = Some(true);
    tp.tap_to_click = Some(true);
    tp.disable_while_typing = Some(true);
    tp.scroll_factor = Some(0.5);
    s.input.gestures = Some(vec![Gesture {
        fingers: 3,
        direction: Direction::Down,
        action: Action::Special,
        modifier: Mods::parse("SUPER").unwrap(),
        workspace: Some("magic".into()),
        enabled: true,
    }]);
    s.input.swipe = SwipeOptions {
        distance: Some(400),
        invert: Some(false),
        create_new: Some(false),
        cancel_ratio: Some(0.4),
    };
    s.input.devices.insert(
        "logitech-mx.v2".into(),
        DeviceOverride {
            sensitivity: Some(0.1),
            accel_profile: Some(AccelProfile::Flat),
            natural_scroll: Some(true),
            left_handed: Some(true),
            scroll_factor: Some(1.5),
        },
    );
    for name in ["DP-1", "HDMI-A.1"] {
        s.monitors.outputs.insert(
            name.into(),
            OutputConfig {
                mode: Some("1920x1080@60".into()),
                position: Some(Position { x: 0, y: 0 }),
                scale: Some(1.25),
                transform: Some(1),
                enabled: Some(true),
                mirror_of: Some("eDP-1".into()),
                vrr: Some(1),
                description: Some("Dell U2720Q ABC123".into()),
            },
        );
    }
    let dell = MonitorId {
        make: "Dell Inc.".into(),
        model: "DELL U2720Q".into(),
        serial: "ABC123".into(),
    };
    let m = &mut s.monitors;
    m.primary = Some(dell.clone());
    m.auto_profiles = false;
    m.bar = BarChoice {
        show: BarShow::Chosen,
        on: vec![dell.clone()],
    };
    m.workspaces = WorkspacePlan {
        strategy: Strategy::Manual,
        count: 8,
        manual: vec![ManualSlot {
            workspace: 2,
            monitor: dell.clone(),
        }],
        move_back: false,
    };
    m.profiles = vec![MonitorProfile {
        id: "m-7f2c".into(),
        name: "Desk".into(),
        outputs: vec![SavedOutput {
            monitor: dell,
            hint: "DP-1".into(),
            config: OutputConfig::default(),
        }],
        workspaces: Some(WorkspacePlan::default()),
    }];
    st.layouts.insert("2".into(), Layout::Scrolling);
    st.set_section(
        "rules",
        serde_json::json!({
            "window": [{"name": "Float", "enabled": true,
                "match": {"class": "a"}, "effects": {"float": true}}],
            "layer": [{"name": "Blur", "enabled": true,
                "match": {"namespace": "x"}, "effects": {"blur": true}}],
            "workspace": [{"workspace": "3", "persistent": true}],
        }),
    )
    .unwrap();
    st.set_section(
        "env",
        serde_json::json!({"vars": [{"name": "GDK_BACKEND", "value": "x11"}]}),
    )
    .unwrap();
    st.set_section(
        "compat",
        serde_json::json!({"sharp_x11": true, "electron_wayland": true,
            "chromium_wayland": true, "qt_wayland": true, "steam_scale": true,
            "apps": [{"id": "code.desktop", "platform": "wayland", "scale": 1.5,
                "env": [{"name": "FOO", "value": "1"}], "args": ["--x"],
                "elsewhere": true}]}),
    )
    .unwrap();
    st.set_section(
        "autostart",
        serde_json::json!({"files": [{"id": "nm-applet.desktop", "enabled": false,
                "delay": 5}],
            "added": [{"name": "Sync", "app": "syncthing.desktop", "delay": 10},
                {"name": "Backup", "argv": ["backup", "--quiet"], "enabled": false}]}),
    )
    .unwrap();
    st.set_section(
        "keybinds",
        serde_json::json!({"overrides": [
                {"id": "uq.lookseedo", "combo": "SUPER + K"},
                {"id": "uq.close", "enabled": false}],
            "custom": [{"id": "k-3f9c", "name": "Screenshot",
                "combo": "SUPER + SHIFT + P", "action":
                {"kind": "command", "command": "grim shot.png"}}]}),
    )
    .unwrap();
    st.set_section(
        "integration_rules",
        serde_json::json!({"enabled": ["romforge.emulators"]}),
    )
    .unwrap();
    let schedules = [
        Schedule::Off,
        Schedule::Fixed {
            light_at: "07:00:00".parse().unwrap(),
            dark_at: "19:00:00".parse().unwrap(),
        },
        Schedule::Sun {
            lat: 51.5,
            lon: -0.13,
        },
    ];
    schedules
        .into_iter()
        .map(|schedule| State {
            schedule,
            ..st.clone()
        })
        .chain([State::default()])
        .collect()
}

#[test]
fn every_key_a_summary_can_show_has_a_plain_label() {
    let mut missing = Vec::new();
    for st in populated() {
        for domain in journal_domains() {
            for key in leaf_keys(&st, domain) {
                let entry = (domain, key.as_str());
                if label(domain, &key).is_none()
                    && !UNLABELLED.contains(&entry)
                    && !missing.contains(&format!("{domain}: {key}"))
                {
                    missing.push(format!("{domain}: {key}"));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "history summaries would show these internal key names: {missing:?}. Add a plain \
         label for each in crates/uq-core/src/history/labels.rs (`label`), and set the new \
         field in `populated()` so this test sees it. New keys (monitors and input in \
         Phase 3 slice 3 too) need a label before they ship."
    );
}

#[test]
fn every_profile_key_has_a_plain_label() {
    use crate::profile::{DomainKind, PROFILE_DOMAINS, key_label, leaves};
    let mut dotted_names = Vec::new();
    for st in populated() {
        let mut keys: Vec<(&str, Vec<String>)> = vec![
            ("appearance", vec!["theme".into()]),
            ("appearance", vec!["variant".into()]),
            ("power", vec!["profile".into()]),
        ];
        for (domain, kind) in PROFILE_DOMAINS {
            if matches!(kind, DomainKind::Section | DomainKind::Appearance) {
                keys.extend(
                    leaves(&st.section(domain).unwrap())
                        .into_iter()
                        .map(|(path, _)| (domain, path)),
                );
            }
        }
        for (domain, path) in keys {
            if key_label(domain, &path) == path.join(".") {
                dotted_names.push(format!("{domain}.{}", path.join(".")));
            }
        }
    }
    assert!(
        dotted_names.is_empty(),
        "a profile would show these internal key names: {dotted_names:?}. Add a plain label \
         for each in `label` above."
    );
}

#[test]
fn wildcard_paths_name_the_screen_or_workspace() {
    assert_eq!(
        label("wallpaper", "outputs.DP-1.image").as_deref(),
        Some("Wallpaper on DP-1")
    );
    assert_eq!(
        label("wallpaper", "outputs.*.image").as_deref(),
        Some("Wallpaper (all screens)")
    );
    assert_eq!(
        label("wallpaper", "outputs.HDMI-A.1.interval").as_deref(),
        Some("Slideshow interval on HDMI-A.1")
    );
    assert_eq!(
        label("layouts", "workspaces.2").as_deref(),
        Some("Workspace 2 layout")
    );
    assert_eq!(
        label("look", "gaps_in").as_deref(),
        Some("Gaps between windows")
    );
    assert_eq!(label("wallpaper", "outputs.DP-1.bogus"), None);
}

#[test]
fn summaries_use_plain_labels() {
    let old = State::default();
    let mut new = old.clone();
    new.variant = crate::theme::Variant::Light;
    new.settings.idle.suspend_battery = Some(600);
    let changes = crate::history::diff(&old, &new, &label);
    let lines: Vec<_> = changes.iter().map(|c| c.summary.as_str()).collect();
    assert!(lines.contains(&"Light or dark: dark → light"), "{lines:?}");
    assert!(
        lines.iter().any(|l| l.starts_with("Suspend on battery: ")),
        "{lines:?}"
    );
}

#[test]
fn rules_summary_uses_names() {
    let old = State::default();
    let mut new = old.clone();
    new.set_section(
        "rules",
        serde_json::json!({"window": [{"name": "Pavucontrol floats", "enabled": true,
            "match": {"class": "a"}, "effects": {"float": true}}]}),
    )
    .unwrap();
    let changes = crate::history::diff(&old, &new, &label);
    let lines: Vec<_> = changes.iter().map(|c| c.summary.as_str()).collect();
    assert_eq!(lines, ["Window rule “Pavucontrol floats” added"]);
}

#[test]
fn keybinds_summary_uses_labels() {
    let old = State::default();
    let mut new = old.clone();
    new.set_section(
        "keybinds",
        serde_json::json!({"overrides": [{"id": "uq.lookseedo", "combo": "SUPER + K"}],
            "custom": [{"name": "Screenshot", "combo": "SUPER + SHIFT + P",
                "action": {"kind": "command", "command": "grim shot.png"}}]}),
    )
    .unwrap();
    let changes = crate::history::diff(&old, &new, &label);
    let lines: Vec<_> = changes.iter().map(|c| c.summary.as_str()).collect();
    assert_eq!(
        lines,
        ["“Open look=see>do_” now uses Super + K, Shortcut “Screenshot” added"]
    );
}
