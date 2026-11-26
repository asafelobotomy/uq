//! What a profile holds: reading current values and binds from uqd, and creating, editing,
//! reporting and importing profiles with them.

use super::*;

/// The journal domains a profile's keys come from.
pub(crate) fn profile_journal_domains() -> Vec<&'static str> {
    let mut out = vec!["theme"];
    out.extend(
        PROFILE_DOMAINS
            .iter()
            .filter(|(_, k)| {
                matches!(
                    k,
                    DomainKind::Section
                        | DomainKind::Appearance
                        | DomainKind::Rules
                        | DomainKind::Keybinds
                        | DomainKind::Env
                        | DomainKind::Compat
                        | DomainKind::Autostart
                )
            })
            .map(|(d, _)| *d),
    );
    out
}

pub(crate) async fn read_binds(proxy: &DesktopProxy<'_>) -> zbus::Result<Keybinds> {
    Keybinds::from_section(
        &parse_json(&proxy.get_section("keybinds").await?)?,
        &Keybinds::default(),
    )
    .map_err(|_| failure("uQ's settings service sent unreadable shortcuts"))
}

/// The current value of every set key of `journal_domains`, as profile keys. A domain a profile
/// can't hold adds nothing.
pub(crate) async fn current(
    proxy: &DesktopProxy<'_>,
    journal_domains: &[&str],
) -> zbus::Result<Vec<Key>> {
    let rules = match journal_domains.contains(&"rules") {
        true => read_rules(proxy).await?,
        false => Rules::default(),
    };
    let binds = match journal_domains.contains(&"keybinds") {
        true => read_binds(proxy).await?,
        false => Keybinds::default(),
    };
    let startup = match journal_domains.iter().any(|d| startup::DOMAINS.contains(d)) {
        true => Startup::read(proxy, Vec::new()).await?,
        false => Startup::default(),
    };
    current_with(proxy, journal_domains, &rules, &binds, &startup).await
}

/// [`current`] with the rules and the startup settings already read. Every window and panel
/// rule's switch is a key, and every startup key a profile can hold.
pub(crate) async fn current_with(
    proxy: &DesktopProxy<'_>,
    journal_domains: &[&str],
    rules: &Rules,
    binds: &Keybinds,
    startup: &Startup,
) -> zbus::Result<Vec<Key>> {
    let mut out: Vec<Key> = Vec::new();
    for &domain in journal_domains {
        let special = match domain {
            "rules" => rule_keys(rules),
            "keybinds" => bind_keys(binds),
            d if startup::DOMAINS.contains(&d) => startup.keys(d),
            _ => Vec::new(),
        };
        if domain == "rules" || domain == "keybinds" || startup::DOMAINS.contains(&domain) {
            for k in special {
                if !out.iter().any(|o| o.domain == k.domain && o.path == k.path) {
                    out.push(k);
                }
            }
            continue;
        }
        let tree = if domain == "theme" {
            let (theme, variant) = proxy.get_theme().await?;
            json!({"theme": theme, "variant": variant})
        } else if holds_journal_domain(domain) {
            parse_json(&proxy.get_section(domain).await?)?
        } else {
            continue;
        };
        for (path, value) in leaves(&tree) {
            if value.is_null() {
                continue;
            }
            let Some((domain, path)) = from_journal(domain, &path) else {
                continue;
            };
            if !out.iter().any(|k| k.domain == domain && k.path == path) {
                out.push(Key {
                    domain,
                    path,
                    value,
                });
            }
        }
    }
    Ok(out)
}

/// `keys` with their current values. A key that's unset now can't go in a profile: it's counted.
pub(crate) async fn with_values(
    proxy: &DesktopProxy<'_>,
    keys: Vec<(String, Vec<String>)>,
) -> zbus::Result<(Vec<Key>, usize)> {
    let now: HashMap<(String, Vec<String>), Value> = current(proxy, &profile_journal_domains())
        .await?
        .into_iter()
        .map(|k| ((k.domain, k.path), k.value))
        .collect();
    let mut found = Vec::new();
    let mut skipped = 0;
    for (domain, path) in keys {
        match now.get(&(domain.clone(), path.clone())) {
            Some(value) => found.push(Key {
                domain,
                path,
                value: value.clone(),
            }),
            // A variable that isn't set now: the profile removes it.
            None if domain == "env" && path.len() == 1 => found.push(Key {
                domain,
                path,
                value: Value::Bool(false),
            }),
            None => skipped += 1,
        }
    }
    Ok((found, skipped))
}

/// `{domain, path, label, text}` for the page. A rule switch is named from `rules`; startup
/// apps and fixes by the names `startup` knows.
pub(crate) fn key_json(
    domain: &str,
    path: &[String],
    value: &Value,
    rules: &Rules,
    binds: &Keybinds,
    startup: &Startup,
) -> Value {
    let label = match (domain, path) {
        ("rules", [id]) => rule_label(rules, id),
        ("keybinds", [id]) => bind_label(binds, id),
        (d, _) if startup::DOMAINS.contains(&d) => startup.label(domain, path),
        _ => key_label(domain, path),
    };
    json!({
        "domain": domain,
        "path": path,
        "label": label,
        "text": value_text(domain, path, value),
    })
}

impl ProfilesState {
    /// Save a new profile holding `keys`.
    pub(crate) async fn create(
        &self,
        proxy: &DesktopProxy<'_>,
        name: &str,
        icon: &str,
        keys: Vec<Key>,
    ) -> zbus::Result<()> {
        if keys.is_empty() {
            return Err(profile_error(ProfileError::Empty));
        }
        let name = name.trim();
        let mut p = Profile {
            id: id_for_name(name, &Self::ids(proxy).await?),
            name: name.to_string(),
            icon: icon_of(icon),
            values: Default::default(),
        };
        for key in keys {
            p.with_key(key);
        }
        p.validate().map_err(profile_error)?;
        proxy.set_profile(&p.to_toml(), false).await?;
        Ok(())
    }

    /// Read profile `id`, let `edit` change it, and write it back.
    pub(crate) async fn edit(
        proxy: &DesktopProxy<'_>,
        id: &str,
        edit: impl FnOnce(&mut Profile),
    ) -> zbus::Result<()> {
        let mut p = Self::load(proxy, id).await?;
        edit(&mut p);
        p.validate().map_err(profile_error)?;
        proxy.set_profile(&p.to_toml(), true).await?;
        Ok(())
    }

    /// Keep an apply's or "Back to normal"'s report (`{active, kept, notices}` JSON).
    pub(crate) fn report(&self, text: &str) -> zbus::Result<()> {
        let v = parse_json(text)?;
        let kept = v["kept"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|k| k["label"].as_str().map(str::to_string))
            .collect();
        let notices = v["notices"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|n| n.as_str().map(str::to_string))
            .collect();
        let mut shown = self.shown.lock().unwrap();
        let seq = shown.next();
        shown.report = Report { seq, kept, notices };
        Ok(())
    }

    pub(crate) async fn import(
        &self,
        proxy: &DesktopProxy<'_>,
        path: &Path,
        replace: bool,
    ) -> zbus::Result<()> {
        let text = read_file(path).map_err(profile_error)?;
        match proxy.set_profile(&text, replace).await {
            Ok(_) => Ok(()),
            Err(zbus::Error::MethodError(name, text, _)) if name.as_str() == CONFLICT => {
                let mut shown = self.shown.lock().unwrap();
                let seq = shown.next();
                shown.conflict = Some(Conflict {
                    seq,
                    path: path.display().to_string(),
                    message: conflict_message(&text.unwrap_or_default()),
                });
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}
