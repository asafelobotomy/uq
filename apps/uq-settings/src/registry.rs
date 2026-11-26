//! The core pages: their ids (the `ShowPage` page ids), names, icons, categories and search
//! entries. Plugin pages (Task 8) join them at runtime.

use serde::Serialize;

use crate::plugins::PluginPage;
mod entries_personal;
use entries_personal::*;
mod entries_uq;
use entries_uq::*;
mod entries_devices;
use entries_devices::*;
mod entries_system;
use entries_system::*;
mod core_pages;
use core_pages::*;

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

/// Pages shown only on some machines: the Login screen page needs uQ's login screen
/// (`/etc/uq/greetd.toml`). The others are always shown.
pub fn drop_unavailable(pages: &mut Vec<PageEntry>, greeter_installed: bool) {
    if !greeter_installed {
        pages.retain(|p| p.id != "login-screen");
    }
}

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
/// Every page and setting as a flat JSON list for look=see>do_ (`uq-settings --search-index`):
/// `[{page, setting, label, pageName, icon, keywords}]`, a page's own row with `setting` "".
pub fn search_index_json(pages: &[PageEntry]) -> String {
    let mut rows = Vec::new();
    for p in pages {
        rows.push(serde_json::json!({
            "page": p.id, "setting": "", "label": p.name, "pageName": p.name,
            "icon": p.icon, "keywords": p.keywords,
        }));
        for s in &p.settings {
            rows.push(serde_json::json!({
                "page": p.id, "setting": s.id, "label": s.label, "pageName": p.name,
                "icon": p.icon, "keywords": s.keywords,
            }));
        }
    }
    serde_json::Value::Array(rows).to_string()
}

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
mod tests;
