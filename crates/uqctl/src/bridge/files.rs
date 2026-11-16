//! Spotlight's files: an in-memory index of the user's files and folders, searched by name and
//! shown grouped by type (Folders, Documents, Images, Audio, Video, Code, Archives, Other files),
//! plus files elsewhere on the system through `plocate` when it is installed ("System files").
//!
//! The index covers the home folder and the drives mounted under `/mnt`, `/media` and
//! `/run/media/<user>`. Hidden files and folders, symbolic links and build or package folders
//! (`node_modules`, `target`, `__pycache__`, …) are left out, and it stops at [`MAX_ENTRIES`]. It
//! is built on a blocking thread, off the request path, when Spotlight opens and the last build
//! is older than [`STALE_AFTER`]; searches use the last finished build meanwhile. One search is a
//! linear pass over pre-lowercased names: a few milliseconds for 100 000 entries.

use super::calc::ProviderItem;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// At most this many files and folders are indexed.
pub const MAX_ENTRIES: usize = 400_000;
/// No deeper than this below a root.
const MAX_DEPTH: usize = 24;
/// A build older than this is redone when Spotlight next opens.
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

/// The types Spotlight groups files by, in the order it shows them.
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

    /// The provider id Spotlight groups by (`files:<kind>`).
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

/// Build the index over `roots` (`now`: for "recent"). Never follows symbolic links.
pub fn build(roots: &[PathBuf], now: SystemTime) -> Index {
    let mut idx = Index::default();
    let mut stack: Vec<(PathBuf, usize)> = roots.iter().map(|r| (r.clone(), 0)).collect();
    let recent = |m: &std::fs::Metadata| {
        m.modified()
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|d| d.as_secs() < RECENT_DAYS * 86_400)
    };
    while let Some((dir, depth)) = stack.pop() {
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
            let kind = if ft.is_dir() {
                if skip_dir(name) {
                    continue;
                }
                if depth + 1 < MAX_DEPTH {
                    stack.push((path.clone(), depth + 1));
                }
                Kind::Folder
            } else if ft.is_file() {
                Kind::of_file(name)
            } else {
                continue;
            };
            let recent = e.metadata().map(|m| recent(&m)).unwrap_or(false);
            idx.entries.push(Entry {
                path: p.into(),
                name: name.to_lowercase().into(),
                kind,
                recent,
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

/// How well a lowercased name matches a lowercased query: whole name 1.0, the name without its
/// extension 0.95, a prefix 0.85, a word start 0.7, anywhere 0.5; None for no match.
fn name_score(q: &str, name: &str) -> Option<f64> {
    if name == q {
        return Some(1.0);
    }
    if name.rsplit_once('.').is_some_and(|(stem, _)| stem == q) {
        return Some(0.95);
    }
    if name.starts_with(q) {
        return Some(0.85);
    }
    let at = name.find(q)?;
    let before = name[..at].chars().next_back();
    if before.is_some_and(|c| !c.is_alphanumeric()) {
        Some(0.7)
    } else {
        Some(0.5)
    }
}

/// The best [`PER_KIND`] rows of each type for `query`, as provider rows. Scores sit under an
/// app's name match: at most 0.8.
pub fn search(idx: &Index, query: &str) -> Vec<(Kind, Vec<ProviderItem>)> {
    let q = query.trim().to_lowercase();
    if q.chars().count() < 2 {
        return Vec::new();
    }
    let mut per: Vec<Vec<(f64, &Entry)>> = vec![Vec::new(); Kind::ALL.len()];
    for e in &idx.entries {
        let Some(s) = name_score(&q, &e.name) else {
            continue;
        };
        let s = s * 0.75 + if e.recent { 0.05 } else { 0.0 };
        let bucket = &mut per[e.kind as usize];
        // Keep the best PER_KIND, cheaply: a short sorted list.
        if bucket.len() < PER_KIND || bucket.last().is_some_and(|(b, _)| s > *b) {
            let at = bucket.partition_point(|(b, x)| *b > s || (*b == s && x.path < e.path));
            bucket.insert(at, (s, e));
            bucket.truncate(PER_KIND);
        }
    }
    Kind::ALL
        .iter()
        .zip(per)
        .filter(|(_, rows)| !rows.is_empty())
        .map(|(k, rows)| {
            (
                *k,
                rows.into_iter().map(|(s, e)| row(*k, &e.path, s)).collect(),
            )
        })
        .collect()
}

/// A provider row for a path: the name, then the folder it is in (with `~` for home).
pub fn row(kind: Kind, path: &str, score: f64) -> ProviderItem {
    let p = Path::new(path);
    let name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string();
    let parent = p
        .parent()
        .and_then(|d| d.to_str())
        .unwrap_or("")
        .to_string();
    let home = std::env::var("HOME").unwrap_or_default();
    let parent = match parent.strip_prefix(&home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => parent,
    };
    ProviderItem {
        id: path.to_string(),
        title: name,
        subtitle: parent,
        icon: kind.icon().into(),
        score,
        actions: Vec::new(),
    }
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

/// Files elsewhere on the system whose name holds `query`, from plocate's database: not under
/// `roots` (the index has those), not hidden. At most [`SYSTEM_ROWS`].
pub async fn system(query: &str, roots: &[PathBuf]) -> Vec<ProviderItem> {
    let q = query.trim();
    let Some(bin) = plocate() else {
        return Vec::new();
    };
    if q.chars().count() < 3 {
        return Vec::new();
    }
    let run = tokio::process::Command::new(bin)
        .args(["-i", "-b", "-l", &PLOCATE_LIMIT.to_string(), "--", q])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let Ok(Ok(out)) = tokio::time::timeout(PLOCATE_TIMEOUT, run).await else {
        return Vec::new();
    };
    let ql = q.to_lowercase();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with('/'))
        .filter(|l| !roots.iter().any(|r| Path::new(l).starts_with(r)))
        .filter(|l| !l.split('/').any(|seg| seg.starts_with('.')))
        .take(SYSTEM_ROWS)
        .map(|l| {
            let name = Path::new(l)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();
            let s = name_score(&ql, &name).unwrap_or(0.5) * 0.6;
            row(Kind::System, l, s)
        })
        .collect()
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
mod tests {
    use super::*;
    use std::fs;

    fn tree() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        for dir in [
            "Documents/Taxes",
            "Pictures",
            "Music",
            "proj/src",
            "proj/target/debug",
            "proj/node_modules/x",
            ".hidden",
        ] {
            fs::create_dir_all(r.join(dir)).unwrap();
        }
        for f in [
            "Documents/Taxes/report-2026.pdf",
            "Documents/notes.md",
            "Pictures/report.png",
            "Music/song.flac",
            "proj/src/main.rs",
            "proj/target/debug/report.bin",
            "proj/node_modules/x/report.js",
            ".hidden/report.txt",
            "report",
            "archive.tar",
        ] {
            fs::write(r.join(f), "x").unwrap();
        }
        std::os::unix::fs::symlink(r.join("Documents"), r.join("docs-link")).unwrap();
        d
    }

    #[test]
    fn kinds_from_extensions() {
        assert_eq!(Kind::of_file("a.PDF"), Kind::Document);
        assert_eq!(Kind::of_file("a.jpeg"), Kind::Image);
        assert_eq!(Kind::of_file("a.flac"), Kind::Audio);
        assert_eq!(Kind::of_file("a.mkv"), Kind::Video);
        assert_eq!(Kind::of_file("a.rs"), Kind::Code);
        assert_eq!(Kind::of_file("a.tar"), Kind::Archive);
        assert_eq!(Kind::of_file("Makefile"), Kind::Other);
        assert_eq!(Kind::of_file(".bashrc"), Kind::Other);
        assert_eq!(Kind::of_file("a.weird"), Kind::Other);
    }

    #[test]
    fn index_skips_hidden_build_folders_and_links() {
        let d = tree();
        let idx = build(&[d.path().to_path_buf()], SystemTime::now());
        let paths: Vec<&str> = idx.entries.iter().map(|e| &*e.path).collect();
        assert!(paths.iter().any(|p| p.ends_with("report-2026.pdf")));
        assert!(paths.iter().any(|p| p.ends_with("/Taxes")));
        for gone in ["target", "node_modules", ".hidden", "docs-link"] {
            assert!(!paths.iter().any(|p| p.contains(gone)), "{gone}: {paths:?}");
        }
        assert!(!idx.truncated);
    }

    #[test]
    fn search_groups_by_type_best_first() {
        let d = tree();
        let idx = build(&[d.path().to_path_buf()], SystemTime::now());
        let found = search(&idx, "Report");
        let kinds: Vec<Kind> = found.iter().map(|(k, _)| *k).collect();
        assert_eq!(kinds, vec![Kind::Document, Kind::Image, Kind::Other]);
        let other = &found[2].1;
        assert_eq!(other[0].title, "report");
        assert_eq!(found[1].1[0].title, "report.png");
        assert!(
            found
                .iter()
                .flat_map(|(_, r)| r)
                .all(|r| r.score <= 0.8 + 1e-9)
        );
        assert_eq!(search(&idx, "taxes")[0].0, Kind::Folder);
        assert!(search(&idx, "r").is_empty(), "one letter is too short");
        assert!(search(&idx, "zzzz").is_empty());
    }

    #[test]
    fn whole_names_rank_above_parts() {
        assert!(
            name_score("report", "report").unwrap() > name_score("report", "report.png").unwrap()
        );
        assert!(
            name_score("report", "report.png").unwrap()
                > name_score("report", "reports.txt").unwrap()
        );
        assert!(name_score("tax", "my-taxes").unwrap() > name_score("tax", "syntax").unwrap());
        assert_eq!(name_score("zz", "abc"), None);
    }

    #[test]
    fn per_kind_is_capped() {
        let d = tempfile::tempdir().unwrap();
        for i in 0..12 {
            fs::write(d.path().join(format!("img{i}.png")), "x").unwrap();
        }
        let idx = build(&[d.path().to_path_buf()], SystemTime::now());
        let found = search(&idx, "img");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1.len(), PER_KIND);
    }

    #[test]
    fn rows_show_home_as_tilde() {
        let home = std::env::var("HOME").unwrap();
        let r = row(Kind::Document, &format!("{home}/Documents/a.pdf"), 0.5);
        assert_eq!(r.title, "a.pdf");
        assert_eq!(r.subtitle, "~/Documents");
        assert_eq!(r.icon, "x-office-document");
    }

    /// Times a build and searches over the real home folder and drives:
    /// `cargo test -p uqctl --release -- --ignored files::tests::timing --nocapture`.
    #[test]
    #[ignore = "reads the real home folder"]
    fn timing() {
        let roots = default_roots();
        let t = Instant::now();
        let idx = build(&roots, SystemTime::now());
        println!(
            "indexed {} entries in {:?} (truncated: {})",
            idx.entries.len(),
            t.elapsed(),
            idx.truncated
        );
        for q in ["re", "report", "screenshot", "main.rs", "zzzzqq"] {
            let t = Instant::now();
            let n: usize = search(&idx, q).iter().map(|(_, r)| r.len()).sum();
            println!("search {q:?}: {n} rows in {:?}", t.elapsed());
        }
    }

    #[test]
    fn stale_after_five_minutes() {
        let mut f = Files::default();
        let t = Instant::now();
        assert!(f.wants_build(t));
        f.built = Some(t);
        assert!(!f.wants_build(t + Duration::from_secs(60)));
        assert!(f.wants_build(t + STALE_AFTER));
        f.building = true;
        assert!(!f.wants_build(t + STALE_AFTER));
    }
}
