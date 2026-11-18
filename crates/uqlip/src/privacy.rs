//! What never goes into the history.
//!
//! Password managers mark what they copy with the `x-kde-passwordManagerHint` MIME type (value
//! `secret`); KeePassXC, Bitwarden and 1Password all do. Anything marked is skipped. On top of
//! that the owner can name apps whose copies are never kept (by window class), and pause the
//! history altogether. Skipping is silent on purpose: the shell never shows what was skipped.

use serde::{Deserialize, Serialize};

pub const SECRET_HINT_MIME: &str = "x-kde-passwordManagerHint";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Privacy {
    /// Window classes (case-insensitive, whole name) whose copies are not kept.
    pub ignore_apps: Vec<String>,
    /// Text longer than this many bytes is not kept (a pasted log is rarely wanted again).
    pub max_text_bytes: u64,
}

impl Default for Privacy {
    fn default() -> Self {
        Privacy {
            ignore_apps: [
                "org.keepassxc.KeePassXC",
                "keepassxc",
                "KeePassXC",
                "Bitwarden",
                "1Password",
                "proton-pass",
                "Proton Pass",
                "org.gnome.seahorse.Application",
            ]
            .map(String::from)
            .to_vec(),
            max_text_bytes: 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Skip {
    Paused,
    SecretHint,
    IgnoredApp,
}

impl Privacy {
    /// `hint` is the content of `x-kde-passwordManagerHint` if the source offered it.
    /// Offering the type at all, with any content other than "normal" or "clipboard", is
    /// treated as secret: a manager that sets it means it.
    pub fn check(
        &self,
        paused: bool,
        offered: &[String],
        hint: Option<&str>,
        app: Option<&str>,
    ) -> Result<(), Skip> {
        if paused {
            return Err(Skip::Paused);
        }
        if offered.iter().any(|m| m == SECRET_HINT_MIME) {
            let h = hint.unwrap_or("secret").trim();
            if !matches!(h, "normal" | "clipboard") {
                return Err(Skip::SecretHint);
            }
        }
        if let Some(a) = app
            && self.ignore_apps.iter().any(|i| i.eq_ignore_ascii_case(a))
        {
            return Err(Skip::IgnoredApp);
        }
        Ok(())
    }
}

/// Pick what to keep from the offered types: an image when there is one, else plain text.
/// Returns the MIME to ask for and its kind.
pub fn choose(offered: &[String]) -> Option<(String, crate::store::Kind)> {
    use crate::store::Kind;
    for want in ["image/png", "image/jpeg", "image/webp", "image/bmp"] {
        if offered.iter().any(|m| m == want) {
            return Some((want.to_owned(), Kind::Image));
        }
    }
    for want in [
        "text/plain;charset=utf-8",
        "text/plain",
        "UTF8_STRING",
        "TEXT",
        "STRING",
    ] {
        if offered.iter().any(|m| m == want) {
            return Some((want.to_owned(), Kind::Text));
        }
    }
    None
}
