//! Checking and loading a bar layout: the island lists, symbols, feeds and zones.

use super::*;

/// `^[A-Z0-9^.=-]{1,16}$` after upper-casing; must hold a letter or digit.
pub fn clean_symbol(s: &str) -> Option<String> {
    let s = s.trim().to_ascii_uppercase();
    let ok = (1..=16).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'^' | b'.' | b'=' | b'-'))
        && s.bytes().any(|b| b.is_ascii_alphanumeric());
    ok.then_some(s)
}

/// An `https://` URL with a host, no whitespace or control characters, at most 512 bytes.
pub fn valid_feed_url(u: &str) -> bool {
    let Some(rest) = u.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    u.len() <= MAX_URL
        && !host.is_empty()
        && !host.contains('@')
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':'))
        && !u.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// An IANA zone name: `Area/Location[/Sub]` or `UTC`.
pub fn valid_zone(z: &str) -> bool {
    let part = |p: &str| {
        !p.is_empty()
            && p.len() <= 32
            && p.starts_with(|c: char| c.is_ascii_alphabetic())
            && p.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'+'))
    };
    z == "UTC" || {
        let parts: Vec<&str> = z.split('/').collect();
        (2..=3).contains(&parts.len()) && parts.iter().all(|p| part(p))
    }
}

impl BarLayout {
    pub const FIELDS: [&str; 4] = ["start", "center", "end", "options"];
    pub const ZONES: [&str; 3] = ["start", "center", "end"];

    /// The layout a `shell.islands` table stood for: System and Work on the left; weather
    /// (when a location is set), the clock, notifications and status on the right.
    pub fn from_islands(i: &super::super::Islands, weather: bool) -> Self {
        let pick = |list: &[(&str, bool)]| -> Vec<BarItem> {
            list.iter()
                .filter(|(_, on)| *on)
                .map(|(id, _)| BarItem::new(id))
                .collect()
        };
        BarLayout {
            start: pick(&[("system", i.system), ("work", i.work)]),
            center: Vec::new(),
            end: pick(&[
                ("weather", weather),
                ("clock", i.time),
                ("notif", i.notif),
                ("status", i.status),
            ]),
            options: BarOptions::default(),
        }
    }

    pub fn zone(&self, name: &str) -> Option<&Vec<BarItem>> {
        match name {
            "start" => Some(&self.start),
            "center" => Some(&self.center),
            "end" => Some(&self.end),
            _ => None,
        }
    }

    pub fn zone_mut(&mut self, name: &str) -> Option<&mut Vec<BarItem>> {
        match name {
            "start" => Some(&mut self.start),
            "center" => Some(&mut self.center),
            "end" => Some(&mut self.end),
            _ => None,
        }
    }

    pub fn items(&self) -> impl Iterator<Item = &BarItem> {
        self.start.iter().chain(&self.center).chain(&self.end)
    }

    /// True when island `id` is placed in any zone.
    pub fn has(&self, id: &str) -> bool {
        self.items().any(|i| i.id == id)
    }

    /// Rules shared by the API and the loader.
    pub fn validate(&self) -> Result<(), SettingError> {
        const KEY: &str = "shell.bar";
        let mut seen: Vec<&str> = Vec::new();
        let mut n = 0;
        for item in self.items() {
            n += 1;
            if !known_island(&item.id) {
                return Err(invalid(KEY, format!("unknown island {:?}", item.id)));
            }
            item.check_units()
                .and_then(|()| item.check_text())
                .and_then(|()| item.check_fill())
                .map_err(|e| invalid(KEY, format!("island {:?}: {e}", item.id)))?;
            if item.id != SEPARATOR {
                if seen.contains(&item.id.as_str()) {
                    return Err(invalid(
                        KEY,
                        format!("island {:?} is placed twice", item.id),
                    ));
                }
                seen.push(&item.id);
            }
        }
        if n > MAX_ITEMS {
            return Err(invalid(KEY, format!("at most {MAX_ITEMS} islands")));
        }
        let o = &self.options;
        check_list(
            "shell.bar.options.stocks.symbols",
            &o.stocks.symbols,
            MAX_SYMBOLS,
            |s| clean_symbol(s).as_deref() == Some(s),
        )?;
        check_list(
            "shell.bar.options.news.feeds",
            &o.news.feeds,
            MAX_FEEDS,
            valid_feed_url,
        )?;
        o.all
            .text
            .check()
            .map_err(|e| invalid("shell.bar.options.all.text", e))?;
        if o.crumbs.warn_gb > MAX_WARN_GB {
            return Err(invalid(
                "shell.bar.options.crumbs.warn_gb",
                format!("at most {MAX_WARN_GB}"),
            ));
        }
        check_list(
            "shell.bar.options.worldclock.zones",
            &o.worldclock.zones,
            MAX_ZONES,
            valid_zone,
        )?;
        Ok(())
    }

    /// Load-time repair: drop unknown, duplicate and surplus entries and bad option values.
    pub fn sanitised(mut self) -> Self {
        let mut seen: Vec<String> = Vec::new();
        let mut n = 0;
        for zone in [&mut self.start, &mut self.center, &mut self.end] {
            zone.retain(|i| {
                let keep = known_island(&i.id)
                    && (i.id == SEPARATOR || !seen.contains(&i.id))
                    && n < MAX_ITEMS;
                if keep {
                    n += 1;
                    seen.push(i.id.clone());
                }
                keep
            });
            zone.iter_mut().for_each(BarItem::repair_units);
        }
        let o = &mut self.options;
        let mut syms: Vec<String> = Vec::new();
        for s in o.stocks.symbols.iter().filter_map(|s| clean_symbol(s)) {
            if !syms.contains(&s) {
                syms.push(s);
            }
        }
        syms.truncate(MAX_SYMBOLS);
        o.stocks.symbols = syms;
        dedup_valid(&mut o.news.feeds, MAX_FEEDS, valid_feed_url);
        dedup_valid(&mut o.worldclock.zones, MAX_ZONES, valid_zone);
        o.crumbs.warn_gb = o.crumbs.warn_gb.min(MAX_WARN_GB);
        o.all.text = std::mem::take(&mut o.all.text).repaired();
        self
    }

    /// Parse `shell.bar` strictly: unknown keys and wrong types are errors, missing fields
    /// take their defaults (an absent zone is empty; absent options are the defaults).
    pub fn from_value(v: &Value) -> Result<Self, SettingError> {
        const KEY: &str = "shell.bar";
        if !v.is_object() {
            return Err(invalid(KEY, "expected an object"));
        }
        check_known(KEY, v, &Self::FIELDS)?;
        let mut out = BarLayout {
            start: Vec::new(),
            center: Vec::new(),
            end: Vec::new(),
            options: BarOptions::default(),
        };
        for zone in Self::ZONES {
            let Some(z) = v.get(zone) else { continue };
            let key = format!("{KEY}.{zone}");
            let arr = z
                .as_array()
                .ok_or_else(|| invalid(&key, "expected an array"))?;
            for item in arr {
                check_known(&key, item, &BarItem::FIELDS)?;
                if let Some(t) = item.get("text") {
                    check_known(&format!("{key}.text"), t, &TextStyle::FIELDS)?;
                }
                let it: BarItem = serde_json::from_value(item.clone())
                    .map_err(|e| invalid(&key, e.to_string()))?;
                out.zone_mut(zone).expect("known zone").push(it);
            }
        }
        if let Some(o) = v.get("options") {
            let key = "shell.bar.options";
            check_known(key, o, &BarOptions::FIELDS)?;
            out.options =
                serde_json::from_value(o.clone()).map_err(|e| invalid(key, e.to_string()))?;
        }
        out.validate()?;
        Ok(out)
    }
}

pub(crate) fn check_list(
    key: &str,
    list: &[String],
    max: usize,
    ok: impl Fn(&str) -> bool,
) -> Result<(), SettingError> {
    if list.len() > max {
        return Err(invalid(key, format!("at most {max} entries")));
    }
    for (i, s) in list.iter().enumerate() {
        if !ok(s) {
            return Err(invalid(key, format!("invalid entry {s:?}")));
        }
        if list[..i].contains(s) {
            return Err(invalid(key, format!("duplicate entry {s:?}")));
        }
    }
    Ok(())
}

pub(crate) fn dedup_valid(list: &mut Vec<String>, max: usize, ok: impl Fn(&str) -> bool) {
    let mut out: Vec<String> = Vec::new();
    for s in list.drain(..) {
        if ok(&s) && !out.contains(&s) {
            out.push(s);
        }
    }
    out.truncate(max);
    *list = out;
}
