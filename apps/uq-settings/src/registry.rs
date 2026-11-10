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

static CORE_PAGES: [PageInfo; 3] = [
    PageInfo {
        id: "appearance",
        name: "Appearance",
        icon: "preferences-desktop-theme",
        category: Category::Personal,
        qml: "pages/AppearancePage.qml",
        settings: &APPEARANCE_SETTINGS,
    },
    PageInfo {
        id: "look",
        name: "Look and layouts",
        icon: "preferences-desktop-effects",
        category: Category::Personal,
        qml: "pages/LookPage.qml",
        settings: &LOOK_SETTINGS,
    },
    PageInfo {
        id: "wallpaper",
        name: "Wallpaper",
        icon: "preferences-desktop-wallpaper",
        category: Category::Personal,
        qml: "pages/WallpaperPage.qml",
        settings: &WALLPAPER_SETTINGS,
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
/// file and whether it is a plugin page.
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
    fn pages_json_lists_pages_in_order() {
        let v: serde_json::Value = serde_json::from_str(&pages_json(&all_pages(&[]))).unwrap();
        let ids: Vec<_> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["appearance", "look", "wallpaper"]);
        assert_eq!(v[0]["category"], "personal");
        assert_eq!(v[0]["categoryName"], "Personal");
        assert_eq!(v[1]["qml"], "pages/LookPage.qml");
        assert_eq!(v[2]["qml"], "pages/WallpaperPage.qml");
        assert_eq!(v[2]["plugin"], false);
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
        assert_eq!(v.as_array().unwrap().len(), 4);
        assert_eq!(v[3]["id"], "demo");
        assert_eq!(v[3]["category"], "apps");
        assert_eq!(v[3]["categoryName"], "Apps");
        assert_eq!(v[3]["plugin"], true);
        assert_eq!(
            v[3]["qml"],
            "file:///usr/share/uq/settings/pages/demo/Page.qml"
        );
    }
}
