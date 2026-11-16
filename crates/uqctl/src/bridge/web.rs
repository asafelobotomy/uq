//! Spotlight's web row: a typed web address (`example.com`, `https://…`, `localhost:3000`, an IP
//! address) opens in the default browser.
//!
//! Only http and https are ever opened. A query that is clearly an address (a scheme, `www.`,
//! `localhost`, an IP address or a common domain ending such as `.com`) is the top hit; one that
//! only looks like a domain (`notes.md`) still gets a row, ranked below files and apps.

use super::calc::ProviderItem;
use super::desktop_entry::Dirs;
use uq_core::mimeapps;

const MAX_URL: usize = 2048;
/// Domain endings that make a bare `name.tld` clearly a web address.
const COMMON_TLDS: &[&str] = &[
    "com", "org", "net", "io", "dev", "app", "ai", "co", "me", "info", "edu", "gov", "uk", "de",
    "fr", "nl", "eu", "ca", "au", "us", "jp", "xyz", "tv", "gg", "so", "sh", "page", "blog",
    "news", "tech", "site", "online", "store", "cloud", "wiki", "social", "chat",
];

/// A typed address: the URL to open, and whether the query is clearly one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub url: String,
    pub strong: bool,
}

fn ipv4(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 3
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u8>().is_ok()
        })
}

fn label_ok(l: &str) -> bool {
    !l.is_empty()
        && l.len() <= 63
        && !l.starts_with('-')
        && !l.ends_with('-')
        && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// The domain's ending when `host` is a plausible domain name (two or more labels, an
/// alphabetic ending of two or more letters, or a punycode one), else None.
fn domain_tld(host: &str) -> Option<&str> {
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 || !labels.iter().all(|l| label_ok(l)) {
        return None;
    }
    let tld = *labels.last()?;
    let ok =
        (tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic())) || tld.starts_with("xn--");
    ok.then_some(tld)
}

/// The address in `query`, if it is one.
pub fn parse(query: &str) -> Option<Address> {
    let q = query.trim();
    if q.is_empty() || q.len() > MAX_URL || q.chars().any(char::is_whitespace) {
        return None;
    }
    let lower = q.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        let u = reqwest::Url::parse(q).ok()?;
        let host = u.host_str()?;
        if host.is_empty() {
            return None;
        }
        return Some(Address {
            url: u.to_string(),
            strong: true,
        });
    }
    if q.contains("://") || q.starts_with(['=', '>', ':', '/', '.', '~']) {
        return None;
    }
    let end = q.find(['/', '?', '#']).unwrap_or(q.len());
    let hostport = &q[..end];
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()) => {
            (h, Some(p))
        }
        Some(_) => return None,
        None => (hostport, None),
    };
    let host_l = host.to_ascii_lowercase();
    let (scheme, strong) = if host_l == "localhost" || ipv4(&host_l) {
        ("http", true)
    } else {
        let tld = domain_tld(&host_l)?;
        let strong = host_l.starts_with("www.")
            || port.is_some()
            || COMMON_TLDS.contains(&tld.to_ascii_lowercase().as_str());
        ("https", strong)
    };
    let u = reqwest::Url::parse(&format!("{scheme}://{q}")).ok()?;
    Some(Address {
        url: u.to_string(),
        strong,
    })
}

/// The default browser's `{name, icon}` (from `x-scheme-handler/https`), if one is set.
fn browser() -> Option<(String, String)> {
    let id = mimeapps::lookup(
        &mimeapps::user_file(),
        &mimeapps::system_files(),
        "x-scheme-handler/https",
    )
    .or_else(|| {
        mimeapps::lookup(
            &mimeapps::user_file(),
            &mimeapps::system_files(),
            "x-scheme-handler/http",
        )
    })?;
    let app = super::defaultapps::app(&Dirs::from_env(), &id);
    let name = app.get("name")?.as_str()?.to_string();
    let icon = app
        .get("icon")
        .and_then(|i| i.as_str())
        .unwrap_or("")
        .to_string();
    Some((name, icon))
}

/// The row for an address: "Open <what was typed>", in the default browser.
pub fn item(query: &str, a: &Address) -> ProviderItem {
    let (name, icon) = browser().unwrap_or_default();
    ProviderItem {
        id: a.url.clone(),
        title: format!("Open {}", query.trim()),
        subtitle: if name.is_empty() {
            "Web address, in your browser".into()
        } else {
            format!("Web address, in {name}")
        },
        icon: if super::providers::valid_icon(&icon) {
            icon
        } else {
            "web-browser".into()
        },
        score: if a.strong { 1.0 } else { 0.45 },
        actions: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(q: &str) -> Option<(String, bool)> {
        parse(q).map(|a| (a.url, a.strong))
    }

    #[test]
    fn clear_addresses_are_strong() {
        assert_eq!(
            url("example.com"),
            Some(("https://example.com/".into(), true))
        );
        assert_eq!(
            url("www.example.zz"),
            Some(("https://www.example.zz/".into(), true))
        );
        assert_eq!(
            url("github.com/a/b?q=1#x"),
            Some(("https://github.com/a/b?q=1#x".into(), true))
        );
        assert_eq!(
            url("HTTPS://Example.com/X"),
            Some(("https://example.com/X".into(), true))
        );
        assert_eq!(url("http://x.y/p"), Some(("http://x.y/p".into(), true)));
        assert_eq!(
            url("localhost:3000"),
            Some(("http://localhost:3000/".into(), true))
        );
        assert_eq!(
            url("192.168.1.1/admin"),
            Some(("http://192.168.1.1/admin".into(), true))
        );
        assert_eq!(
            url("my.host:8080"),
            Some(("https://my.host:8080/".into(), true))
        );
    }

    #[test]
    fn domain_like_words_are_weak() {
        assert_eq!(url("notes.md"), Some(("https://notes.md/".into(), false)));
        assert_eq!(url("main.rs"), Some(("https://main.rs/".into(), false)));
    }

    #[test]
    fn not_addresses() {
        for q in [
            "",
            "firefox",
            "hello world",
            "a.b c",
            "1.5",
            "3.14",
            "file.c2",
            "=1+1",
            ">ls",
            ":smile",
            "/usr/bin",
            "~/x",
            ".hidden",
            "ftp://x.com",
            "javascript:alert(1)",
            "file:///etc",
            "x..com",
            "-a.com",
            "a-.com",
            "999.1.1.1",
            "a.com:",
            "a.com:99999x",
            "localhost:",
        ] {
            assert_eq!(url(q), None, "{q}");
        }
        assert_eq!(url(&format!("{}.com", "a".repeat(MAX_URL))), None);
    }

    #[test]
    fn ranks() {
        let strong = parse("example.com").unwrap();
        assert_eq!(item("example.com", &strong).score, 1.0);
        assert_eq!(item("example.com", &strong).title, "Open example.com");
        let weak = parse("notes.md").unwrap();
        assert!(item("notes.md", &weak).score < 0.5);
    }
}
