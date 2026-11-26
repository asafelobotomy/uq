//! Parsing RSS and Atom feeds: items, links, dates and plain text.

use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Field {
    FeedTitle,
    Title,
    Link,
    Date,
    Updated,
}

#[derive(Default)]
pub(crate) struct ItemBuf {
    pub(crate) title: String,
    pub(crate) link: Option<String>,
    pub(crate) alt_link: Option<String>,
    pub(crate) date: String,
    pub(crate) updated: String,
}

/// Parse an RSS 2.0, RSS 1.0 or Atom document. A document cut off at the size cap keeps the
/// items completed before the cut.
pub fn parse_feed(bytes: &[u8]) -> Result<Parsed, &'static str> {
    let text = String::from_utf8_lossy(bytes);
    let mut r = quick_xml::Reader::from_str(&text);
    r.config_mut().check_end_names = false;
    let mut stack: Vec<String> = Vec::new();
    let mut is_feed = false;
    let mut feed_title: Option<String> = None;
    let mut item: Option<ItemBuf> = None;
    let mut items = Vec::new();
    let mut capture: Option<(Field, usize)> = None;
    let mut buf = String::new();
    loop {
        let ev = match r.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(ev) => ev,
        };
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(ev, Event::Empty(_));
                let name = e.name().as_ref().to_string();
                if stack.is_empty() {
                    is_feed = matches!(name.as_str(), "rss" | "feed" | "rdf:RDF" | "RDF");
                    if !is_feed {
                        return Err("not a feed");
                    }
                }
                let parent = stack.last().map(String::as_str).unwrap_or("");
                if capture.is_none() {
                    let in_item = item.is_some() && matches!(parent, "item" | "entry");
                    match name.as_str() {
                        "item" | "entry" => item = Some(ItemBuf::default()),
                        "title" if in_item => capture = Some((Field::Title, stack.len() + 1)),
                        "title"
                            if feed_title.is_none()
                                && item.is_none()
                                && matches!(parent, "channel" | "feed") =>
                        {
                            capture = Some((Field::FeedTitle, stack.len() + 1));
                        }
                        "link" if in_item => {
                            let href = e.try_get_attribute("href").ok().flatten().and_then(|a| {
                                a.normalized_value(XmlVersion::Implicit1_0)
                                    .ok()
                                    .map(|v| v.into_owned())
                            });
                            match href {
                                Some(h) => {
                                    let rel = e
                                        .try_get_attribute("rel")
                                        .ok()
                                        .flatten()
                                        .and_then(|a| {
                                            a.normalized_value(XmlVersion::Implicit1_0).ok()
                                        })
                                        .map(|v| v.into_owned());
                                    let it = item.as_mut().expect("in item");
                                    if matches!(rel.as_deref(), None | Some("alternate")) {
                                        it.link.get_or_insert(h);
                                    } else {
                                        it.alt_link.get_or_insert(h);
                                    }
                                }
                                None => capture = Some((Field::Link, stack.len() + 1)),
                            }
                        }
                        "pubDate" | "published" | "dc:date" if in_item => {
                            capture = Some((Field::Date, stack.len() + 1));
                        }
                        "updated" if in_item => capture = Some((Field::Updated, stack.len() + 1)),
                        _ => {}
                    }
                }
                stack.push(name);
                // An empty element ends at once.
                if empty {
                    end_element(
                        &mut stack,
                        &mut capture,
                        &mut buf,
                        &mut item,
                        &mut items,
                        &mut feed_title,
                    );
                }
            }
            Event::End(_) => end_element(
                &mut stack,
                &mut capture,
                &mut buf,
                &mut item,
                &mut items,
                &mut feed_title,
            ),
            Event::Text(t) if capture.is_some() => {
                buf.push_str(&t);
            }
            Event::CData(t) if capture.is_some() => {
                buf.push_str(&t);
            }
            Event::GeneralRef(g) if capture.is_some() => {
                if let Ok(Some(c)) = g.resolve_char_ref() {
                    buf.push(c);
                } else {
                    let name: &str = &g;
                    match name {
                        "lt" => buf.push('<'),
                        "gt" => buf.push('>'),
                        "amp" => buf.push('&'),
                        "quot" => buf.push('"'),
                        "apos" => buf.push('\''),
                        // Left for the HTML pass (`&nbsp;` and friends).
                        other => {
                            buf.push('&');
                            buf.push_str(other);
                            buf.push(';');
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if !is_feed {
        return Err("not a feed");
    }
    Ok(Parsed {
        title: feed_title,
        items,
    })
}

pub(crate) fn end_element(
    stack: &mut Vec<String>,
    capture: &mut Option<(Field, usize)>,
    buf: &mut String,
    item: &mut Option<ItemBuf>,
    items: &mut Vec<Item>,
    feed_title: &mut Option<String>,
) {
    if let Some((field, depth)) = *capture
        && depth == stack.len()
    {
        let text = std::mem::take(buf);
        match (field, item.as_mut()) {
            (Field::FeedTitle, _) => {
                *feed_title = Some(plain_text(&text, MAX_SOURCE)).filter(|t| !t.is_empty());
            }
            (Field::Title, Some(it)) => it.title = text,
            (Field::Link, Some(it)) => {
                it.link.get_or_insert(text);
            }
            (Field::Date, Some(it)) => it.date = text,
            (Field::Updated, Some(it)) => it.updated = text,
            _ => {}
        }
        *capture = None;
    }
    let name = stack.pop().unwrap_or_default();
    if matches!(name.as_str(), "item" | "entry")
        && let Some(it) = item.take()
    {
        let title = plain_text(&it.title, MAX_TITLE);
        let link = it.link.or(it.alt_link).and_then(|l| clean_link(&l));
        if let Some(link) = link
            && !title.is_empty()
        {
            let date = if it.date.trim().is_empty() {
                &it.updated
            } else {
                &it.date
            };
            items.push(Item {
                title,
                link,
                published: parse_date(date),
            });
        }
    }
}

/// An absolute http(s) URL with a host, normalised; anything else is dropped.
pub fn clean_link(l: &str) -> Option<String> {
    let l = l.trim();
    if l.len() > MAX_LINK {
        return None;
    }
    let u = reqwest::Url::parse(l).ok()?;
    (matches!(u.scheme(), "http" | "https") && u.host_str().is_some_and(|h| !h.is_empty()))
        .then(|| u.to_string())
}

/// RFC 2822 (RSS) or RFC 3339 (Atom) to unix seconds.
pub fn parse_date(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(s)
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(&s.replacen(' ', "T", 1)))
    {
        return Some(d.timestamp());
    }
    // RFC 2822 without the weekday (feeds often get it wrong, and chrono checks it), with a
    // `UTC` zone name read as `+0000`.
    let s = match s.split_once(',') {
        Some((day, rest)) if day.len() <= 9 && day.chars().all(|c| c.is_ascii_alphabetic()) => {
            rest.trim()
        }
        _ => s,
    };
    let fixed = s.strip_suffix(" UTC").map(|b| format!("{b} +0000"));
    chrono::DateTime::parse_from_rfc2822(fixed.as_deref().unwrap_or(s))
        .ok()
        .map(|d| d.timestamp())
}

/// Markup and entities out, whitespace collapsed, at most `max` characters.
pub fn plain_text(s: &str, max: usize) -> String {
    let s = strip_tags(&decode_entities(&strip_tags(s)));
    let mut out = String::new();
    for word in s.split(|c: char| c.is_whitespace() || c.is_control()) {
        if word.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.chars().count() > max {
        out = out
            .chars()
            .take(max - 1)
            .collect::<String>()
            .trim_end()
            .to_owned();
        out.push('…');
    }
    out
}

/// Remove `<tag ...>`, `</tag>`, `<!-- -->` and `<?...?>`; a `<` not starting a tag stays.
pub(crate) fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let tag_like = after
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '/' | '!' | '?'));
        match (tag_like, after.find('>')) {
            (true, Some(j)) => {
                out.push(' ');
                rest = &after[j + 1..];
            }
            (true, None) => {
                rest = "";
            }
            (false, _) => {
                out.push('<');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Decode numeric and the common named HTML entities; unknown ones stay as written.
pub(crate) fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let end = after.find(';').filter(|&j| j <= 10);
        let decoded = end.and_then(|j| {
            let name = &after[..j];
            let c = if let Some(num) = name.strip_prefix('#') {
                let n = match num.strip_prefix(['x', 'X']) {
                    Some(h) => u32::from_str_radix(h, 16).ok(),
                    None => num.parse().ok(),
                };
                n.and_then(char::from_u32).filter(|c| *c != '\0')
            } else {
                named_entity(name)
            };
            c.map(|c| (c, j))
        });
        match decoded {
            Some((c, j)) => {
                out.push(c);
                rest = &after[j + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

pub(crate) fn named_entity(n: &str) -> Option<char> {
    Some(match n {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "ndash" => '–',
        "mdash" => '—',
        "lsquo" => '‘',
        "rsquo" => '’',
        "ldquo" => '“',
        "rdquo" => '”',
        "hellip" => '…',
        "euro" => '€',
        "pound" => '£',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "deg" => '°',
        "middot" => '·',
        "bull" => '•',
        "eacute" => 'é',
        "egrave" => 'è',
        "aacute" => 'á',
        "oacute" => 'ó',
        "uuml" => 'ü',
        "ouml" => 'ö',
        "auml" => 'ä',
        "szlig" => 'ß',
        _ => return None,
    })
}
