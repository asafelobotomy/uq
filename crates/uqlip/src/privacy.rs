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
    /// Skip text that looks like a secret (API keys, tokens, private keys, card numbers),
    /// whatever app it came from.
    pub skip_secret_looking: bool,
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
            skip_secret_looking: true,
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

/// Does this text look like something that should not sit in a history? Deliberately narrow:
/// only well-known key shapes, JWTs, private keys, card numbers that pass the Luhn check, and a
/// single long mixed-case token with digits. A wrong skip costs a copy that is not remembered, so
/// plain words, URLs, paths, hex hashes and ordinary sentences are never matched.
pub fn looks_secret(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() || t.len() > 8192 {
        return false;
    }
    if t.contains("-----BEGIN ") && t.contains("PRIVATE KEY-----") {
        return true;
    }
    // Everything below is a single token: any whitespace means prose or a command.
    if t.chars().any(char::is_whitespace) {
        return looks_like_card(t);
    }
    const PREFIXES: [&str; 14] = [
        "sk-",
        "sk_live_",
        "sk_test_",
        "rk_live_",
        "ghp_",
        "gho_",
        "ghs_",
        "github_pat_",
        "glpat-",
        "xoxb-",
        "xoxp-",
        "xoxa-",
        "AKIA",
        "AIza",
    ];
    if t.len() >= 16 && PREFIXES.iter().any(|p| t.starts_with(p)) {
        return true;
    }
    if t.starts_with("eyJ") && t.matches('.').count() == 2 && t.len() >= 30 {
        return true;
    }
    if looks_like_card(t) {
        return true;
    }
    // A long random-looking token: letters of both cases and digits, no separators of a URL or path.
    t.len() >= 32
        && t.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && t.chars().any(|c| c.is_ascii_uppercase())
        && t.chars().any(|c| c.is_ascii_lowercase())
        && t.chars().any(|c| c.is_ascii_digit())
}

/// 13 to 19 digits (spaces and dashes allowed between groups) that pass the Luhn check.
fn looks_like_card(t: &str) -> bool {
    if !t
        .chars()
        .all(|c| c.is_ascii_digit() || c == ' ' || c == '-')
    {
        return false;
    }
    let digits: Vec<u32> = t.chars().filter_map(|c| c.to_digit(10)).collect();
    if !(13..=19).contains(&digits.len()) {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, d)| {
            if i % 2 == 1 {
                let x = d * 2;
                if x > 9 { x - 9 } else { x }
            } else {
                *d
            }
        })
        .sum();
    sum.is_multiple_of(10)
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

#[cfg(test)]
mod tests {
    use super::looks_secret;

    #[test]
    fn catches_known_secret_shapes() {
        assert!(looks_secret("ghp_abcdefghijklmnopqrstuvwxyz0123456789"));
        assert!(looks_secret("sk-ant-api03-abcdefghijklmnop"));
        assert!(looks_secret("AKIAIOSFODNN7EXAMPLE"));
        assert!(looks_secret(
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTYifQ.abc123def456ghi789"
        ));
        assert!(looks_secret(
            "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----"
        ));
        assert!(looks_secret("4111 1111 1111 1111"));
        assert!(looks_secret("aB3dE6gH9jK2mN5pQ8sT1vW4yZ7bC0dF"));
    }

    #[test]
    fn leaves_ordinary_copies_alone() {
        assert!(!looks_secret("the first thing I copied"));
        assert!(!looks_secret(
            "https://example.com/a/very/long/path?with=query&and=more"
        ));
        assert!(!looks_secret(
            "/home/user/Projects/uq/crates/uqlip/src/privacy.rs"
        ));
        assert!(!looks_secret("da39a3ee5e6b4b0d3255bfef95601890afd80709"));
        assert!(!looks_secret("4111 1111 1111 1112"));
        assert!(!looks_secret("1234567890"));
        assert!(!looks_secret("sk-"));
        assert!(!looks_secret("git commit -m fix"));
    }
}
