//! User themes: the rules every user theme passes, the safe scan of the user's theme folder and
//! the writer for a variant file.
//!
//! Everything under the user's themes folder, and every `.uqtheme`, is untrusted. A theme that
//! breaks a rule never loads and never stops anything else from loading; the scan says why it
//! was skipped, in plain words.

use super::{Theme, Variant, derive};
use crate::autostart::scan::read_capped_if;
use crate::contrast;
use crate::settings::valid_theme_name;
use serde::Serialize;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Every user theme's id starts with this, so none can shadow a system theme.
pub const PREFIX: &str = "user-";
pub const MAX_USER_THEMES: usize = 50;
/// The most folder entries looked at in one scan.
pub const MAX_SCANNED: usize = 200;
pub const MAX_FILE_BYTES: u64 = 65_536;
const _: () = assert!(MAX_FILE_BYTES == crate::autostart::scan::MAX_BYTES);
/// The longest theme name or author.
pub const MAX_TEXT_CHARS: usize = 64;
const MAX_SLUG_CHARS: usize = 40;
const MAX_FONT_CHARS: usize = 100;
/// The most directory entries collected before sorting.
const MAX_LISTED: usize = 4096;

/// `[a-z0-9]([a-z0-9-]{0,38}[a-z0-9])?`: 1 to 40 characters.
pub fn valid_slug(s: &str) -> bool {
    let b = s.as_bytes();
    let edge = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    (1..=MAX_SLUG_CHARS).contains(&b.len())
        && edge(b[0])
        && edge(b[b.len() - 1])
        && b.iter().all(|&c| edge(c) || c == b'-')
}

/// `user-` and a valid slug.
pub fn valid_user_id(id: &str) -> bool {
    id.strip_prefix(PREFIX).is_some_and(valid_slug)
}

/// A slug for a new theme called `name`: its lower-case ASCII words joined by `-`, cut to 40,
/// with `-2`, `-3` and so on when `taken` has it, and `theme` when no word is left. The caller
/// adds [`PREFIX`].
pub fn slug_for(name: &str, taken: &[String]) -> String {
    let words: Vec<String> = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let base = if words.is_empty() {
        "theme".to_string()
    } else {
        words.join("-")
    };
    let cut = |s: &str, n: usize| {
        s.chars()
            .take(n)
            .collect::<String>()
            .trim_end_matches('-')
            .to_string()
    };
    let first = cut(&base, MAX_SLUG_CHARS);
    if !taken.contains(&first) {
        return first;
    }
    (2..)
        .map(|n| {
            let suffix = format!("-{n}");
            format!("{}{suffix}", cut(&base, MAX_SLUG_CHARS - suffix.len()))
        })
        .find(|c| !taken.contains(c))
        .expect("an unbounded range has a free slug")
}

/// A theme name or author that is safe to print inside a config file's comment (G-F1): 1 to
/// 64 characters and trimmed; no control characters, no invisible or direction-changing format
/// characters, no `*/` (it would end a CSS comment) and no `{{`, `{%` or `{#` (template
/// syntax).
pub fn valid_display_text(s: &str) -> bool {
    let n = s.chars().count();
    (1..=MAX_TEXT_CHARS).contains(&n)
        && s == s.trim()
        && !s.chars().any(|c| c.is_control() || invisible(c))
        && !["*/", "{{", "{%", "{#"].iter().any(|bad| s.contains(bad))
}

fn invisible(c: char) -> bool {
    matches!(c, '\u{061C}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2069}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{00AD}'
            | '\u{180E}'
            | '\u{FEFF}'
            | '\u{E0000}'..='\u{E007F}')
}

/// Why a user theme can't be used, as one plain sentence.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Problem {
    #[error("Its folder name must start with user- and use only a–z, 0–9 and dashes.")]
    FolderName,
    #[error("It is a link, and uQ doesn't read through links.")]
    Link,
    #[error("It isn't a folder.")]
    NotAFolder,
    #[error("The {0} version is missing.")]
    MissingVariant(Variant),
    #[error("The {0} version can't be read, or is too big.")]
    Unreadable(Variant),
    #[error("The {0} version isn't a theme file uQ understands.")]
    BadFile(Variant),
    #[error("The dark and light versions have different names.")]
    NamesDiffer,
    #[error("A theme needs a name of 1 to 64 characters, without special characters.")]
    Name,
    #[error("The author's name can't be used.")]
    Author,
    #[error("A font name must be 1 to 100 characters, without special characters.")]
    Font,
    #[error("An icon or cursor theme name can only use letters, digits and . _ + - and spaces.")]
    IconName,
    #[error("The {0} is out of range.")]
    OutOfRange(&'static str),
    #[error("{text}")]
    HardToRead { variant: Variant, text: String },
    #[error(
        "The {0} version has the wrong kind of background: dark needs a darker background than its text, light a lighter one."
    )]
    WrongBrightness(Variant),
    #[error("Only {MAX_USER_THEMES} themes of your own can be used at once.")]
    TooMany,
}

impl Problem {
    /// The variant the message already names, if any.
    pub fn variant(&self) -> Option<Variant> {
        match self {
            Problem::MissingVariant(v)
            | Problem::Unreadable(v)
            | Problem::BadFile(v)
            | Problem::WrongBrightness(v)
            | Problem::HardToRead { variant: v, .. } => Some(*v),
            _ => None,
        }
    }
}

fn font_ok(s: &str) -> bool {
    (1..=MAX_FONT_CHARS).contains(&s.chars().count()) && !s.chars().any(char::is_control)
}

/// The ranges, text, contrast and brightness one variant must pass (R4, R5).
pub fn check_variant(t: &Theme, author: Option<&str>) -> Result<(), Problem> {
    if !valid_display_text(&t.name) {
        return Err(Problem::Name);
    }
    if author.is_some_and(|a| !valid_display_text(a)) {
        return Err(Problem::Author);
    }
    let range = |what: &'static str, n: u8, lo: u8, hi: u8| {
        if (lo..=hi).contains(&n) {
            Ok(())
        } else {
            Err(Problem::OutOfRange(what))
        }
    };
    range("corner radius", t.ui.radius, 0, 40)?;
    range("gap between windows", t.ui.gap_in, 0, 50)?;
    range("gap to the screen edge", t.ui.gap_out, 0, 100)?;
    range("border width", t.ui.border_width, 0, 20)?;
    range("font size", t.fonts.size, 6, 32)?;
    range("cursor size", t.icons.cursor_size, 16, 96)?;
    if !font_ok(&t.fonts.ui) || !font_ok(&t.fonts.mono) {
        return Err(Problem::Font);
    }
    if !valid_theme_name(&t.icons.theme) || !valid_theme_name(&t.icons.cursor_theme) {
        return Err(Problem::IconName);
    }
    if let Some(f) = contrast::check_theme(t).first() {
        return Err(Problem::HardToRead {
            variant: t.variant,
            text: derive::problem_text(t.variant, f),
        });
    }
    if derive::brightness_ok(t) {
        Ok(())
    } else {
        Err(Problem::WrongBrightness(t.variant))
    }
}

/// Both variants pass [`check_variant`] and have the same name.
pub fn check_pair(dark: &Theme, light: &Theme, author: Option<&str>) -> Result<(), Problem> {
    check_variant(dark, author)?;
    check_variant(light, author)?;
    if dark.name != light.name {
        return Err(Problem::NamesDiffer);
    }
    Ok(())
}

/// One theme read from the user's folder, with both variants checked.
#[derive(Debug, Clone, PartialEq)]
pub struct UserTheme {
    pub dark: Theme,
    pub light: Theme,
    pub author: Option<String>,
}

/// A folder the scan left out.
#[derive(Debug, Clone, PartialEq)]
pub struct Skipped {
    /// The folder's name, only when it is safe to show.
    pub folder: Option<String>,
    pub problem: Problem,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserScan {
    pub themes: Vec<UserTheme>,
    pub skipped: Vec<Skipped>,
    /// The themes folder is a link, so nothing was read.
    pub root_is_link: bool,
}

/// The sorted, capped names of what `root` holds, or `None` when it is absent, not a folder, or
/// a link (`link` says which).
fn list(root: &Path, link: &mut bool) -> Option<Vec<std::ffi::OsString>> {
    let m = std::fs::symlink_metadata(root).ok()?;
    if m.file_type().is_symlink() {
        *link = true;
        return None;
    }
    if !m.is_dir() {
        return None;
    }
    let mut names: Vec<_> = std::fs::read_dir(root)
        .ok()?
        .take(MAX_LISTED)
        .filter_map(|e| Some(e.ok()?.file_name()))
        .collect();
    names.sort();
    names.truncate(MAX_SCANNED);
    Some(names)
}

/// Read every theme in `root` without following a link or waiting on a pipe (R3).
pub fn scan(root: &Path) -> UserScan {
    let mut out = UserScan::default();
    let Some(names) = list(root, &mut out.root_is_link) else {
        return out;
    };
    for name in names {
        let Some(folder) = name.to_str() else {
            out.skipped.push(Skipped {
                folder: None,
                problem: Problem::FolderName,
            });
            continue;
        };
        let path = root.join(folder);
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let shown = valid_theme_name(folder).then(|| folder.to_string());
        let mut skip = |problem| {
            out.skipped.push(Skipped {
                folder: shown.clone(),
                problem,
            });
        };
        if meta.file_type().is_symlink() {
            skip(Problem::Link);
            continue;
        }
        if !meta.is_dir() {
            continue;
        }
        if !valid_user_id(folder) {
            skip(Problem::FolderName);
            continue;
        }
        if out.themes.len() >= MAX_USER_THEMES {
            skip(Problem::TooMany);
            continue;
        }
        match read_theme(&path, folder) {
            Ok(t) => out.themes.push(t),
            Err(p) => out.skipped.push(Skipped {
                folder: shown,
                problem: p,
            }),
        }
    }
    out
}

fn read_variant(dir: &Path, id: &str, v: Variant) -> Result<(Theme, Option<String>), Problem> {
    let path = dir.join(format!("{v}.toml"));
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Problem::MissingVariant(v));
        }
        Err(_) => return Err(Problem::Unreadable(v)),
    };
    if meta.file_type().is_symlink() {
        return Err(Problem::Link);
    }
    // The file opened must be the one just looked at, so a swap between the two is not read.
    let text = read_capped_if(&path, |m| m.dev() == meta.dev() && m.ino() == meta.ino())
        .ok_or(Problem::Unreadable(v))?;
    Theme::from_toml_with_author(&text, id, v).map_err(|_| Problem::BadFile(v))
}

fn read_theme(dir: &Path, id: &str) -> Result<UserTheme, Problem> {
    let (dark, a_dark) = read_variant(dir, id, Variant::Dark)?;
    let (light, a_light) = read_variant(dir, id, Variant::Light)?;
    // R4: an empty author means none, as in a .uqtheme file.
    let a_dark = a_dark.filter(|a| !a.is_empty());
    let a_light = a_light.filter(|a| !a.is_empty());
    if a_light.as_deref().is_some_and(|a| !valid_display_text(a)) {
        return Err(Problem::Author);
    }
    check_pair(&dark, &light, a_dark.as_deref())?;
    Ok(UserTheme {
        dark,
        light,
        author: a_dark.or(a_light),
    })
}

/// What a metadata-only look at the folder found: each variant file's folder, size and
/// modification time (seconds and nanoseconds).
pub type Fingerprint = Vec<(String, u64, i64, i64)>;

/// A cheap fingerprint of the user's themes folder, read with the same no-follow rules as
/// [`scan`]. It changes when a variant file is added, removed, resized or touched.
pub fn fingerprint(root: &Path) -> Fingerprint {
    let mut link = false;
    let Some(names) = list(root, &mut link) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for name in names {
        let Some(folder) = name.to_str() else {
            continue;
        };
        let Ok(m) = std::fs::symlink_metadata(root.join(folder)) else {
            continue;
        };
        if !m.is_dir() {
            continue;
        }
        for v in Variant::ALL {
            let Ok(f) = std::fs::symlink_metadata(root.join(folder).join(format!("{v}.toml")))
            else {
                continue;
            };
            if f.is_file() {
                out.push((folder.to_string(), f.len(), f.mtime(), f.mtime_nsec()));
            }
        }
    }
    out
}

/// A variant file's contents; `Theme::from_toml` reads it back equal. The `toml` crate does the
/// quoting.
pub fn variant_toml(t: &Theme, author: Option<&str>) -> String {
    #[derive(Serialize)]
    struct Out<'a> {
        name: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        author: Option<&'a str>,
        colors: &'a super::Colors,
        ui: &'a super::Ui,
        fonts: &'a super::Fonts,
        icons: &'a super::Icons,
        terminal: &'a super::Terminal,
    }
    toml::to_string(&Out {
        name: &t.name,
        author,
        colors: &t.colors,
        ui: &t.ui,
        fonts: &t.fonts,
        icons: &t.icons,
        terminal: &t.terminal,
    })
    .expect("a theme is plain data")
}

#[cfg(test)]
mod tests;
