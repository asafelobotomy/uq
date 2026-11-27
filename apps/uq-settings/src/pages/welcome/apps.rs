//! Welcome's Apps step (Phase 5 spec §5.1 step 6): which apps made for uQ are installed, and
//! for each missing one a line on what it does and where it comes from.
//!
//! An app counts as installed when uQ found one of its integrations (`uqctl integrations
//! list`: a widget, settings page, launcher provider or app action with its id), or when one of
//! its programs is in `/usr/bin`. The descriptions and package names follow bindle's catalogue.

use std::path::Path;

use serde_json::{Value, json};
use uq_core::integration::scan::Entry;

#[derive(Debug, Clone, Copy)]
pub struct App {
    /// bindle's id for it.
    pub id: &'static str,
    pub name: &'static str,
    /// One plain line on what it adds to uQ.
    pub description: &'static str,
    /// Its AUR package (the one Bindle installs; uQ's compat/family.toml lists them all).
    pub package: &'static str,
    /// Integration ids (folder or file ids under /usr/share/uq) that show it is installed.
    pub integrations: &'static [&'static str],
    /// Programs in /usr/bin that show it is installed.
    pub bins: &'static [&'static str],
}

pub const APPS: [App; 16] = [
    App {
        id: "noti-fi",
        name: "noti-fi",
        description: "Notifications with grouped pop-ups, rules, history and Do Not Disturb.",
        package: "noti-fi",
        integrations: &["noti-fi", "notifi"],
        bins: &["notifd"],
    },
    App {
        id: "scdr",
        name: "scdr",
        description: "Keeps the app you're using responsive and slows down background work.",
        package: "scdr",
        integrations: &["scdr"],
        bins: &["scdr", "scdrctl"],
    },
    App {
        id: "crumbs",
        name: "crumBS",
        description: "Uninstalls apps with their leftovers and cleans up old files.",
        package: "crum-bs-bin",
        integrations: &["crumbs"],
        bins: &["crumbs"],
    },
    App {
        id: "scale_tail",
        name: "scale_tail",
        description: "Tailscale in quick settings: connect and switch exit nodes.",
        package: "scale_tail",
        integrations: &["scale_tail", "scale-tail"],
        bins: &["scale_tail"],
    },
    App {
        id: "xzram",
        name: "XZram",
        description: "A Memory and swap page for zram and swap files.",
        package: "xzram",
        integrations: &["xzram"],
        bins: &["xzram"],
    },
    App {
        id: "appofholding",
        name: "appOFholding",
        description: "AppImages in the app menu, kept up to date.",
        package: "appofholding",
        integrations: &["appofholding"],
        bins: &["appofholding"],
    },
    App {
        id: "hauler",
        name: "hauler",
        description: "Fast, resumable downloads, with progress in the bar.",
        package: "hauler",
        integrations: &["hauler"],
        bins: &["hauler"],
    },
    App {
        id: "emobie",
        name: "emobie",
        description: "Emoji in the launcher and on a hotkey.",
        package: "emobie",
        integrations: &["emobie"],
        bins: &["emobie"],
    },
    App {
        id: "oysterav",
        name: "oysterAV",
        description: "Security status in quick settings, and scans.",
        package: "oysterav",
        integrations: &["oysterav"],
        bins: &["oyst-cli", "oysterav"],
    },
    App {
        id: "romforge",
        name: "romforge",
        description: "Your games in the launcher.",
        package: "romforge-bin",
        integrations: &["romforge"],
        bins: &["romforge"],
    },
    App {
        id: "kyko",
        name: "kyko",
        description: "uQ's terminal, with a drop-down one on F12.",
        package: "kyko",
        integrations: &["kyko"],
        bins: &["kyko"],
    },
    App {
        id: "uqview",
        name: "uQview",
        description: "Opens pictures in uQ's theme and marks up screenshots.",
        package: "uqview",
        integrations: &["uqview"],
        bins: &["uqview"],
    },
    App {
        id: "reelo",
        name: "reelo",
        description: "Plays video and music in uQ's theme, with uQ's media controls.",
        package: "reelo",
        integrations: &["reelo"],
        bins: &["reelo"],
    },
    App {
        id: "uqbit",
        name: "uQbit",
        description: "Downloads torrents in the background, only through your VPN if you ask.",
        package: "uqbit",
        integrations: &["uqbit"],
        bins: &["uqbit"],
    },
    App {
        id: "bindaru",
        name: "bindaru",
        description: "Finds, installs and removes software from the repositories and the AUR.",
        package: "bindaru",
        integrations: &["software", "bindaru"],
        bins: &["bindaru"],
    },
    App {
        id: "av-morning-star",
        name: "AV Morning Star",
        description: "Downloads video and audio from YouTube and many other sites.",
        package: "av-morning-star",
        integrations: &[],
        bins: &["av-morning-star"],
    },
];

/// bindle's program in the programs folder, for "Install with bindle".
pub const BINDLE: &str = "bindle";

/// `id` names one of `app`'s integrations, ignoring case and `-`/`_`. A reverse-DNS id
/// (`org.example.noti-fi`) matches on its last part.
fn matches(app: &App, id: &str) -> bool {
    let norm = |s: &str| s.to_ascii_lowercase().replace('_', "-");
    let id = norm(id);
    let last = id.rsplit('.').next().unwrap_or(&id).to_string();
    app.integrations
        .iter()
        .any(|i| norm(i) == id || norm(i) == last)
}

/// Whether `app` is installed, from the integrations uQ found and the programs on disk.
pub fn installed(app: &App, entries: &[Entry], bin_exists: &dyn Fn(&str) -> bool) -> bool {
    entries.iter().any(|e| e.valid && matches(app, &e.id)) || app.bins.iter().any(|b| bin_exists(b))
}

/// `[{id, name, description, installed, package}]`, installed apps first.
pub fn list(entries: &[Entry], bin_exists: &dyn Fn(&str) -> bool) -> Vec<Value> {
    let mut rows: Vec<(bool, Value)> = APPS
        .iter()
        .map(|a| {
            let is = installed(a, entries, bin_exists);
            let mut row = json!({
                "id": a.id,
                "name": a.name,
                "description": a.description,
                "installed": is,
            });
            row["package"] = json!(a.package);
            (is, row)
        })
        .collect();
    rows.sort_by_key(|(is, _)| !is);
    rows.into_iter().map(|(_, r)| r).collect()
}

/// A program in `/usr/bin` (or `dir` in tests).
pub fn bin_in(dir: &Path) -> impl Fn(&str) -> bool + '_ {
    move |name: &str| dir.join(name).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uq_core::integration::manifest::Bus;
    use uq_core::integration::scan::Kind;

    fn entry(id: &str, valid: bool) -> Entry {
        Entry {
            kind: Kind::Page,
            id: id.into(),
            name: None,
            source: PathBuf::from("/usr/share/uq/settings/pages").join(id),
            valid,
            visible: valid,
            error: None,
            waits_for: None,
            bus: Bus::Session,
        }
    }

    fn none(_: &str) -> bool {
        false
    }

    #[test]
    fn an_integration_or_a_program_counts_as_installed() {
        let noti = &APPS[0];
        assert!(!installed(noti, &[], &none));
        assert!(installed(noti, &[entry("noti-fi", true)], &none));
        assert!(installed(
            noti,
            &[entry("org.example.Noti_Fi", true)],
            &none
        ));
        assert!(
            !installed(noti, &[entry("noti-fi", false)], &none),
            "a refused manifest doesn't count"
        );
        assert!(installed(noti, &[], &|b| b == "notifd"));
        assert!(!installed(noti, &[entry("scdr", true)], &none));
    }

    #[test]
    fn list_puts_installed_first_and_says_where_missing_ones_come_from() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("crumbs"), "").unwrap();
        let rows = list(&[entry("scdr", true)], &bin_in(dir.path()));
        assert_eq!(rows.len(), APPS.len());
        let ids: Vec<_> = rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(&ids[..2], ["scdr", "crumbs"]);
        assert!(rows[..2].iter().all(|r| r["installed"] == true));
        assert!(rows[2..].iter().all(|r| r["installed"] == false));
        let noti = rows.iter().find(|r| r["id"] == "noti-fi").unwrap();
        assert_eq!(noti["package"], "noti-fi");
        let hauler = rows.iter().find(|r| r["id"] == "hauler").unwrap();
        assert_eq!(hauler["package"], "hauler");
    }

    #[test]
    fn every_app_is_described() {
        let mut ids = std::collections::BTreeSet::new();
        for a in APPS {
            assert!(ids.insert(a.id), "{} twice", a.id);
            assert!(
                !a.description.is_empty() && a.description.len() <= 80,
                "{}",
                a.id
            );
            assert!(a.description.ends_with('.'), "{}", a.id);
            assert!(!a.integrations.is_empty() || !a.bins.is_empty(), "{}", a.id);
        }
    }

    #[test]
    fn every_app_is_an_aur_package_of_the_family() {
        // The family as uQ's compat/family.toml lists it: every app (all but uQ and Bindle,
        // which Welcome names on its own) is here, under one of its AUR packages.
        let family: toml::Table = include_str!("../../../../../compat/family.toml")
            .parse()
            .unwrap();
        let packages = family["package"].as_table().unwrap();
        for a in APPS {
            let p = packages
                .get(a.id)
                .unwrap_or_else(|| panic!("{} isn't in family.toml", a.id));
            let aur: Vec<&str> = p["aur"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            assert!(
                aur.contains(&a.package),
                "{}: {} isn't one of {aur:?}",
                a.id,
                a.package
            );
        }
        for name in packages
            .keys()
            .filter(|n| !["uq", "bindle"].contains(&n.as_str()))
        {
            assert!(
                APPS.iter().any(|a| a.id == name),
                "{name} is missing from Welcome's apps"
            );
        }
    }
}
