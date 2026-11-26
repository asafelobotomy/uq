//! Search entries for the System and Apps pages.

use super::*;

pub(crate) static COMPAT_SETTINGS: [SearchEntry; 7] = [
    entry(
        "sharp-x11",
        "Sharp X11 apps",
        &["blurry", "xwayland", "scaling", "hidpi", "fractional"],
    ),
    entry(
        "electron-wayland",
        "Run Electron apps on Wayland",
        &["electron", "blurry", "discord", "code"],
    ),
    entry(
        "chromium-wayland",
        "Run Chromium-based browsers on Wayland",
        &["chrome", "brave", "browser", "blurry"],
    ),
    entry("qt-wayland", "Qt apps prefer Wayland", &["qt", "kde"]),
    entry("steam-scale", "Steam UI scale", &["steam", "games", "size"]),
    entry(
        "blurry-now",
        "Blurry right now",
        &["fix", "app", "blurry", "x11"],
    ),
    entry(
        "app-fixes",
        "App fixes",
        &["per-app", "wayland", "x11", "scale", "arguments"],
    ),
];

pub(crate) static POWER_SETTINGS: [SearchEntry; 5] = [
    entry(
        "power-mode",
        "Power mode",
        &[
            "performance",
            "power saver",
            "balanced",
            "profile",
            "energy",
        ],
    ),
    entry(
        "battery",
        "Battery",
        &["charge", "health", "time left", "charging"],
    ),
    entry(
        "lid-action",
        "When the lid is closed",
        &["laptop", "lid", "sleep", "suspend", "hibernate"],
    ),
    entry(
        "lid-action-ac",
        "When the lid is closed while plugged in",
        &["laptop", "lid", "docked", "charger", "mains"],
    ),
    entry(
        "power-key-action",
        "When the power button is pressed",
        &["power button", "shut down", "sleep", "key"],
    ),
];

pub(crate) static USERS_SETTINGS: [SearchEntry; 2] = [
    entry(
        "user-accounts",
        "People who can log in",
        &[
            "user",
            "account",
            "password",
            "avatar",
            "picture",
            "add user",
            "remove user",
            "administrator",
            "admin",
        ],
    ),
    entry(
        "users-auto-login",
        "Auto-login",
        &["automatic login", "autologin", "without password"],
    ),
];

pub(crate) static LOGIN_SCREEN_SETTINGS: [SearchEntry; 7] = [
    entry(
        "greeter-theme",
        "Login screen theme",
        &["greeter", "login", "colors", "dark", "light"],
    ),
    entry(
        "greeter-wallpaper",
        "Login screen wallpaper",
        &["greeter", "login", "background", "picture"],
    ),
    entry(
        "greeter-displays",
        "Use my display layout at login",
        &["greeter", "monitors", "screens", "arrangement"],
    ),
    entry(
        "greeter-hide-users",
        "Hide the list of users",
        &["greeter", "user list", "privacy", "username"],
    ),
    entry(
        "greeter-reduce-motion",
        "Login screen motion",
        &["greeter", "animations", "reduce motion"],
    ),
    entry(
        "startup-banner",
        "Startup banner",
        &["boot", "shutdown", "ascii", "logo", "splash", "console"],
    ),
    entry(
        "auto-login",
        "Log in automatically",
        &[
            "autologin",
            "automatic login",
            "without password",
            "greeter",
        ],
    ),
];

pub(crate) static DATETIME_SETTINGS: [SearchEntry; 5] = [
    entry(
        "timezone",
        "Time zone",
        &["timezone", "region", "city", "clock", "utc"],
    ),
    entry(
        "ntp",
        "Set the time from the network",
        &["automatic time", "ntp", "sync", "internet time"],
    ),
    entry(
        "manual-time",
        "Date and time by hand",
        &["set date", "set time", "clock", "manual"],
    ),
    entry(
        "datetime-clock-24h",
        "24-hour clock in the bar",
        &["24 hour", "12 hour", "am pm", "clock format"],
    ),
    entry(
        "datetime-date-format",
        "Date format in the bar",
        &[
            "date format",
            "uk",
            "us",
            "american",
            "british",
            "month first",
            "day first",
        ],
    ),
];

pub(crate) static REGION_SETTINGS: [SearchEntry; 4] = [
    entry(
        "language",
        "System language",
        &["locale", "translation", "lang"],
    ),
    entry(
        "formats",
        "Formats",
        &[
            "date format",
            "number format",
            "currency",
            "units",
            "metric",
            "paper",
        ],
    ),
    entry(
        "temp-unit",
        "Temperature unit",
        &[
            "celsius",
            "fahrenheit",
            "temperature",
            "weather",
            "units",
            "degrees",
        ],
    ),
    entry(
        "console-keymap",
        "Keyboard layout for the text console",
        &["keymap", "console", "tty", "keyboard"],
    ),
];

pub(crate) static DEFAULT_APPS_SETTINGS: [SearchEntry; 9] = [
    entry(
        "default-browser",
        "Web browser",
        &["browser", "internet", "links", "web"],
    ),
    entry("default-mail", "Email", &["mail", "email", "mailto"]),
    entry(
        "default-files",
        "Files",
        &["file manager", "folders", "directory"],
    ),
    entry("default-text", "Text editor", &["text", "editor", "notes"]),
    entry(
        "default-images",
        "Pictures",
        &["image viewer", "photos", "images"],
    ),
    entry(
        "default-video",
        "Videos",
        &["video player", "movies", "film"],
    ),
    entry(
        "default-music",
        "Music",
        &["audio player", "songs", "music player"],
    ),
    entry(
        "default-pdf",
        "PDF documents",
        &["pdf", "document viewer", "reader"],
    ),
    entry(
        "default-terminal",
        "Terminal",
        &["terminal", "console", "command line", "shell"],
    ),
];

pub(crate) static ABOUT_SETTINGS: [SearchEntry; 2] = [
    entry(
        "hostname",
        "Computer name",
        &["hostname", "device name", "network name", "rename"],
    ),
    entry(
        "versions",
        "System details and versions",
        &[
            "version",
            "about",
            "kernel",
            "processor",
            "cpu",
            "memory",
            "ram",
            "graphics",
            "gpu",
            "hardware",
        ],
    ),
];
