//! Each page is registered with its search entries.

use super::*;

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
    assert_eq!(page.domains, ["rules", "integration_rules"]);
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
            (
                "rules-from-apps",
                "Rules from apps",
                &["game", "emulator", "fullscreen", "installed", "integration"][..]
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
fn keybinds_page_registered_with_search_entries() {
    let page = find("keybinds").expect("the keyboard shortcuts page is registered");
    assert_eq!(page.name, "Keyboard shortcuts");
    assert_eq!(page.category, Category::Uq);
    assert_eq!(page.icon, "preferences-desktop-keyboard-shortcuts");
    assert_eq!(page.qml, "pages/KeybindsPage.qml");
    assert_eq!(page.domains, ["keybinds"]);
    let entries: Vec<_> = page
        .settings
        .iter()
        .map(|s| (s.id, s.label, s.keywords))
        .collect();
    assert_eq!(
        entries,
        [
            (
                "shortcut-list",
                "Keyboard shortcuts",
                &["keys", "keybinds", "hotkeys", "bind"][..]
            ),
            (
                "add-shortcut",
                "Add a shortcut",
                &["custom", "command", "launch"][..]
            ),
            (
                "reset-shortcuts",
                "Put shortcuts back",
                &["reset", "default"][..]
            ),
            (
                "next-profile-shortcut",
                "Switch to the next profile",
                &["profile", "switch"][..]
            ),
            (
                "shortcut-capture",
                "Press keys to set a shortcut",
                &["record", "capture"][..]
            ),
        ]
    );
    let ids: Vec<_> = core_pages().iter().map(|p| p.id).collect();
    let at = ids.iter().position(|id| *id == "keybinds").unwrap();
    assert_eq!(
        (ids[at - 1], ids[at + 1]),
        ("history", "startup"),
        "after History, the last uQ page"
    );
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
                "start-stagger",
                "Space out the starts",
                &["stagger", "gap", "one at a time", "order"][..]
            ),
            (
                "start-retry",
                "Try again if an app fails",
                &["retry", "again", "fail", "crash"][..]
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
    assert_eq!(pages[first_system - 1].id, "keybinds", "after the uQ pages");
}
