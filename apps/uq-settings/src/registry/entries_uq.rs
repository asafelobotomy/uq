//! Search entries for the uQ pages: shell, profiles, rules, history, problems, keybinds and startup.

use super::*;

pub(crate) static SHELL_SETTINGS: [SearchEntry; 20] = [
    entry(
        "pins",
        "Pinned apps",
        &["favorites", "favourites", "launcher", "dock"],
    ),
    entry(
        "providers",
        "Launcher search providers",
        &["launcher", "calculator", "run", "results", "order"],
    ),
    entry(
        "islands",
        "Edit the bar",
        &[
            "bar", "panel", "islands", "widgets", "show", "hide", "move", "resize", "size",
            "weather", "stocks", "news",
        ],
    ),
    entry(
        "reset-bar",
        "Reset the bar",
        &["bar", "panel", "islands", "default", "layout"],
    ),
    entry(
        "edge-indicators",
        "Screen-edge indicators",
        &[
            "edge",
            "side",
            "next window",
            "neighbour",
            "neighbor",
            "scroll",
            "hot edge",
        ],
    ),
    entry(
        "edge-style",
        "Screen-edge indicator style",
        &["edge", "peek", "slice", "tab", "fullscreen", "preview"],
    ),
    entry(
        "edge-titles",
        "Window titles at the screen's edges",
        &["edge", "title", "name", "text", "label", "icon only"],
    ),
    entry(
        "fit-windows",
        "Keep windows on screen",
        &[
            "fit",
            "scale",
            "scaling",
            "clipped",
            "cut off",
            "off screen",
            "too big",
            "shrink",
            "resize",
            "floating",
            "window",
        ],
    ),
    entry(
        "notif-backend",
        "Notification service",
        &[
            "notifications",
            "notifi",
            "mako",
            "dunst",
            "swaync",
            "daemon",
        ],
    ),
    entry(
        "notif-summary",
        "Notification summary in the bar",
        &["notifications", "count", "badge"],
    ),
    entry(
        "idle-dim",
        "Dim the screen",
        &["idle", "brightness", "timeout"],
    ),
    entry(
        "idle-lock",
        "Lock after",
        &["idle", "lock screen", "timeout"],
    ),
    entry(
        "idle-screen-off",
        "Turn the screen off",
        &["idle", "display", "dpms", "timeout"],
    ),
    entry(
        "idle-suspend",
        "Suspend on battery",
        &["idle", "sleep", "power", "timeout"],
    ),
    entry(
        "idle-locked-screen-off",
        "Screen off after locking",
        &["idle", "lock", "display", "seconds"],
    ),
    entry(
        "idle-media",
        "Media keeps the screen from locking",
        &["idle", "video", "inhibit", "playing"],
    ),
    entry(
        "privacy-ip",
        "Public IP lookup",
        &["privacy", "network", "online", "address"],
    ),
    entry(
        "privacy-launches",
        "Remember launched apps",
        &["privacy", "history", "record", "frequency"],
    ),
    entry(
        "clear-launch-counts",
        "Clear launch history",
        &["privacy", "forget", "reset", "launcher"],
    ),
    entry(
        "print-key",
        "Print key",
        &[
            "screenshot",
            "print screen",
            "prtsc",
            "capture",
            "snip",
            "region",
            "satty",
            "mark up",
            "annotate",
            "uqview",
        ],
    ),
];

pub(crate) static PROFILES_SETTINGS: [SearchEntry; 2] = [
    entry(
        "profiles",
        "Profiles",
        &["modes", "gaming", "presentation", "switch", "preset"],
    ),
    entry(
        "profile-files",
        "Export or import a profile",
        &["share", "file", "backup", "copy"],
    ),
];

pub(crate) static RULES_SETTINGS: [SearchEntry; 6] = [
    entry(
        "window-rules",
        "Window rules",
        &[
            "float",
            "workspace",
            "opacity",
            "app",
            "window",
            "rule",
            "pin",
        ],
    ),
    entry(
        "pick-window",
        "Pick a window",
        &["click", "choose", "which window"],
    ),
    entry(
        "why-window",
        "Why does this window…?",
        &["explain", "matches", "rule"],
    ),
    entry(
        "panel-rules",
        "Panel rules",
        &["blur", "notifications", "bar", "layer"],
    ),
    entry(
        "workspace-rules",
        "Workspace rules",
        &["monitor", "persistent", "gaps"],
    ),
    entry(
        "rules-from-apps",
        "Rules from apps",
        &["game", "emulator", "fullscreen", "installed", "integration"],
    ),
];

pub(crate) static RESTORE_SETTINGS: [SearchEntry; 10] = [
    entry(
        "reset-all",
        "Reset all settings",
        &["reset", "defaults", "factory", "start again", "clear"],
    ),
    entry(
        "backup",
        "Save the settings to a file",
        &["backup", "export", "save", "copy", "move"],
    ),
    entry(
        "backup-restore",
        "Bring back settings from a file",
        &["backup", "import", "load", "restore"],
    ),
    entry(
        "rebase",
        "Rebase the system",
        &["rebase", "reinstall", "repair", "bindle", "as installed"],
    ),
    entry(
        "factory-restore",
        "Factory Restore",
        &["factory", "erase", "reinstall", "wipe", "install again"],
    ),
    entry(
        "keep-installer",
        "Keep the installer on this computer",
        &["installer", "offline", "iso", "boot menu"],
    ),
    entry(
        "take-snapshot",
        "Take a snapshot",
        &["snapshot", "snapper", "btrfs", "backup", "save"],
    ),
    entry(
        "snapshot-timeline",
        "Take a snapshot every hour",
        &["snapshot", "automatic", "timeline", "hourly"],
    ),
    entry(
        "snapshot-limits",
        "Snapshots kept",
        &["snapshot", "keep", "limit", "space", "cleanup"],
    ),
    entry(
        "rollback",
        "Roll back the system",
        &["rollback", "roll back", "snapshot", "undo update"],
    ),
];

pub(crate) static HISTORY_SETTINGS: [SearchEntry; 2] = [
    entry(
        "history",
        "Settings history",
        &["undo", "revert", "changes", "log"],
    ),
    entry(
        "restore-points",
        "Restore points",
        &["backup", "snapshot", "restore", "go back"],
    ),
];

pub(crate) static PROBLEMS_SETTINGS: [SearchEntry; 2] = [
    entry(
        "crashes",
        "Crash reports",
        &["crash", "crashed", "report", "bug", "copy report"],
    ),
    entry(
        "problems",
        "Warnings and errors",
        &[
            "error", "errors", "warning", "log", "journal", "debug", "code",
        ],
    ),
];

pub(crate) static KEYBINDS_SETTINGS: [SearchEntry; 5] = [
    entry(
        "shortcut-list",
        "Keyboard shortcuts",
        &["keys", "keybinds", "hotkeys", "bind"],
    ),
    entry(
        "add-shortcut",
        "Add a shortcut",
        &["custom", "command", "launch"],
    ),
    entry(
        "reset-shortcuts",
        "Put shortcuts back",
        &["reset", "default"],
    ),
    entry(
        "next-profile-shortcut",
        "Switch to the next profile",
        &["profile", "switch"],
    ),
    entry(
        "shortcut-capture",
        "Press keys to set a shortcut",
        &["record", "capture"],
    ),
];

pub(crate) static STARTUP_SETTINGS: [SearchEntry; 10] = [
    entry(
        "login-apps",
        "Apps that start when you log in",
        &["autostart", "login", "startup", "boot"],
    ),
    entry(
        "add-startup",
        "Add an app or command",
        &["autostart", "add", "command"],
    ),
    entry("start-delay", "Start delay", &["delay", "wait", "later"]),
    entry(
        "start-stagger",
        "Space out the starts",
        &["stagger", "gap", "one at a time", "order"],
    ),
    entry(
        "start-retry",
        "Try again if an app fails",
        &["retry", "again", "fail", "crash"],
    ),
    entry(
        "start-restore",
        "Restore your last session",
        &["restore", "session", "reopen", "resume", "workspace"],
    ),
    entry(
        "logout-commands",
        "Commands at logout",
        &["logout", "log out", "shutdown", "script", "backup"],
    ),
    entry(
        "background-services",
        "Background services",
        &["service", "systemd", "daemon", "unit", "background"],
    ),
    entry(
        "services-show-all",
        "Show every service",
        &["service", "static", "all"],
    ),
    entry(
        "env-vars",
        "Environment variables",
        &["env", "variable", "path", "export"],
    ),
];
