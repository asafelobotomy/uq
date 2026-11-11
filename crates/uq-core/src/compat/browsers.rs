//! Chromium-based browsers: the flags file each reads, and the command-line spellings uQ adds.
//!
//! Not Hyprland names, so there is no Hyprland source to cite. The flags-file names follow the
//! Arch packages' launcher scripts and are a live-check item, as is whether Chromium 140 and newer
//! still reads `--ozone-platform-hint` (see the plan's "Not confirmed from source").

use crate::desktop_entry::normalise_id;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Browser {
    pub name: &'static str,
    pub desktop_ids: &'static [&'static str],
    /// The file name in `$XDG_CONFIG_HOME`.
    pub flags_file: &'static str,
}

pub const BROWSERS: [Browser; 5] = [
    Browser {
        name: "Chromium",
        desktop_ids: &["chromium.desktop"],
        flags_file: "chromium-flags.conf",
    },
    Browser {
        name: "Google Chrome",
        desktop_ids: &["google-chrome.desktop"],
        flags_file: "chrome-flags.conf",
    },
    Browser {
        name: "Brave",
        desktop_ids: &["brave-browser.desktop"],
        flags_file: "brave-flags.conf",
    },
    Browser {
        name: "Vivaldi",
        desktop_ids: &["vivaldi-stable.desktop"],
        flags_file: "vivaldi-stable.conf",
    },
    Browser {
        name: "Microsoft Edge",
        desktop_ids: &["microsoft-edge.desktop"],
        flags_file: "microsoft-edge-stable-flags.conf",
    },
];

/// The first line of every flags file uQ writes; uQ changes only files that start with it.
pub const FLAGS_MARKER: &str =
    "# Managed by uQ (Apps and scaling). uQ only changes files that start with this line.";

/// The flag "Run Chromium-based browsers on Wayland" writes.
pub const FLAGS_LINE: &str = "--ozone-platform-hint=auto";

/// A per-app fix's "Run on" for a browser: this prefix, then `wayland` or `x11`.
pub const PLATFORM_FLAG: &str = "--ozone-platform=";

/// A per-app fix's size for a browser: this prefix, then the scale.
pub const SCALE_FLAG: &str = "--force-device-scale-factor=";

/// The browser a desktop id belongs to (the id may lack `.desktop`).
pub fn browser_for(id: &str) -> Option<&'static Browser> {
    let id = normalise_id(id);
    BROWSERS
        .iter()
        .find(|b| b.desktop_ids.contains(&id.as_str()))
}

/// The whole text of a flags file uQ writes.
pub fn flags_text() -> String {
    format!("{FLAGS_MARKER}\n{FLAGS_LINE}\n")
}
