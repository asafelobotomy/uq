//! The small setting groups: location, idle, notifications, privacy, pointer, UI and printing.

use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IdleTimings {
    pub dim: Option<u32>,
    pub lock: Option<u32>,
    pub screen_off: Option<u32>,
    pub suspend_battery: Option<u32>,
    pub locked_screen_off: u32,
    pub media_blocks_lock: bool,
}

impl Default for IdleTimings {
    fn default() -> Self {
        Self {
            dim: Some(240),
            lock: Some(300),
            screen_off: Some(360),
            suspend_battery: Some(1200),
            locked_screen_off: 30,
            media_blocks_lock: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotifBackend {
    #[default]
    Auto,
    Notifi,
    Builtin,
    Swaync,
    Mako,
    Dunst,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NotifSettings {
    pub backend: NotifBackend,
    pub show_summary: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacySettings {
    pub public_ip_lookup: bool,
    pub record_launches: bool,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            public_ip_lookup: true,
            record_launches: true,
        }
    }
}

/// libinput's pointer acceleration profile; `Default` leaves the device's own choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccelProfile {
    #[default]
    Default,
    Adaptive,
    Flat,
}

/// Pointer settings for mice, rendered into the `input` block of `generated.lua`.
/// Person-wide interface preferences (`[ui]`), not tied to a theme.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    /// Reduce motion: the shell skips its fades and crossfades and Hyprland's animations are
    /// off (generated.lua). Default false.
    pub reduce_motion: bool,
    /// What the Print key does (`uq-screenshot print` reads it from `state.toml`).
    pub print_action: PrintAction,
}

/// What Print takes a screenshot of. Shift+Print and Super+Print keep their own jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrintAction {
    /// Pick an area.
    #[default]
    Region,
    /// The screen under the pointer.
    Screen,
    /// The focused window.
    Window,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MouseSettings {
    /// -1.0 (slowest) to 1.0 (fastest); 0 is unchanged.
    pub sensitivity: f64,
    pub accel_profile: AccelProfile,
    pub natural_scroll: bool,
    pub left_handed: bool,
    /// Wheel scroll multiplier, 0.1 to 10.
    pub scroll_factor: f64,
}

impl Default for MouseSettings {
    fn default() -> Self {
        Self {
            sensitivity: 0.0,
            accel_profile: AccelProfile::Default,
            natural_scroll: false,
            left_handed: false,
            scroll_factor: 1.0,
        }
    }
}
