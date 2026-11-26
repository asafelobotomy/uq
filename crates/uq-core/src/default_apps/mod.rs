//! Default apps (spec §2.6): the kinds of files and links the Default apps page offers, the
//! installed apps that open them, uQ's own apps for each kind, and the edits to `mimeapps.list`
//! that set or clear a choice. Shared by the Settings page and uqd, which gives uQ's own apps
//! the kinds a person never chose, once each, at login.

pub mod edit;
pub mod fill;
pub mod handlers;
pub mod info;

pub use edit::{remove_defaults, set_defaults};
pub use handlers::{Handler, scan_handlers};

/// One kind of file or link: its id, its label, the types it covers and, for pictures, videos
/// and music, the family every type of which an app opens is given to it too. The first type
/// decides which apps are offered.
pub struct Kind {
    pub id: &'static str,
    pub label: &'static str,
    pub mimes: &'static [&'static str],
    /// `image/`: an app chosen for pictures also gets every other `image/…` type it opens.
    pub family: Option<&'static str>,
    /// uQ's own apps for this kind, best first, by desktop id.
    pub native: &'static [&'static str],
}

pub const UQVIEW: &str = "io.github.asafelobotomy.uqview.desktop";
pub const REELO: &str = "io.github.asafelobotomy.reelo.desktop";
pub const UQBIT: &str = "io.github.asafelobotomy.uqbit.desktop";
pub const KYKO: &str = "org.kyko.Kyko.desktop";

/// Every desktop id of uQ's own apps, for the page's "uQ app" mark.
pub const NATIVE_APPS: [&str; 4] = [UQVIEW, REELO, UQBIT, KYKO];

pub const KINDS: [Kind; 11] = [
    Kind {
        id: "browser",
        label: "Web browser",
        mimes: &[
            "x-scheme-handler/http",
            "x-scheme-handler/https",
            "text/html",
            "application/xhtml+xml",
        ],
        family: None,
        native: &[],
    },
    Kind {
        id: "mail",
        label: "Email",
        mimes: &["x-scheme-handler/mailto"],
        family: None,
        native: &[],
    },
    Kind {
        id: "calendar",
        label: "Calendar",
        mimes: &["text/calendar", "x-scheme-handler/webcal"],
        family: None,
        native: &[],
    },
    Kind {
        id: "files",
        label: "Files",
        mimes: &["inode/directory"],
        family: None,
        native: &[],
    },
    Kind {
        id: "text",
        label: "Text editor",
        mimes: &["text/plain"],
        family: None,
        native: &[],
    },
    Kind {
        id: "images",
        label: "Pictures",
        mimes: &[
            "image/png",
            "image/jpeg",
            "image/gif",
            "image/webp",
            "image/bmp",
            "image/tiff",
            "image/svg+xml",
        ],
        family: Some("image/"),
        native: &[UQVIEW],
    },
    Kind {
        id: "video",
        label: "Videos",
        mimes: &[
            "video/mp4",
            "video/x-matroska",
            "video/webm",
            "video/mpeg",
            "video/quicktime",
            "video/x-msvideo",
        ],
        family: Some("video/"),
        native: &[REELO],
    },
    Kind {
        id: "music",
        label: "Music",
        mimes: &[
            "audio/mpeg",
            "audio/flac",
            "audio/ogg",
            "audio/x-vorbis+ogg",
            "audio/x-wav",
            "audio/mp4",
            "audio/aac",
        ],
        family: Some("audio/"),
        native: &[REELO],
    },
    Kind {
        id: "pdf",
        label: "PDF documents",
        mimes: &["application/pdf"],
        family: None,
        native: &[],
    },
    Kind {
        id: "archives",
        label: "Archives",
        mimes: &[
            "application/zip",
            "application/x-7z-compressed",
            "application/x-tar",
            "application/x-compressed-tar",
            "application/x-xz-compressed-tar",
            "application/x-bzip2-compressed-tar",
            "application/x-zstd-compressed-tar",
            "application/vnd.rar",
            "application/x-rar",
        ],
        family: None,
        native: &[],
    },
    Kind {
        id: "torrents",
        label: "Torrents",
        mimes: &["application/x-bittorrent", "x-scheme-handler/magnet"],
        family: None,
        native: &[UQBIT],
    },
];

/// The desktop's own lists in `config_home`, which win over `mimeapps.list`:
/// `hyprland-mimeapps.list` for `XDG_CURRENT_DESKTOP=Hyprland`.
pub fn desktop_files(config_home: &std::path::Path, desktops: &str) -> Vec<std::path::PathBuf> {
    desktops
        .split(':')
        .map(str::trim)
        .filter(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .map(|d| config_home.join(format!("{}-mimeapps.list", d.to_lowercase())))
        .collect()
}

pub fn kind(id: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|k| k.id == id)
}

impl Kind {
    /// The type that decides which apps are offered and what the page shows as the choice.
    pub fn primary(&self) -> &'static str {
        self.mimes[0]
    }

    /// Whether `h` can be chosen for this kind.
    pub fn offers(&self, h: &Handler) -> bool {
        h.opens(self.primary())
    }

    /// The types choosing `h` sets: the kind's own types the app says it opens, then every
    /// other type of the kind's family it opens. A picture viewer without GIF support never
    /// becomes GIF's default, and one that opens AVIF gets AVIF too.
    pub fn mimes_for(&self, h: &Handler) -> Vec<String> {
        let mut out: Vec<String> = self
            .mimes
            .iter()
            .filter(|m| h.opens(m))
            .map(|m| m.to_string())
            .collect();
        if let Some(fam) = self.family {
            for m in &h.mimes {
                if m.starts_with(fam) && !out.contains(m) {
                    out.push(m.clone());
                }
            }
        }
        out
    }

    /// Every type the page clears to go back to the system's choice: the kind's own, and the
    /// family's types any of `apps` was given.
    pub fn mimes_to_clear(&self, apps: &[Handler]) -> Vec<String> {
        let mut out: Vec<String> = self.mimes.iter().map(|m| m.to_string()).collect();
        if let Some(fam) = self.family {
            for m in apps.iter().flat_map(|h| &h.mimes) {
                if m.starts_with(fam) && !out.contains(m) {
                    out.push(m.clone());
                }
            }
        }
        out
    }

    /// uQ's first own app for this kind that is installed and opens it.
    pub fn native_in<'a>(&self, apps: &'a [Handler]) -> Option<&'a Handler> {
        self.native
            .iter()
            .find_map(|id| apps.iter().find(|h| h.id == *id && self.offers(h)))
    }
}

#[cfg(test)]
mod tests;
