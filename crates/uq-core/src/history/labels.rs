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
        ("appearance", "theme_apps") => "Theme other apps",
        ("shell", "islands.system") => "Bar island: System",
        ("shell", "islands.work") => "Bar island: Workspaces",
        ("shell", "islands.time") => "Bar island: Clock",
        ("shell", "islands.notif") => "Bar island: Notifications",
        ("shell", "islands.status") => "Bar island: Status icons",
        ("shell", "bar.start") => "Bar: left islands",
        ("shell", "bar.center") => "Bar: centre islands",
        ("shell", "bar.end") => "Bar: right islands",
        ("shell", "bar.options.clock.seconds") => "Bar clock: seconds",
        ("shell", "bar.options.clock.date") => "Bar clock: date",
        ("shell", "bar.options.stocks.symbols") => "Stock ticker symbols",
        ("shell", "bar.options.news.feeds") => "News feeds",
        ("shell", "bar.options.worldclock.zones") => "World clock time zones",
        ("shell", "bar.options.work.sort") => "Open windows: order",
        ("shell", "bar.options.work.close") => "Open windows: close button",
        ("shell", "bar.options.work.descending") => "Open windows: reverse order",
        ("shell", "bar.options.sysmon.cpu") => "System monitor: processor",
        ("shell", "bar.options.sysmon.mem") => "System monitor: memory",
        ("shell", "bar.options.sysmon.gpu") => "System monitor: graphics card",
        ("shell", "bar.options.sysmon.temp") => "System monitor: temperature",
        ("shell", "bar.options.crumbs.warn_gb") => "Storage cleanup: amber at",
        ("shell", "bar.options.security.notify") => "Security: notifications",
        ("shell", "providers") => "Launcher search providers",
        ("shell", "clock_24h") => "24-hour clock",
        ("shell", "date_format") => "Date format",
        ("shell", "temp_unit") => "Temperature unit",
        ("shell", "terminal") => "Terminal",
        ("shell", "edge_indicators") => "Screen-edge indicators",
        ("shell", "edge_titles") => "Window titles on screen-edge indicators",
        ("shell", "edge_style") => "Screen-edge indicator style",
        ("shell", "fit_windows") => "Keep windows on screen",
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
        ("settings", "ui.reduce_motion") => "Reduce motion",
        ("settings", "ui.print_action") => "Print key",
        ("layouts", k) => {
            let ws = k.strip_prefix("workspaces.")?;
            return Some(format!("Workspace {ws} layout"));
        }
        ("rules", "window") => "Window rules",
        ("rules", "layer") => "Panel rules",
        ("rules", "workspace") => "Workspace rules",
        ("keybinds", "overrides") => "uQ's shortcuts",
        ("keybinds", "custom") => "Your own shortcuts",
        ("env", "vars") => "Environment variables",
        ("compat", "sharp_x11") => "Sharp X11 apps",
        ("compat", "electron_wayland") => "Run Electron apps on Wayland",
        ("compat", "chromium_wayland") => "Run Chromium-based browsers on Wayland",
        ("compat", "qt_wayland") => "Qt apps prefer Wayland",
        ("compat", "steam_scale") => "Steam UI scale",
        ("compat", "apps") => "App fixes",
        ("autostart", "files") => "Startup apps",
        ("autostart", "added") => "Apps and commands you added to startup",
        ("integration_rules", "enabled") => "Rules from apps",
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
        "gestures" => "Touchpad gestures",
        "swipe.distance" => "Swipe distance",
        "swipe.invert" => "Reverse swipe direction",
        "swipe.create_new" => "Swipe past the last workspace to make a new one",
        "swipe.cancel_ratio" => "How far to swipe before it switches",
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
    let fixed = match key {
        "primary.make" | "primary.model" | "primary.serial" => Some("Main display"),
        "auto_profiles" => Some("Switch arrangements when screens change"),
        "bar.show" => Some("Show the bar on"),
        "bar.on" => Some("Screens with a bar"),
        "workspaces.strategy" => Some("Workspaces across screens"),
        "workspaces.count" => Some("Planned workspaces"),
        "workspaces.manual" => Some("Workspaces on each screen"),
        "workspaces.move_back" => Some("Move workspaces back when a screen returns"),
        "profiles" => Some("Saved display arrangements"),
        _ => None,
    };
    if let Some(f) = fixed {
        return Some(f.into());
    }
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
mod tests;
