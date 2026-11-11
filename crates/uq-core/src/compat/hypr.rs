//! The one table of names Apps and scaling renders: the Hyprland option, and the environment
//! variable names and values each switch and each per-app platform sets.
//!
//! The renderer, the launch builder, the launcher overrides, Explain, the profile labels and the
//! page's catalogue all read from here, so a name that a live check finds wrong is fixed in one
//! row.
//!
//! Hyprland names were read from Hyprland's own source at two tags:
//! - `v0.56.2` (`efb50993780079460b0cbed1363e2166a2de1d9f`)
//! - `v0.57.0` (`0aa6745f705f2c4b0a5004eb019ef76e5bbdb426`)
//!
//! The environment variable names below are not Hyprland's: they are read by GTK, Qt, SDL,
//! Electron and Steam. They are live-check items (see the plan's "Not confirmed from source").

/// `src/config/values/ConfigValues.cpp`, line 549 at v0.56.2 and line 653 at v0.57.0:
/// `MS<Bool>("xwayland:force_zero_scaling", "forces a scale of 1 on xwayland windows on scaled
/// displays.", false)`. Rendered as `hl.config({ xwayland = { force_zero_scaling = true } })`.
/// Not confirmed from source: that a reload applies it to new X11 windows without restarting
/// XWayland, and that `getoption` answers for it (live checks).
pub const XWAYLAND_ZERO_SCALING: &str = "xwayland:force_zero_scaling";

/// The five global switches, in page order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwitchKey {
    SharpX11,
    ElectronWayland,
    ChromiumWayland,
    QtWayland,
    SteamScale,
}

impl SwitchKey {
    pub const ALL: [SwitchKey; 5] = [
        SwitchKey::SharpX11,
        SwitchKey::ElectronWayland,
        SwitchKey::ChromiumWayland,
        SwitchKey::QtWayland,
        SwitchKey::SteamScale,
    ];

    /// The section JSON key.
    pub fn key(self) -> &'static str {
        match self {
            SwitchKey::SharpX11 => "sharp_x11",
            SwitchKey::ElectronWayland => "electron_wayland",
            SwitchKey::ChromiumWayland => "chromium_wayland",
            SwitchKey::QtWayland => "qt_wayland",
            SwitchKey::SteamScale => "steam_scale",
        }
    }

    pub fn from_key(key: &str) -> Option<SwitchKey> {
        SwitchKey::ALL.into_iter().find(|k| k.key() == key)
    }

    /// The plain label the page, History and Explain show.
    pub fn label(self) -> &'static str {
        match self {
            SwitchKey::SharpX11 => "Sharp X11 apps",
            SwitchKey::ElectronWayland => "Run Electron apps on Wayland",
            SwitchKey::ChromiumWayland => "Run Chromium-based browsers on Wayland",
            SwitchKey::QtWayland => "Qt apps prefer Wayland",
            SwitchKey::SteamScale => "Steam UI scale",
        }
    }

    /// What it does, in one plain sentence, then when it applies.
    pub fn description(self) -> &'static str {
        match self {
            SwitchKey::SharpX11 => {
                "Older X11 apps draw at your screen's full sharpness instead of being stretched \
                 and blurry. Applies to apps started after the change."
            }
            SwitchKey::ElectronWayland => {
                "Apps built on Electron, such as many chat apps and code editors, run natively \
                 for sharp text. Applies to apps started after the change."
            }
            SwitchKey::ChromiumWayland => {
                "Chromium, Chrome, Brave, Vivaldi and Edge run natively for sharp text and \
                 smooth scaling. Applies to apps started after the change."
            }
            SwitchKey::QtWayland => {
                "Qt apps run natively when they can, and as X11 apps when they can't. Applies \
                 to apps started after the change."
            }
            SwitchKey::SteamScale => {
                "Steam's own windows follow your main screen's scale. Applies to apps started \
                 after the change."
            }
        }
    }
}

/// How a variable's value is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Fixed(&'static str),
    /// The scale as a whole number (GTK reads only integers): see `launch::gdk_scale`.
    MainScaleInt,
    /// The scale with at most two places: see `launch::fmt_scale`.
    MainScale,
}

impl Value {
    /// Whether the value needs a scale: such lines are left out of `environment.d` (F18), and
    /// out of every list when no scale is known (R13).
    pub fn scale_derived(&self) -> bool {
        !matches!(self, Value::Fixed(_))
    }
}

/// The variable each env-based switch sets, for `hl.env` and autostart children. The Chromium
/// switch writes flag files instead (`browsers.rs`), and Sharp X11 apps also renders
/// [`XWAYLAND_ZERO_SCALING`]. `QT_SCALE_FACTOR` is never global (R12).
/// Not from Hyprland's source: live-check items (GTK ignoring `GDK_SCALE` on Wayland, Electron 28+
/// reading `ELECTRON_OZONE_PLATFORM_HINT=auto`, Steam taking a fractional scale).
pub const SWITCH_ENV: [(SwitchKey, &str, Value); 4] = [
    (SwitchKey::SharpX11, "GDK_SCALE", Value::MainScaleInt),
    (
        SwitchKey::ElectronWayland,
        "ELECTRON_OZONE_PLATFORM_HINT",
        Value::Fixed("auto"),
    ),
    (
        SwitchKey::QtWayland,
        "QT_QPA_PLATFORM",
        Value::Fixed("wayland;xcb"),
    ),
    (
        SwitchKey::SteamScale,
        "STEAM_FORCE_DESKTOPUI_SCALING",
        Value::MainScale,
    ),
];

/// A per-app fix's "Run on" choice, by platform: each toolkit's variable, with a fallback after
/// the `,` or `;` so a toolkit without Wayland still starts. Toolkit names, not Hyprland's.
pub const PLATFORM_ENV: [(&str, [(&str, &str); 4]); 2] = [
    (
        "wayland",
        [
            ("GDK_BACKEND", "wayland,x11"),
            ("QT_QPA_PLATFORM", "wayland;xcb"),
            ("SDL_VIDEODRIVER", "wayland,x11"),
            ("ELECTRON_OZONE_PLATFORM_HINT", "wayland"),
        ],
    ),
    (
        "x11",
        [
            ("GDK_BACKEND", "x11"),
            ("QT_QPA_PLATFORM", "xcb"),
            ("SDL_VIDEODRIVER", "x11"),
            ("ELECTRON_OZONE_PLATFORM_HINT", "x11"),
        ],
    ),
];

/// A per-app fix's size: GTK's whole-number scale and Qt's exact one. Steam also gets the
/// Steam UI scale variable (the [`SwitchKey::SteamScale`] row of [`SWITCH_ENV`]).
pub const SCALE_ENV: [(&str, Value); 2] = [
    ("GDK_SCALE", Value::MainScaleInt),
    ("QT_SCALE_FACTOR", Value::MainScale),
];

/// The desktop id whose per-app size also sets Steam's own scale.
pub const STEAM_ID: &str = "steam.desktop";

/// The variable a switch sets, from [`SWITCH_ENV`].
pub fn switch_var(key: SwitchKey) -> Option<(&'static str, Value)> {
    SWITCH_ENV
        .iter()
        .find(|(k, _, _)| *k == key)
        .map(|(_, n, v)| (*n, *v))
}
