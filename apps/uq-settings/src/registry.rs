//! The core pages: their ids (the `ShowPage` page ids), names, icons, categories and search
//! entries. Plugin pages (Task 8) join them at runtime.

use serde::Serialize;

use crate::plugins::PluginPage;

/// The sidebar's groups (spec §2.2), in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(
    dead_code,
    reason = "pages for the other categories come in later slices"
)]
pub enum Category {
    Personal,
    Devices,
    Connections,
    Uq,
    System,
    Apps,
}

impl Category {
    #[allow(
        dead_code,
        reason = "the order the sidebar and the registry test rely on"
    )]
    pub const ALL: [Category; 6] = [
        Category::Personal,
        Category::Devices,
        Category::Connections,
        Category::Uq,
        Category::System,
        Category::Apps,
    ];

    /// The heading shown above the category's pages.
    pub fn label(self) -> &'static str {
        match self {
            Category::Personal => "Personal",
            Category::Devices => "Devices",
            Category::Connections => "Connections",
            Category::Uq => "uQ",
            Category::System => "System",
            Category::Apps => "Apps",
        }
    }
}

/// One searchable setting on a page. `id` is the `ShowPage` setting id and the `SettingRow` id.
#[derive(Debug, Serialize)]
pub struct SearchEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub keywords: &'static [&'static str],
}

#[derive(Debug, Serialize)]
pub struct PageInfo {
    pub id: &'static str,
    pub name: &'static str,
    /// A Freedesktop icon name.
    pub icon: &'static str,
    pub category: Category,
    /// The page's QML file, relative to `qml/Main.qml`.
    pub qml: &'static str,
    pub settings: &'static [SearchEntry],
    /// The history journal's domains whose changes this page makes (the History page's filter).
    pub domains: &'static [&'static str],
}

const fn entry(
    id: &'static str,
    label: &'static str,
    keywords: &'static [&'static str],
) -> SearchEntry {
    SearchEntry {
        id,
        label,
        keywords,
    }
}

static APPEARANCE_SETTINGS: [SearchEntry; 10] = [
    entry(
        "theme",
        "Theme",
        &["colors", "colours", "palette", "scheme"],
    ),
    entry(
        "variant",
        "Light or dark",
        &["variant", "mode", "dark mode", "light mode"],
    ),
    entry(
        "schedule",
        "Switch automatically",
        &["schedule", "sunrise", "sunset", "time", "auto"],
    ),
    entry(
        "ui-font",
        "Interface font",
        &["typeface", "text", "system font"],
    ),
    entry(
        "mono-font",
        "Monospace font",
        &["typeface", "terminal", "code"],
    ),
    entry(
        "font-size",
        "Font size",
        &["text size", "scale", "bigger text"],
    ),
    entry("cursor-theme", "Cursor theme", &["mouse", "pointer"]),
    entry(
        "cursor-size",
        "Cursor size",
        &["mouse", "pointer", "bigger cursor"],
    ),
    entry("icon-theme", "Icon theme", &["icons", "symbols"]),
    entry(
        "kvantum",
        "Style Qt apps to match",
        &["kvantum", "qt", "widgets", "style"],
    ),
];

static WALLPAPER_SETTINGS: [SearchEntry; 5] = [
    entry(
        "wallpaper-image",
        "Wallpaper image",
        &["background", "picture", "desktop background", "photo"],
    ),
    entry(
        "wallpaper-fit",
        "Wallpaper fit",
        &["fill", "stretch", "center", "tile", "scale"],
    ),
    entry(
        "wallpaper-slideshow",
        "Wallpaper slideshow folder",
        &["folder", "rotate", "cycle", "slideshow", "background"],
    ),
    entry(
        "wallpaper-interval",
        "Slideshow interval",
        &["change every", "minutes", "rotate", "time"],
    ),
    entry(
        "wallpaper-color",
        "Background colour",
        &["color", "solid", "fallback", "backdrop"],
    ),
];

static LOOK_SETTINGS: [SearchEntry; 15] = [
    entry(
        "gaps-inner",
        "Gaps between windows",
        &["gap", "spacing", "padding", "inner"],
    ),
    entry(
        "gaps-outer",
        "Gaps around the screen",
        &["gap", "margin", "outer", "edge"],
    ),
    entry(
        "border-width",
        "Window border width",
        &["outline", "frame", "thickness"],
    ),
    entry(
        "rounding",
        "Corner rounding",
        &["radius", "round corners", "corners"],
    ),
    entry(
        "blur",
        "Blur behind windows",
        &["frosted", "glass", "transparency"],
    ),
    entry(
        "blur-size",
        "Blur size",
        &["blur radius", "frosted", "strength"],
    ),
    entry(
        "blur-passes",
        "Blur passes",
        &["blur quality", "frosted", "iterations"],
    ),
    entry("shadow", "Window shadows", &["drop shadow", "depth"]),
    entry(
        "animations",
        "Animations",
        &["motion", "transitions", "effects"],
    ),
    entry(
        "animation-speed",
        "Animation speed",
        &["faster", "slower", "motion", "transitions"],
    ),
    entry(
        "default-layout",
        "Default layout",
        &["tiling", "dwindle", "master", "scrolling"],
    ),
    entry(
        "workspace-layouts",
        "Layout for each workspace",
        &["tiling", "workspace rules", "per workspace"],
    ),
    entry(
        "dwindle-options",
        "Dwindle layout options",
        &["split", "tiling", "preserve split"],
    ),
    entry(
        "master-options",
        "Master layout options",
        &["master", "mfact", "orientation", "stack"],
    ),
    entry(
        "scrolling-options",
        "Scrolling layout options",
        &["columns", "column width", "tiling"],
    ),
];

static SHELL_SETTINGS: [SearchEntry; 16] = [
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
        "Bar islands",
        &["bar", "panel", "widgets", "show", "hide"],
    ),
    entry(
        "clock-24h",
        "24-hour clock",
        &["time", "12 hour", "am pm", "format"],
    ),
    entry(
        "terminal",
        "Terminal",
        &["kitty", "console", "emulator", "default terminal"],
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
];

static PROFILES_SETTINGS: [SearchEntry; 2] = [
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

static RULES_SETTINGS: [SearchEntry; 5] = [
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
];

static HISTORY_SETTINGS: [SearchEntry; 2] = [
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

static DISPLAYS_SETTINGS: [SearchEntry; 8] = [
    entry(
        "display-arrangement",
        "Display arrangement",
        &["monitor", "screen", "position", "layout", "left", "right"],
    ),
    entry(
        "display-resolution",
        "Resolution",
        &["monitor", "screen", "size", "pixels", "4k"],
    ),
    entry(
        "display-refresh",
        "Refresh rate",
        &["monitor", "screen", "hz", "frame rate", "144"],
    ),
    entry(
        "display-scale",
        "Display scale",
        &["monitor", "screen", "zoom", "hidpi", "bigger", "smaller"],
    ),
    entry(
        "display-rotation",
        "Rotation",
        &[
            "monitor",
            "screen",
            "portrait",
            "landscape",
            "rotate",
            "flip",
        ],
    ),
    entry(
        "display-enabled",
        "Use this display",
        &["monitor", "screen", "turn off", "disable", "on"],
    ),
    entry(
        "display-mirror",
        "Mirror a display",
        &[
            "monitor",
            "screen",
            "duplicate",
            "same picture",
            "projector",
        ],
    ),
    entry(
        "display-vrr",
        "Variable refresh rate",
        &[
            "monitor",
            "screen",
            "vrr",
            "freesync",
            "g-sync",
            "adaptive sync",
            "hz",
        ],
    ),
];

static INPUT_SETTINGS: [SearchEntry; 14] = [
    entry(
        "keyboard-layouts",
        "Keyboard layouts",
        &[
            "keyboard", "language", "layout", "qwerty", "azerty", "dvorak",
        ],
    ),
    entry(
        "layout-switch",
        "Switch layouts with",
        &["keyboard", "language", "shortcut", "alt shift", "toggle"],
    ),
    entry(
        "repeat-delay",
        "Delay before keys repeat",
        &["keyboard", "repeat", "hold", "delay"],
    ),
    entry(
        "repeat-rate",
        "Key repeat speed",
        &["keyboard", "repeat", "rate", "typing"],
    ),
    entry(
        "mouse-speed",
        "Pointer speed",
        &["mouse", "sensitivity", "cursor", "faster", "slower"],
    ),
    entry(
        "mouse-accel",
        "Pointer acceleration",
        &["mouse", "acceleration", "flat", "adaptive", "precision"],
    ),
    entry(
        "mouse-natural-scroll",
        "Natural scrolling",
        &["mouse", "scroll direction", "reverse", "wheel"],
    ),
    entry(
        "mouse-left-handed",
        "Left-handed buttons",
        &["mouse", "swap buttons", "left hand"],
    ),
    entry(
        "mouse-scroll-speed",
        "Scroll speed",
        &["mouse", "wheel", "scrolling", "faster"],
    ),
    entry(
        "touchpad-natural-scroll",
        "Natural scrolling on the touchpad",
        &["touchpad", "trackpad", "scroll direction", "reverse"],
    ),
    entry(
        "tap-to-click",
        "Tap to click",
        &["touchpad", "trackpad", "tap", "click"],
    ),
    entry(
        "disable-while-typing",
        "Turn the touchpad off while typing",
        &["touchpad", "trackpad", "palm", "typing"],
    ),
    entry(
        "touchpad-scroll-speed",
        "Touchpad scroll speed",
        &["touchpad", "trackpad", "scrolling", "two finger"],
    ),
    entry(
        "device-overrides",
        "Settings for one pointing device",
        &["mouse", "touchpad", "trackball", "per device", "device"],
    ),
];

static COMPAT_SETTINGS: [SearchEntry; 7] = [
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

static STARTUP_SETTINGS: [SearchEntry; 4] = [
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
        "env-vars",
        "Environment variables",
        &["env", "variable", "path", "export"],
    ),
];

static CORE_PAGES: [PageInfo; 11] = [
    PageInfo {
        id: "appearance",
        name: "Appearance",
        icon: "preferences-desktop-theme",
        category: Category::Personal,
        qml: "pages/AppearancePage.qml",
        settings: &APPEARANCE_SETTINGS,
        domains: &["theme", "schedule", "appearance"],
    },
    PageInfo {
        id: "look",
        name: "Look and layouts",
        icon: "preferences-desktop-effects",
        category: Category::Personal,
        qml: "pages/LookPage.qml",
        settings: &LOOK_SETTINGS,
        domains: &["look", "layouts"],
    },
    PageInfo {
        id: "wallpaper",
        name: "Wallpaper",
        icon: "preferences-desktop-wallpaper",
        category: Category::Personal,
        qml: "pages/WallpaperPage.qml",
        settings: &WALLPAPER_SETTINGS,
        domains: &["wallpaper"],
    },
    PageInfo {
        id: "displays",
        name: "Displays",
        icon: "preferences-desktop-display",
        category: Category::Devices,
        qml: "pages/DisplaysPage.qml",
        settings: &DISPLAYS_SETTINGS,
        domains: &["monitors"],
    },
    PageInfo {
        id: "apps-scaling",
        name: "Apps and scaling",
        icon: "preferences-desktop-display-randr",
        category: Category::Devices,
        qml: "pages/AppsScalingPage.qml",
        settings: &COMPAT_SETTINGS,
        domains: &["compat"],
    },
    PageInfo {
        id: "input",
        name: "Input",
        icon: "preferences-desktop-peripherals",
        category: Category::Devices,
        qml: "pages/InputPage.qml",
        settings: &INPUT_SETTINGS,
        domains: &["input"],
    },
    PageInfo {
        id: "shell",
        name: "Shell",
        icon: "preferences-desktop",
        category: Category::Uq,
        qml: "pages/ShellPage.qml",
        settings: &SHELL_SETTINGS,
        domains: &["shell", "settings"],
    },
    PageInfo {
        id: "profiles",
        name: "Profiles",
        icon: "bookmarks",
        category: Category::Uq,
        qml: "pages/ProfilesPage.qml",
        settings: &PROFILES_SETTINGS,
        domains: &[],
    },
    PageInfo {
        id: "rules",
        name: "Window rules",
        icon: "preferences-system-windows",
        category: Category::Uq,
        qml: "pages/RulesPage.qml",
        settings: &RULES_SETTINGS,
        domains: &["rules"],
    },
    PageInfo {
        id: "history",
        name: "History",
        icon: "document-open-recent",
        category: Category::Uq,
        qml: "pages/HistoryPage.qml",
        settings: &HISTORY_SETTINGS,
        domains: &[],
    },
    PageInfo {
        id: "startup",
        name: "Startup",
        icon: "system-run",
        category: Category::System,
        qml: "pages/StartupPage.qml",
        settings: &STARTUP_SETTINGS,
        domains: &["autostart", "env"],
    },
];

/// The core pages, sorted by category.
pub fn core_pages() -> &'static [PageInfo] {
    &CORE_PAGES
}

pub fn find(id: &str) -> Option<&'static PageInfo> {
    core_pages().iter().find(|p| p.id == id)
}

/// A searchable setting, owned (plugin pages carry theirs from `page.toml`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedSearchEntry {
    pub id: String,
    pub label: String,
    pub keywords: Vec<String>,
}

/// A page as the sidebar, the loader and the search see it: core and plugin pages alike.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageEntry {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub category: Category,
    /// Core: relative to `qml/Main.qml`. Plugin: the page file's absolute `file://` URL.
    pub qml: String,
    pub plugin: bool,
    /// Page-level search keywords (plugin pages only).
    pub keywords: Vec<String>,
    pub settings: Vec<OwnedSearchEntry>,
}

impl From<&PageInfo> for PageEntry {
    fn from(p: &PageInfo) -> Self {
        PageEntry {
            id: p.id.into(),
            name: p.name.into(),
            icon: p.icon.into(),
            category: p.category,
            qml: p.qml.into(),
            plugin: false,
            keywords: Vec::new(),
            settings: p
                .settings
                .iter()
                .map(|s| OwnedSearchEntry {
                    id: s.id.into(),
                    label: s.label.into(),
                    keywords: s.keywords.iter().map(|k| (*k).into()).collect(),
                })
                .collect(),
        }
    }
}

impl From<&PluginPage> for PageEntry {
    fn from(p: &PluginPage) -> Self {
        PageEntry {
            id: p.id.clone(),
            name: p.name.clone(),
            icon: p.icon.clone(),
            category: Category::Apps,
            qml: crate::plugins::file_url(&p.qml_path),
            plugin: true,
            keywords: p.keywords.clone(),
            settings: p.settings.clone(),
        }
    }
}

/// Core pages first (in category order), then the plugin pages (Apps).
pub fn all_pages(plugins: &[PluginPage]) -> Vec<PageEntry> {
    core_pages()
        .iter()
        .map(PageEntry::from)
        .chain(plugins.iter().map(PageEntry::from))
        .collect()
}

/// The sidebar model: every page in category order, each with its category's heading, its QML
/// file, whether it is a plugin page and the history domains it changes.
pub fn pages_json(pages: &[PageEntry]) -> String {
    let pages: Vec<_> = pages
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "icon": p.icon,
                "category": p.category,
                "categoryName": p.category.label(),
                "qml": p.qml,
                "plugin": p.plugin,
                "domains": find(&p.id).filter(|_| !p.plugin).map_or(&[][..], |i| i.domains),
            })
        })
        .collect();
    serde_json::Value::Array(pages).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_unique_and_categories_ordered() {
        let pages = core_pages();
        let ids: HashSet<_> = pages.iter().map(|p| p.id).collect();
        assert_eq!(ids.len(), pages.len(), "duplicate page id");
        assert!(ids.contains("appearance") && ids.contains("look") && ids.contains("wallpaper"));
        assert!(
            pages.windows(2).all(|w| w[0].category <= w[1].category),
            "core pages out of category order"
        );
        let mut sorted = Category::ALL;
        sorted.sort();
        assert_eq!(
            sorted,
            Category::ALL,
            "Category::ALL out of declaration order"
        );
        let labels: Vec<_> = Category::ALL.iter().map(|c| c.label()).collect();
        assert_eq!(
            labels,
            ["Personal", "Devices", "Connections", "uQ", "System", "Apps"]
        );
        for p in pages {
            let ids: HashSet<_> = p.settings.iter().map(|s| s.id).collect();
            assert_eq!(
                ids.len(),
                p.settings.len(),
                "duplicate setting id on {}",
                p.id
            );
        }
    }

    #[test]
    fn provenance_rows_name_registry_settings() {
        let ids: HashSet<_> = core_pages()
            .iter()
            .flat_map(|p| p.settings.iter().map(|s| s.id))
            .collect();
        for row in uq_core::settings::provenance::OPTIONS {
            assert!(
                ids.contains(row.setting_id),
                "{} names setting {:?}, which no page lists",
                row.key,
                row.setting_id
            );
        }
    }

    #[test]
    fn history_page_registered_with_search_entries() {
        let page = find("history").expect("the history page is registered");
        assert_eq!(page.name, "History");
        assert_eq!(page.category, Category::Uq);
        assert_eq!(page.icon, "document-open-recent");
        assert_eq!(page.qml, "pages/HistoryPage.qml");
        let entries: Vec<_> = page
            .settings
            .iter()
            .map(|s| (s.id, s.label, s.keywords))
            .collect();
        assert_eq!(
            entries,
            [
                (
                    "history",
                    "Settings history",
                    &["undo", "revert", "changes", "log"][..]
                ),
                (
                    "restore-points",
                    "Restore points",
                    &["backup", "snapshot", "restore", "go back"][..]
                ),
            ]
        );
    }

    #[test]
    fn profiles_page_registered_with_search_entries() {
        let page = find("profiles").expect("the profiles page is registered");
        assert_eq!(page.name, "Profiles");
        assert_eq!(page.category, Category::Uq);
        assert_eq!(page.icon, "bookmarks");
        assert_eq!(page.qml, "pages/ProfilesPage.qml");
        assert!(page.domains.is_empty());
        let entries: Vec<_> = page
            .settings
            .iter()
            .map(|s| (s.id, s.label, s.keywords))
            .collect();
        assert_eq!(
            entries,
            [
                (
                    "profiles",
                    "Profiles",
                    &["modes", "gaming", "presentation", "switch", "preset"][..]
                ),
                (
                    "profile-files",
                    "Export or import a profile",
                    &["share", "file", "backup", "copy"][..]
                ),
            ]
        );
        let ids: Vec<_> = core_pages().iter().map(|p| p.id).collect();
        let at = ids.iter().position(|id| *id == "profiles").unwrap();
        assert_eq!(
            (ids[at - 1], ids[at + 1]),
            ("shell", "rules"),
            "between Shell and Window rules"
        );
    }

    #[test]
    fn rules_page_registered_with_search_entries() {
        let page = find("rules").expect("the rules page is registered");
        assert_eq!(page.name, "Window rules");
        assert_eq!(page.category, Category::Uq);
        assert_eq!(page.icon, "preferences-system-windows");
        assert_eq!(page.qml, "pages/RulesPage.qml");
        assert_eq!(page.domains, ["rules"]);
        let entries: Vec<_> = page
            .settings
            .iter()
            .map(|s| (s.id, s.label, s.keywords))
            .collect();
        assert_eq!(
            entries,
            [
                (
                    "window-rules",
                    "Window rules",
                    &[
                        "float",
                        "workspace",
                        "opacity",
                        "app",
                        "window",
                        "rule",
                        "pin"
                    ][..]
                ),
                (
                    "pick-window",
                    "Pick a window",
                    &["click", "choose", "which window"][..]
                ),
                (
                    "why-window",
                    "Why does this window…?",
                    &["explain", "matches", "rule"][..]
                ),
                (
                    "panel-rules",
                    "Panel rules",
                    &["blur", "notifications", "bar", "layer"][..]
                ),
                (
                    "workspace-rules",
                    "Workspace rules",
                    &["monitor", "persistent", "gaps"][..]
                ),
            ]
        );
        let ids: Vec<_> = core_pages().iter().map(|p| p.id).collect();
        let at = ids.iter().position(|id| *id == "rules").unwrap();
        assert_eq!(
            (ids[at - 1], ids[at + 1]),
            ("profiles", "history"),
            "between Profiles and History"
        );
    }

    #[test]
    fn every_journal_domain_has_one_page() {
        let mut domains: Vec<&str> = uq_core::settings::SECTION_DOMAINS.to_vec();
        domains.extend(["theme", "schedule", "settings"]);
        assert!(domains.contains(&"rules"));
        assert!(domains.contains(&"env") && domains.contains(&"compat"));
        assert!(domains.contains(&"autostart"));
        for domain in domains {
            let pages: Vec<_> = core_pages()
                .iter()
                .filter(|p| p.domains.contains(&domain))
                .map(|p| p.id)
                .collect();
            assert_eq!(pages.len(), 1, "{domain} is on pages {pages:?}");
        }
        let v: serde_json::Value = serde_json::from_str(&pages_json(&all_pages(&[]))).unwrap();
        assert_eq!(v[1]["domains"], serde_json::json!(["look", "layouts"]));
    }

    #[test]
    fn pages_json_lists_pages_in_order() {
        let v: serde_json::Value = serde_json::from_str(&pages_json(&all_pages(&[]))).unwrap();
        let ids: Vec<_> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            [
                "appearance",
                "look",
                "wallpaper",
                "displays",
                "apps-scaling",
                "input",
                "shell",
                "profiles",
                "rules",
                "history",
                "startup"
            ]
        );
        assert_eq!(v[0]["category"], "personal");
        assert_eq!(v[0]["categoryName"], "Personal");
        assert_eq!(v[1]["qml"], "pages/LookPage.qml");
        assert_eq!(v[2]["qml"], "pages/WallpaperPage.qml");
        assert_eq!(v[2]["plugin"], false);
        assert_eq!(v[3]["qml"], "pages/DisplaysPage.qml");
        assert_eq!(v[3]["categoryName"], "Devices");
        assert_eq!(v[4]["qml"], "pages/AppsScalingPage.qml");
        assert_eq!(v[4]["categoryName"], "Devices");
        assert_eq!(v[4]["domains"], serde_json::json!(["compat"]));
        assert_eq!(v[5]["qml"], "pages/InputPage.qml");
        assert_eq!(v[5]["categoryName"], "Devices");
        assert_eq!(v[6]["qml"], "pages/ShellPage.qml");
        assert_eq!(v[6]["categoryName"], "uQ");
        assert_eq!(v[10]["qml"], "pages/StartupPage.qml");
        assert_eq!(v[10]["category"], "system");
        assert_eq!(v[10]["categoryName"], "System");
        assert_eq!(v[10]["domains"], serde_json::json!(["autostart", "env"]));
    }

    #[test]
    fn startup_page_registered_with_search_entries() {
        let page = find("startup").expect("the startup page is registered");
        assert_eq!(page.name, "Startup");
        assert_eq!(page.category, Category::System);
        assert_eq!(page.icon, "system-run");
        assert_eq!(page.qml, "pages/StartupPage.qml");
        assert_eq!(page.domains, ["autostart", "env"]);
        let entries: Vec<_> = page
            .settings
            .iter()
            .map(|s| (s.id, s.label, s.keywords))
            .collect();
        assert_eq!(
            entries,
            [
                (
                    "login-apps",
                    "Apps that start when you log in",
                    &["autostart", "login", "startup", "boot"][..]
                ),
                (
                    "add-startup",
                    "Add an app or command",
                    &["autostart", "add", "command"][..]
                ),
                (
                    "start-delay",
                    "Start delay",
                    &["delay", "wait", "later"][..]
                ),
                (
                    "env-vars",
                    "Environment variables",
                    &["env", "variable", "path", "export"][..]
                ),
            ]
        );
        let pages = core_pages();
        let first_system = pages
            .iter()
            .position(|p| p.category == Category::System)
            .unwrap();
        assert_eq!(pages[first_system].id, "startup", "the first System page");
        assert_eq!(pages[first_system - 1].id, "history", "after the uQ pages");
    }

    #[test]
    fn apps_scaling_page_after_displays() {
        let page = find("apps-scaling").expect("the apps and scaling page is registered");
        assert_eq!(page.name, "Apps and scaling");
        assert_eq!(page.category, Category::Devices);
        assert_eq!(page.icon, "preferences-desktop-display-randr");
        assert_eq!(page.qml, "pages/AppsScalingPage.qml");
        assert_eq!(page.domains, ["compat"]);
        let entries: Vec<_> = page
            .settings
            .iter()
            .map(|s| (s.id, s.label, s.keywords))
            .collect();
        assert_eq!(
            entries,
            [
                (
                    "sharp-x11",
                    "Sharp X11 apps",
                    &["blurry", "xwayland", "scaling", "hidpi", "fractional"][..]
                ),
                (
                    "electron-wayland",
                    "Run Electron apps on Wayland",
                    &["electron", "blurry", "discord", "code"][..]
                ),
                (
                    "chromium-wayland",
                    "Run Chromium-based browsers on Wayland",
                    &["chrome", "brave", "browser", "blurry"][..]
                ),
                ("qt-wayland", "Qt apps prefer Wayland", &["qt", "kde"][..]),
                (
                    "steam-scale",
                    "Steam UI scale",
                    &["steam", "games", "size"][..]
                ),
                (
                    "blurry-now",
                    "Blurry right now",
                    &["fix", "app", "blurry", "x11"][..]
                ),
                (
                    "app-fixes",
                    "App fixes",
                    &["per-app", "wayland", "x11", "scale", "arguments"][..]
                ),
            ]
        );
        let ids: Vec<_> = core_pages().iter().map(|p| p.id).collect();
        let at = ids.iter().position(|id| *id == "apps-scaling").unwrap();
        assert_eq!(
            (ids[at - 1], ids[at + 1]),
            ("displays", "input"),
            "between Displays and Input"
        );
    }

    #[test]
    fn plugin_pages_follow_core_pages_in_apps() {
        let plugin = PluginPage {
            id: "demo".into(),
            name: "Demo".into(),
            icon: "x".into(),
            qml_path: "/usr/share/uq/settings/pages/demo/Page.qml".into(),
            dbus_name: None,
            keywords: vec![],
            settings: vec![],
        };
        let v: serde_json::Value =
            serde_json::from_str(&pages_json(&all_pages(&[plugin]))).unwrap();
        let n = v.as_array().unwrap().len();
        assert_eq!(n, core_pages().len() + 1);
        let last = &v[n - 1];
        assert_eq!(last["id"], "demo");
        assert_eq!(last["category"], "apps");
        assert_eq!(last["categoryName"], "Apps");
        assert_eq!(last["plugin"], true);
        assert_eq!(last["domains"], serde_json::json!([]));
        assert_eq!(
            last["qml"],
            "file:///usr/share/uq/settings/pages/demo/Page.qml"
        );
    }

    #[test]
    fn every_setting_has_keywords() {
        for p in core_pages() {
            for s in p.settings {
                assert!(
                    !s.keywords.is_empty() && s.keywords.iter().all(|k| !k.trim().is_empty()),
                    "{} on {} has no search keywords",
                    s.id,
                    p.id
                );
            }
        }
    }

    #[test]
    fn no_duplicate_setting_ids() {
        let mut seen = HashSet::new();
        for p in core_pages() {
            for s in p.settings {
                assert!(
                    seen.insert(s.id),
                    "setting id {:?} is on more than one page (second: {})",
                    s.id,
                    p.id
                );
            }
        }
    }
}
