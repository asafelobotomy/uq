//! Plain names for file and link types: shared-mime-info's own description of a file type
//! (`/usr/share/mime/image/png.xml` says "PNG image"), and a short list for link types.

use std::path::{Path, PathBuf};

/// The `mime` folders of `$XDG_DATA_HOME` and each `$XDG_DATA_DIRS` entry, in that order.
pub fn mime_dirs(data_home: &Path, data_dirs: &[PathBuf]) -> Vec<PathBuf> {
    std::iter::once(data_home)
        .chain(data_dirs.iter().map(PathBuf::as_path))
        .map(|d| d.join("mime"))
        .collect()
}

/// The largest description file read.
const MAX_INFO_FILE: u64 = 256 * 1024;

/// The English description shared-mime-info gives `mime`, from the first folder that has it.
pub fn describe(dirs: &[PathBuf], mime: &str) -> Option<String> {
    let (top, sub) = mime.split_once('/')?;
    let safe = |s: &str| {
        !s.is_empty() && !s.starts_with('.') && !s.contains(['/', '\\']) && s.len() <= 127
    };
    if !safe(top) || !safe(sub) {
        return None;
    }
    dirs.iter().find_map(|d| {
        let path = d.join(top).join(format!("{sub}.xml"));
        let m = std::fs::metadata(&path).ok()?;
        if !m.is_file() || m.len() > MAX_INFO_FILE {
            return None;
        }
        comment(&std::fs::read_to_string(path).ok()?)
    })
}

/// The first `<comment>` without an `xml:lang`, unescaped and cut to 120 characters.
fn comment(xml: &str) -> Option<String> {
    let start = xml.find("<comment>")? + "<comment>".len();
    let end = start + xml[start..].find("</comment>")?;
    let raw = &xml[start..end];
    let text = raw
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&");
    let text = crate::desktop_entry::clean_display(text.trim(), 120);
    (!text.is_empty()).then_some(text)
}

/// The scheme of a link type, `mailto` for `x-scheme-handler/mailto`.
pub fn scheme(mime: &str) -> Option<&str> {
    mime.strip_prefix("x-scheme-handler/")
        .filter(|s| !s.is_empty())
}

/// A plain name for a link scheme.
pub fn describe_scheme(scheme: &str) -> String {
    let known = match scheme {
        "http" => "Web pages (http)",
        "https" => "Secure web pages (https)",
        "mailto" => "Email addresses",
        "tel" | "callto" => "Phone numbers",
        "sms" => "Text messages",
        "geo" => "Map locations",
        "magnet" => "Torrent magnet links",
        "webcal" | "webcals" => "Calendar subscriptions",
        "irc" | "ircs" => "Chat rooms (IRC)",
        "matrix" => "Matrix chats",
        "xmpp" => "XMPP chats",
        "ftp" | "sftp" => "File transfers",
        "smb" => "Shared folders (SMB)",
        "steam" => "Steam links",
        "discord" => "Discord links",
        "slack" => "Slack links",
        "zoommtg" | "zoomus" => "Zoom meetings",
        "msteams" => "Teams meetings",
        "spotify" => "Spotify links",
        "vscode" => "VS Code links",
        "appstream" => "App store links",
        _ => "",
    };
    if known.is_empty() {
        format!("{scheme}: links")
    } else {
        known.to_string()
    }
}

#[cfg(test)]
pub(super) fn comment_for_test(xml: &str) -> Option<String> {
    comment(xml)
}
