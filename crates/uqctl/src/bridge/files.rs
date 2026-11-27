//! look=see>do_'s files: an in-memory index of the user's files and folders, searched by name and
//! shown grouped by type (Folders, Documents, Images, Audio, Video, Code, Archives, Other files),
//! plus files elsewhere on the system through `plocate` when it is installed ("System files").
//!
//! The index covers the home folder and the drives mounted under `/mnt`, `/media` and
//! `/run/media/<user>`. Hidden files and folders and build or package folders (`node_modules`,
//! `target`, `__pycache__`, …) are left out. Symbolic links are followed (`~/Documents` is often a
//! link to another drive), once per target folder, never into `/` or the system folders. The walk
//! goes level by level, so when it stops at [`MAX_ENTRIES`] the shallow files (the user's
//! Documents) are in and only the deepest are missing. It
//! is built on a blocking thread, off the request path, when look=see>do_ opens and the last build
//! is older than [`STALE_AFTER`]; searches use the last finished build meanwhile. One search is a
//! linear pass over pre-lowercased names: a few milliseconds for 100 000 entries.

use super::calc::ProviderItem;
use std::collections::{HashSet, VecDeque};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
mod search;
pub use search::*;

/// At most this many files and folders are indexed.
pub const MAX_ENTRIES: usize = 400_000;
/// No deeper than this below a root.
const MAX_DEPTH: usize = 24;
/// A build older than this is redone when look=see>do_ next opens.
pub const STALE_AFTER: Duration = Duration::from_secs(5 * 60);
/// Rows per type.
pub const PER_KIND: usize = 5;
/// plocate's rows and how long it may take.
const PLOCATE_LIMIT: usize = 40;
pub const PLOCATE_TIMEOUT: Duration = Duration::from_millis(300);
const SYSTEM_ROWS: usize = 6;
/// Folders that only hold what tools rebuild: never indexed.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "__pycache__",
    "site-packages",
    "venv",
    "build",
    "dist",
    "Cache",
    "cache",
    "CachedData",
    "lost+found",
];
/// Files and folders modified within this many days rank a little higher.
const RECENT_DAYS: u64 = 7;

/// The types look=see>do_ groups files by, in the order it shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Folder,
    Document,
    Image,
    Audio,
    Video,
    Code,
    Archive,
    Other,
    /// From plocate: outside the index.
    System,
}

impl Kind {
    pub const ALL: [Kind; 9] = [
        Kind::Folder,
        Kind::Document,
        Kind::Image,
        Kind::Audio,
        Kind::Video,
        Kind::Code,
        Kind::Archive,
        Kind::Other,
        Kind::System,
    ];

    /// The provider id look=see>do_ groups by (`files:<kind>`).
    pub fn provider(self) -> &'static str {
        match self {
            Kind::Folder => "files:folders",
            Kind::Document => "files:documents",
            Kind::Image => "files:images",
            Kind::Audio => "files:audio",
            Kind::Video => "files:video",
            Kind::Code => "files:code",
            Kind::Archive => "files:archives",
            Kind::Other => "files:other",
            Kind::System => "files:system",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Folder => "Folders",
            Kind::Document => "Documents",
            Kind::Image => "Images",
            Kind::Audio => "Audio",
            Kind::Video => "Video",
            Kind::Code => "Code",
            Kind::Archive => "Archives",
            Kind::Other => "Other files",
            Kind::System => "System files",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Kind::Folder => "folder",
            Kind::Document => "x-office-document",
            Kind::Image => "image-x-generic",
            Kind::Audio => "audio-x-generic",
            Kind::Video => "video-x-generic",
            Kind::Code => "text-x-script",
            Kind::Archive => "package-x-generic",
            Kind::Other | Kind::System => "text-x-generic",
        }
    }

    /// A file's type from its name's extension.
    pub fn of_file(name: &str) -> Kind {
        let ext = match name.rsplit_once('.') {
            Some((stem, e)) if !stem.is_empty() => e.to_ascii_lowercase(),
            _ => return Kind::Other,
        };
        match ext.as_str() {
            "pdf" | "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "epub" | "xls" | "xlsx"
            | "ods" | "csv" | "ppt" | "pptx" | "odp" | "tex" | "org" | "pages" | "numbers"
            | "key" => Kind::Document,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "tif" | "tiff" | "heic"
            | "heif" | "avif" | "ico" | "xcf" | "psd" | "kra" | "raw" | "cr2" | "nef" | "dng" => {
                Kind::Image
            }
            "mp3" | "flac" | "ogg" | "oga" | "opus" | "wav" | "m4a" | "aac" | "wma" | "aiff"
            | "mid" | "midi" => Kind::Audio,
            "mp4" | "mkv" | "webm" | "avi" | "mov" | "m4v" | "wmv" | "flv" | "mpg" | "mpeg"
            | "ogv" => Kind::Video,
            "rs" | "js" | "mjs" | "cjs" | "ts" | "mts" | "cts" | "jsx" | "tsx" | "qml" | "py"
            | "c" | "h" | "cc" | "cpp" | "hpp" | "go" | "java" | "kt" | "swift" | "sh" | "bash"
            | "zsh" | "fish" | "lua" | "rb" | "php" | "pl" | "toml" | "json" | "yaml" | "yml"
            | "html" | "htm" | "css" | "scss" | "xml" | "sql" | "cs" | "zig" | "nix" | "ini"
            | "conf" | "cmake" | "mk" => Kind::Code,
            "zip" | "tar" | "gz" | "tgz" | "xz" | "zst" | "bz2" | "7z" | "rar" | "iso" | "deb"
            | "rpm" | "appimage" | "pkg" => Kind::Archive,
            _ => Kind::Other,
        }
    }
}

/// One indexed file or folder.
#[derive(Debug, Clone)]
struct Entry {
    path: Box<str>,
    /// The name, lowercased, for matching.
    name: Box<str>,
    kind: Kind,
    recent: bool,
}

/// One finished build of the index.
#[derive(Debug, Default)]
pub struct Index {
    entries: Vec<Entry>,
    pub truncated: bool,
}

fn skip_dir(name: &str) -> bool {
    SKIP_DIRS.contains(&name)
}

/// Targets of symbolic links that are never followed: the system folders.
fn system_target(p: &Path) -> bool {
    p == Path::new("/")
        || ["/proc", "/sys", "/dev", "/run"]
            .iter()
            .any(|s| p.starts_with(s))
}

/// Build the index over `roots` (`now`: for "recent"), level by level. Follows symbolic links to
/// files and, once each, to folders (so a link back to a parent can't loop).
pub fn build(roots: &[PathBuf], now: SystemTime) -> Index {
    let mut idx = Index::default();
    let mut queue: VecDeque<(PathBuf, usize)> = roots.iter().map(|r| (r.clone(), 0)).collect();
    let mut seen: HashSet<(u64, u64)> = HashSet::new();
    for (r, _) in &queue {
        if let Ok(m) = std::fs::metadata(r) {
            seen.insert((m.dev(), m.ino()));
        }
    }
    let recent = |m: &std::fs::Metadata| {
        m.modified()
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|d| d.as_secs() < RECENT_DAYS * 86_400)
    };
    while let Some((dir, depth)) = queue.pop_front() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            if idx.entries.len() >= MAX_ENTRIES {
                idx.truncated = true;
                return idx;
            }
            let name = e.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with('.') {
                continue;
            }
            let Ok(ft) = e.file_type() else { continue };
            let path = e.path();
            let Some(p) = path.to_str() else { continue };
            // What the entry is, looking through a link; `meta` is that of the target.
            let (is_dir, is_file, meta) = if ft.is_symlink() {
                let Ok(m) = std::fs::metadata(&path) else {
                    continue; // a dangling link
                };
                (m.is_dir(), m.is_file(), m)
            } else {
                let Ok(m) = e.metadata() else { continue };
                (ft.is_dir(), ft.is_file(), m)
            };
            let kind = if is_dir {
                if skip_dir(name) {
                    continue;
                }
                let new_dir = seen.insert((meta.dev(), meta.ino()));
                if ft.is_symlink() {
                    let outside = std::fs::canonicalize(&path).is_ok_and(|t| system_target(&t));
                    if !new_dir || outside {
                        continue;
                    }
                }
                if depth + 1 < MAX_DEPTH && new_dir {
                    queue.push_back((path.clone(), depth + 1));
                }
                Kind::Folder
            } else if is_file {
                Kind::of_file(name)
            } else {
                continue;
            };
            idx.entries.push(Entry {
                path: p.into(),
                name: name.to_lowercase().into(),
                kind,
                recent: recent(&meta),
            });
        }
    }
    idx
}

/// The roots to index: the home folder and each drive mounted under `/mnt`, `/media` or
/// `/run/media/<user>`. `UQ_SEARCH_ROOTS` (folders separated by `:`, empty for none) replaces
/// them.
pub fn default_roots() -> Vec<PathBuf> {
    if let Some(list) = std::env::var_os("UQ_SEARCH_ROOTS") {
        return std::env::split_paths(&list)
            .filter(|p| p.is_absolute())
            .collect();
    }
    let mut roots = Vec::new();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(h) = &home {
        roots.push(h.clone());
    }
    let user = std::env::var("USER").unwrap_or_default();
    let mut parents = vec![PathBuf::from("/mnt"), PathBuf::from("/media")];
    if !user.is_empty() {
        parents.push(Path::new("/run/media").join(&user));
    }
    for parent in parents {
        let Ok(rd) = std::fs::read_dir(&parent) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let inside_home = home.as_ref().is_some_and(|h| p.starts_with(h));
            if e.file_type().is_ok_and(|t| t.is_dir()) && !inside_home {
                roots.push(p);
            }
        }
    }
    roots
}

/// Whether plocate is installed (looked up once).
pub fn plocate() -> Option<&'static Path> {
    static FOUND: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    FOUND.get_or_init(|| on_path("plocate")).as_deref()
}

/// Where `bin` is on PATH.
pub fn on_path(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(bin))
            .find(|p| p.is_file())
    })
}

/// Plain-text documents: read as they are, unlike PDFs and office files.
const TEXT_DOCS: &[&str] = &["txt", "md", "csv", "tex", "org", "rtf"];

/// Whether a file's text can be read as it is: code and plain-text documents.
pub fn is_text(name: &str) -> bool {
    match Kind::of_file(name) {
        Kind::Code => true,
        Kind::Document => name
            .rsplit_once('.')
            .is_some_and(|(_, e)| TEXT_DOCS.contains(&e.to_ascii_lowercase().as_str())),
        _ => false,
    }
}

impl Index {
    /// The indexed files whose text can be read as it is, the recently changed first.
    pub fn text_files(&self) -> impl Iterator<Item = &str> {
        let text = |e: &&Entry| e.kind != Kind::Folder && is_text(&e.name);
        self.entries
            .iter()
            .filter(|e| e.recent)
            .filter(text)
            .chain(self.entries.iter().filter(|e| !e.recent).filter(text))
            .map(|e| &*e.path)
    }
}

/// The index and when it was last built; shared with the build task.
#[derive(Default)]
pub struct Files {
    pub index: Arc<Index>,
    pub built: Option<Instant>,
    pub building: bool,
    pub roots: Arc<Vec<PathBuf>>,
}

impl Files {
    /// Whether a build should start now (none running, and the last is missing or stale).
    pub fn wants_build(&self, now: Instant) -> bool {
        !self.building
            && self
                .built
                .is_none_or(|t| now.saturating_duration_since(t) >= STALE_AFTER)
    }
}

#[cfg(test)]
mod tests;
