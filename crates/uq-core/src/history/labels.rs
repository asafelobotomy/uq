//! Plain names for every key a history summary can show (spec §9: no internal names).

use crate::settings::provenance;

/// The plain label of `(domain, dotted key)` as a history summary shows it, for example
/// `("look", "gaps_in")` → "Gaps between windows" and `("wallpaper", "outputs.DP-1.image")` →
/// "Wallpaper on DP-1". `None` for a key this table doesn't know.
pub fn label(domain: &str, key: &str) -> Option<String> {
    if let Some(l) = provenance::label_for(domain, key) {
        return Some(l.into());
    }
    let fixed = match (domain, key) {
        ("theme", "theme") => "Theme",
        ("theme", "variant") => "Light or dark",
        ("schedule", "mode") => "Switch automatically",
        ("schedule", "light_at") => "Light from",
        ("schedule", "dark_at") => "Dark from",
        ("schedule", "lat") => "Sunrise and sunset place (latitude)",
        ("schedule", "lon") => "Sunrise and sunset place (longitude)",
        ("appearance", "kvantum") => "Style Qt apps to match",
        ("shell", "islands.system") => "Bar island: System",
        ("shell", "islands.work") => "Bar island: Workspaces",
        ("shell", "islands.time") => "Bar island: Clock",
        ("shell", "islands.notif") => "Bar island: Notifications",
        ("shell", "islands.status") => "Bar island: Status icons",
        ("shell", "providers") => "Launcher search providers",
        ("shell", "clock_24h") => "24-hour clock",
        ("shell", "terminal") => "Terminal",
        ("settings", "pinned") => "Pinned apps",
        ("settings", "weather_location") => "Weather location",
        ("settings", "idle.dim") => "Dim the screen",
        ("settings", "idle.lock") => "Lock after",
        ("settings", "idle.screen_off") => "Turn the screen off",
        ("settings", "idle.suspend_battery") => "Suspend on battery",
        ("settings", "idle.locked_screen_off") => "Screen off after locking",
        ("settings", "idle.media_blocks_lock") => "Media keeps the screen from locking",
        ("settings", "notif.backend") => "Notification service",
        ("settings", "notif.show_summary") => "Notification summary in the bar",
        ("settings", "privacy.public_ip_lookup") => "Public IP lookup",
        ("settings", "privacy.record_launches") => "Remember launched apps",
        ("settings", "mouse.sensitivity") => "Pointer speed",
        ("settings", "mouse.accel_profile") => "Pointer acceleration",
        ("settings", "mouse.natural_scroll") => "Natural scrolling",
        ("settings", "mouse.left_handed") => "Left-handed buttons",
        ("settings", "mouse.scroll_factor") => "Scroll speed",
        ("layouts", k) => {
            let ws = k.strip_prefix("workspaces.")?;
            return Some(format!("Workspace {ws} layout"));
        }
        ("rules", "window") => "Window rules",
        ("rules", "layer") => "Panel rules",
        ("rules", "workspace") => "Workspace rules",
        ("env", "vars") => "Environment variables",
        ("wallpaper", k) => return wallpaper(k),
        ("input", k) => return input(k),
        ("monitors", k) => return monitors(k),
        _ => return None,
    };
    Some(fixed.into())
}

/// `outputs.<output>.<field>`; the output name may hold dots, the field never does.
fn wallpaper(key: &str) -> Option<String> {
    let (output, field) = key.strip_prefix("outputs.")?.rsplit_once('.')?;
    let what = match field {
        "image" => "Wallpaper",
        "fit" => "Wallpaper fit",
        "folder" => "Slideshow folder",
        "interval" => "Slideshow interval",
        "color" => "Background colour",
        _ => return None,
    };
    Some(if output == "*" {
        format!("{what} (all screens)")
    } else {
        format!("{what} on {output}")
    })
}

/// `keyboard.*`, `mouse.*`, `touchpad.*` and `devices.<device>.<field>`. Older journals
/// recorded the mouse keys under `settings`, which keeps its own labels for them.
fn input(key: &str) -> Option<String> {
    if let Some(k) = key.strip_prefix("mouse.") {
        return label("settings", &format!("mouse.{k}"));
    }
    let fixed = match key {
        "keyboard.layouts" => "Keyboard layouts",
        "keyboard.switch_combo" => "Layout switch keys",
        "keyboard.repeat_rate" => "Key repeat rate",
        "keyboard.repeat_delay" => "Key repeat delay",
        "touchpad.natural_scroll" => "Touchpad natural scrolling",
        "touchpad.tap_to_click" => "Tap to click",
        "touchpad.disable_while_typing" => "Touchpad off while typing",
        "touchpad.scroll_factor" => "Touchpad scroll speed",
        k => {
            let (device, field) = k.strip_prefix("devices.")?.rsplit_once('.')?;
            let what = match field {
                "sensitivity" => "Pointer speed",
                "accel_profile" => "Pointer acceleration",
                "natural_scroll" => "Natural scrolling",
                "left_handed" => "Left-handed buttons",
                "scroll_factor" => "Scroll speed",
                _ => return None,
            };
            return Some(format!("{what} for {device}"));
        }
    };
    Some(fixed.into())
}

/// `outputs.<output>.<field>`, where `position` has `x` and `y` leaves.
fn monitors(key: &str) -> Option<String> {
    let rest = key.strip_prefix("outputs.")?;
    let (output, what) = if let Some(o) = rest.strip_suffix(".position.x") {
        (o, "Position (across)")
    } else if let Some(o) = rest.strip_suffix(".position.y") {
        (o, "Position (down)")
    } else {
        let (o, field) = rest.rsplit_once('.')?;
        let what = match field {
            "mode" => "Resolution and refresh rate",
            "position" => "Position",
            "scale" => "Scale",
            "transform" => "Rotation",
            "enabled" => "On or off",
            "mirror_of" => "Mirroring",
            "vrr" => "Variable refresh rate",
            "description" => "Display model",
            _ => return None,
        };
        (o, what)
    };
    Some(format!("{what} of {output}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::diff::{journal_domains, leaf_keys};
    use crate::lua::Layout;
    use crate::schedule::Schedule;
    use crate::settings::{
        AccelProfile, DeviceOverride, ForceSplit, KbLayout, Location, NewStatus, Orientation,
        OutputConfig, Position, ProviderPref,
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
}
