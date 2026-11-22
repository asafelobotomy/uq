//! What a report leaves out before it is shared: window titles (they can name a bank, a chat or a
//! document), the home folder, the user and computer names, e-mail, IP and MAC addresses.
//! App classes, workspace numbers and uQ's own paths stay: they are what makes a report useful.

use regex::Regex;
use std::sync::LazyLock;

/// Who is being redacted for: the home folder, user name and host name.
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    home: Option<String>,
    user: Option<String>,
    host: Option<String>,
}

static EMAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap());
static IPV4: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:25[0-5]|2[0-4]\d|1?\d?\d)(?:\.(?:25[0-5]|2[0-4]\d|1?\d?\d)){3}\b").unwrap()
});
static IPV6: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:[0-9a-fA-F]{1,4}:){4,7}[0-9a-fA-F]{1,4}\b").unwrap());
static MAC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:[0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}\b").unwrap());
/// Hyprland socket2 events that carry a title (as received, `name>>data`, and as Quickshell
/// logs them parsed, `"name" "data"`), with how many comma-separated fields come before it: `activewindow>>class,title`, `windowtitlev2>>addr,title`,
/// `openwindow>>addr,workspace,class,title`.
static EVENT_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\b(activewindow|windowtitlev2|openwindow)(>>|" ")((?:[^,"\n]*,){1,3})([^"\n]*)"#)
        .unwrap()
});
/// A quoted window title in Quickshell's and uQ's debug lines: `toplevel "…"`, `title: "…"`.
static QUOTED_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)\b(toplevel|title|initialTitle|initial_title)(["']?\s*[:=]?\s*)"(?:[^"\\]|\\.)*""#,
    )
    .unwrap()
});

impl Redactor {
    /// For this computer: `$HOME`, `$USER` (or `$LOGNAME`) and the host name.
    pub fn from_env() -> Redactor {
        let host = std::fs::read_to_string("/proc/sys/kernel/hostname")
            .ok()
            .map(|s| s.trim().to_string());
        Redactor::new(
            std::env::var("HOME").ok(),
            std::env::var("USER")
                .or_else(|_| std::env::var("LOGNAME"))
                .ok(),
            host,
        )
    }

    pub fn new(home: Option<String>, user: Option<String>, host: Option<String>) -> Redactor {
        // Too short to replace safely ("a" would hit every word).
        let keep = |s: Option<String>| s.filter(|s| s.len() >= 3 && s != "root");
        Redactor {
            home: home
                .filter(|h| h.len() > 1)
                .map(|h| h.trim_end_matches('/').to_string()),
            user: keep(user),
            host: keep(host).filter(|h| h != "localhost" && h != "archlinux"),
        }
    }

    /// `text` with the private parts replaced by `<title>`, `~`, `<user>`, `<host>`, `<email>`,
    /// `<ip>` and `<mac>`.
    pub fn apply(&self, text: &str) -> String {
        let mut s = EVENT_TITLE
            .replace_all(text, |c: &regex::Captures| {
                // `openwindow` has three fields before the title, the others one.
                let fields = if &c[1] == "openwindow" { 3 } else { 1 };
                let pre: Vec<&str> = c[3].split_terminator(',').collect();
                if pre.len() < fields {
                    // Fewer fields than a title needs: the line had no title after all.
                    return c[0].to_string();
                }
                let (keep, rest) = pre.split_at(fields);
                let mut out = format!("{}{}{},", &c[1], &c[2], keep.join(","));
                if !rest.is_empty() || !c[4].is_empty() {
                    out.push_str("<title>");
                }
                out
            })
            .into_owned();
        s = QUOTED_TITLE
            .replace_all(&s, |c: &regex::Captures| {
                format!("{}{}\"<title>\"", &c[1], &c[2])
            })
            .into_owned();
        if let Some(h) = &self.home {
            s = s.replace(h.as_str(), "~");
        }
        s = EMAIL.replace_all(&s, "<email>").into_owned();
        s = MAC.replace_all(&s, "<mac>").into_owned();
        s = IPV6.replace_all(&s, "<ip>").into_owned();
        s = IPV4
            .replace_all(&s, |c: &regex::Captures| {
                let ip = &c[0];
                // Loopback and the unspecified address say nothing about anyone.
                if ip.starts_with("127.") || ip == "0.0.0.0" {
                    ip.to_string()
                } else {
                    "<ip>".to_string()
                }
            })
            .into_owned();
        if let Some(u) = &self.user {
            s = replace_word(&s, u, "<user>");
        }
        if let Some(h) = &self.host {
            s = replace_word(&s, h, "<host>");
        }
        s
    }
}

/// Replace `word` where it isn't part of a longer name (so a user called "sol" doesn't turn
/// "console" into "con<user>e").
fn replace_word(s: &str, word: &str, with: &str) -> String {
    let re = Regex::new(&format!(
        r"(^|[^A-Za-z0-9_]){}($|[^A-Za-z0-9_])",
        regex::escape(word)
    ))
    .expect("an escaped word is a valid pattern");
    // Twice: adjacent matches share a boundary character.
    let once = re.replace_all(s, |c: &regex::Captures| format!("{}{with}{}", &c[1], &c[2]));
    re.replace_all(&once, |c: &regex::Captures| {
        format!("{}{with}{}", &c[1], &c[2])
    })
    .into_owned()
}
