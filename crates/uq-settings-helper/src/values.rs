//! The closed value sets and strict name patterns every argument must pass before anything else
//! happens. Parsing is by exact match: no trimming, no case folding.

use std::fmt;

/// An argument was outside its closed set or pattern. The message names the argument and the
/// accepted form; it never echoes the rejected value (it may be long or carry control text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid(pub String);

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Invalid {}

/// A logind `Handle*` action uQ offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Suspend,
    Hibernate,
    Poweroff,
    Lock,
    Ignore,
}

impl PowerAction {
    pub const ALL: [PowerAction; 5] = [
        Self::Suspend,
        Self::Hibernate,
        Self::Poweroff,
        Self::Lock,
        Self::Ignore,
    ];

    pub fn parse(arg: &str, s: &str) -> Result<Self, Invalid> {
        Self::ALL
            .into_iter()
            .find(|a| a.as_str() == s)
            .ok_or_else(|| {
                Invalid(format!(
                    "{arg} must be suspend, hibernate, poweroff, lock or ignore"
                ))
            })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Suspend => "suspend",
            Self::Hibernate => "hibernate",
            Self::Poweroff => "poweroff",
            Self::Lock => "lock",
            Self::Ignore => "ignore",
        }
    }
}

/// A greeter theme variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GreeterVariant {
    Dark,
    Light,
}

impl GreeterVariant {
    pub const ALL: [GreeterVariant; 2] = [Self::Dark, Self::Light];

    pub fn parse(s: &str) -> Result<Self, Invalid> {
        Self::ALL
            .into_iter()
            .find(|v| v.as_str() == s)
            .ok_or_else(|| Invalid("variant must be dark or light".into()))
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

/// A theme name: `[a-z0-9-]{1,40}`. No dot, slash, space or control character can pass, so it
/// is safe as one path component and as a TOML basic string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeName(String);

impl ThemeName {
    pub const MAX: usize = 40;

    pub fn parse(s: &str) -> Result<Self, Invalid> {
        let ok = (1..=Self::MAX).contains(&s.len())
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if ok {
            Ok(Self(s.to_string()))
        } else {
            Err(Invalid(
                "theme must be 1 to 40 of a-z, 0-9 and -".to_string(),
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A login name: `^[a-z_][a-z0-9_-]{0,31}$` (the shadow-utils default, without a trailing `$`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserName(String);

impl UserName {
    pub const MAX: usize = 32;

    pub fn parse(s: &str) -> Result<Self, Invalid> {
        let b = s.as_bytes();
        let ok = (1..=Self::MAX).contains(&b.len())
            && (b[0].is_ascii_lowercase() || b[0] == b'_')
            && b[1..]
                .iter()
                .all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-');
        if ok {
            Ok(Self(s.to_string()))
        } else {
            Err(Invalid(
                "user must match ^[a-z_][a-z0-9_-]{0,31}$".to_string(),
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The lowest and highest uid auto-login accepts: regular accounts only (login.defs
/// `UID_MIN`..`UID_MAX`), which also leaves out `nobody` (65534).
pub const UID_MIN: u32 = 1000;
pub const UID_MAX: u32 = 60000;

#[cfg(test)]
/// Inputs no closed set or pattern may accept.
pub(crate) const HOSTILE: &[&str] = &[
    "",
    " ",
    "/",
    ".",
    "..",
    "../",
    "../etc",
    "../../etc/shadow",
    "/etc/passwd",
    "a/b",
    "a\\b",
    "a\nb",
    "a\rb",
    "a\0b",
    "\0",
    "a\u{7f}",
    "a b",
    "a=b",
    "a\"b",
    "a'b",
    "a]b",
    "a[b",
    "a#b",
    "a$b",
    "a`b",
    "a;b",
    "\u{202e}abc",
    "ä",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_actions_are_a_closed_set() {
        for a in PowerAction::ALL {
            assert_eq!(PowerAction::parse("lid", a.as_str()), Ok(a));
        }
        for bad in HOSTILE.iter().chain(&[
            "Suspend",
            "SUSPEND",
            " suspend",
            "suspend ",
            "suspend\n",
            "suspend-then-hibernate",
            "hybrid-sleep",
            "reboot",
            "halt",
            "kexec",
            "sleep",
            "factory-reset",
            "secure-attention-key",
        ]) {
            let e = PowerAction::parse("lid", bad).unwrap_err();
            assert!(e.0.starts_with("lid must be"), "{e}");
        }
    }

    #[test]
    fn variants_are_a_closed_set() {
        assert_eq!(GreeterVariant::parse("dark"), Ok(GreeterVariant::Dark));
        assert_eq!(GreeterVariant::parse("light"), Ok(GreeterVariant::Light));
        for bad in HOSTILE.iter().chain(&["Dark", "auto", "dark ", "darkk"]) {
            assert!(GreeterVariant::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn theme_names_follow_the_pattern() {
        for ok in ["dracula", "a", "my-theme-2", "0", "-", &"a".repeat(40)] {
            assert!(ThemeName::parse(ok).is_ok(), "{ok:?}");
        }
        let long = "a".repeat(41);
        for bad in HOSTILE.iter().copied().chain([
            "Dracula",
            "my_theme",
            "my.theme",
            "theme ",
            long.as_str(),
        ]) {
            assert!(ThemeName::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn user_names_follow_the_pattern() {
        for ok in [
            "alice",
            "_svc",
            "a",
            "bob-2",
            "x_y",
            &format!("a{}", "b".repeat(31)),
        ] {
            assert!(UserName::parse(ok).is_ok(), "{ok:?}");
        }
        let long = format!("a{}", "b".repeat(32));
        for bad in HOSTILE.iter().copied().chain([
            "Alice",
            "1alice",
            "-alice",
            "alice$",
            "al.ice",
            "root\n",
            long.as_str(),
        ]) {
            assert!(UserName::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn errors_never_echo_the_value() {
        let secret = "../../etc/shadow\nINJECT";
        for e in [
            PowerAction::parse("lid", secret).unwrap_err(),
            GreeterVariant::parse(secret).unwrap_err(),
            ThemeName::parse(secret).unwrap_err(),
            UserName::parse(secret).unwrap_err(),
        ] {
            assert!(!e.0.contains("shadow") && !e.0.contains("INJECT"), "{e}");
        }
    }
}
