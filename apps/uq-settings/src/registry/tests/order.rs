//! Page order, ids and categories.

use super::*;

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
            "sound",
            "bluetooth",
            "printers",
            "network",
            "shell",
            "profiles",
            "rules",
            "history",
            "restore",
            "keybinds",
            "startup",
            "power",
            "users",
            "login-screen",
            "datetime",
            "region",
            "default-apps",
            "problems",
            "about"
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
    assert_eq!(v[6]["qml"], "pages/SoundPage.qml");
    assert_eq!(v[6]["categoryName"], "Devices");
    assert_eq!(v[7]["qml"], "pages/BluetoothPage.qml");
    assert_eq!(v[7]["categoryName"], "Devices");
    assert_eq!(v[8]["qml"], "pages/PrintersPage.qml");
    assert_eq!(v[8]["categoryName"], "Devices");
    assert_eq!(v[9]["qml"], "pages/NetworkPage.qml");
    assert_eq!(v[9]["categoryName"], "Connections");
    assert_eq!(v[10]["qml"], "pages/ShellPage.qml");
    assert_eq!(v[10]["categoryName"], "uQ");
    assert_eq!(v[14]["qml"], "pages/RestorePage.qml");
    assert_eq!(v[14]["categoryName"], "uQ");
    assert_eq!(v[15]["qml"], "pages/KeybindsPage.qml");
    assert_eq!(v[15]["categoryName"], "uQ");
    assert_eq!(v[15]["domains"], serde_json::json!(["keybinds"]));
    assert_eq!(v[16]["qml"], "pages/StartupPage.qml");
    assert_eq!(v[16]["category"], "system");
    assert_eq!(v[16]["categoryName"], "System");
    assert_eq!(v[16]["domains"], serde_json::json!(["autostart", "env"]));
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
    let v: serde_json::Value = serde_json::from_str(&pages_json(&all_pages(&[plugin]))).unwrap();
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
fn system_pages_registered_after_startup() {
    let ids: Vec<_> = core_pages().iter().map(|p| p.id).collect();
    let at = ids.iter().position(|id| *id == "startup").unwrap();
    assert_eq!(ids[at + 1..at + 4], ["power", "users", "login-screen"]);
    assert_eq!(ids.last(), Some(&"about"), "About ends the System pages");
    for id in [
        "power",
        "users",
        "login-screen",
        "datetime",
        "region",
        "about",
    ] {
        let page = find(id).unwrap();
        assert_eq!(page.category, Category::System, "{id}");
        assert!(page.domains.is_empty(), "{id} changes no uQ setting");
    }
    let login = find("login-screen").unwrap();
    assert!(login.settings.iter().any(|s| s.id == "auto-login"));
}

#[test]
fn sound_follows_input() {
    let page = find("sound").expect("the sound page is registered");
    assert_eq!(page.category, Category::Devices);
    assert_eq!(page.qml, "pages/SoundPage.qml");
    assert!(page.domains.is_empty(), "it changes no uQ setting");
    let ids: Vec<_> = core_pages().iter().map(|p| p.id).collect();
    let at = ids.iter().position(|id| *id == "sound").unwrap();
    assert_eq!(ids[at - 1], "input");
    assert_eq!(ids[at + 1], "bluetooth");
    assert!(page.settings.iter().any(|s| s.id == "sound-output"));
}

#[test]
fn bluetooth_follows_sound() {
    let page = find("bluetooth").expect("the bluetooth page is registered");
    assert_eq!(page.category, Category::Devices);
    assert_eq!(page.qml, "pages/BluetoothPage.qml");
    assert!(page.domains.is_empty(), "it changes no uQ setting");
    let ids: Vec<_> = core_pages().iter().map(|p| p.id).collect();
    let at = ids.iter().position(|id| *id == "bluetooth").unwrap();
    assert_eq!(ids[at - 1], "sound");
    assert_eq!(ids[at + 1], "printers");
}

#[test]
fn printers_ends_the_devices_pages() {
    let page = find("printers").expect("the printers page is registered");
    assert_eq!(page.category, Category::Devices);
    assert_eq!(page.qml, "pages/PrintersPage.qml");
    assert!(page.domains.is_empty(), "it changes no uQ setting");
    let ids: Vec<_> = core_pages().iter().map(|p| p.id).collect();
    let at = ids.iter().position(|id| *id == "printers").unwrap();
    assert_eq!(ids[at + 1], "network");
    for id in [
        "printers-list",
        "printers-queue",
        "printers-test",
        "printers-add",
    ] {
        assert!(page.settings.iter().any(|s| s.id == id), "{id}");
    }
}

#[test]
fn network_is_the_connections_page() {
    let page = find("network").expect("the network page is registered");
    assert_eq!(page.category, Category::Connections);
    assert_eq!(page.qml, "pages/NetworkPage.qml");
    assert!(page.domains.is_empty(), "it changes no uQ setting");
    let ids: Vec<_> = core_pages().iter().map(|p| p.id).collect();
    let at = ids.iter().position(|id| *id == "network").unwrap();
    assert_eq!(ids[at + 1], "shell", "the last page before the uQ pages");
}

#[test]
fn login_screen_shown_only_with_the_greeter() {
    let mut pages = all_pages(&[]);
    drop_unavailable(&mut pages, true);
    assert!(pages.iter().any(|p| p.id == "login-screen"));
    drop_unavailable(&mut pages, false);
    assert!(!pages.iter().any(|p| p.id == "login-screen"));
    assert!(pages.iter().any(|p| p.id == "users"));
}
