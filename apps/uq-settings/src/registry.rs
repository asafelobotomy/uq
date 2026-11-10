//! The core pages: their ids (the `ShowPage` page ids), names, icons, categories and search
//! entries. Plugin pages (Task 8) join them at runtime.

use serde::Serialize;

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
    #[allow(dead_code, reason = "used by the plugin scan and search (Task 8)")]
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

static CORE_PAGES: [PageInfo; 2] = [
    PageInfo {
        id: "appearance",
        name: "Appearance",
        icon: "preferences-desktop-theme",
        category: Category::Personal,
        qml: "pages/AppearancePage.qml",
        settings: &[],
    },
    PageInfo {
        id: "wallpaper",
        name: "Wallpaper",
        icon: "preferences-desktop-wallpaper",
        category: Category::Personal,
        qml: "pages/WallpaperPage.qml",
        settings: &[],
    },
];

/// The core pages, sorted by category.
pub fn core_pages() -> &'static [PageInfo] {
    &CORE_PAGES
}

pub fn find(id: &str) -> Option<&'static PageInfo> {
    core_pages().iter().find(|p| p.id == id)
}

/// The sidebar model: every page in category order, each with its category's heading and
/// its QML file.
pub fn pages_json() -> String {
    let pages: Vec<_> = core_pages()
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "icon": p.icon,
                "category": p.category,
                "categoryName": p.category.label(),
                "qml": p.qml,
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
        assert!(ids.contains("appearance") && ids.contains("wallpaper"));
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
        let v: serde_json::Value = serde_json::from_str(&pages_json()).unwrap();
        let ids: Vec<_> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["appearance", "wallpaper"]);
        assert_eq!(v[0]["category"], "personal");
        assert_eq!(v[0]["categoryName"], "Personal");
        assert_eq!(v[1]["qml"], "pages/WallpaperPage.qml");
    }
}
