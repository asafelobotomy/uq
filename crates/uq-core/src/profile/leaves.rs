//! A profile's keys as leaves: reading them from the state, writing them back, and their labels.

use super::*;

/// Object leaves by path segment: arrays and scalars are leaves, an empty object is none.
pub fn leaves(v: &Value) -> Vec<(Vec<String>, Value)> {
    let mut out = Vec::new();
    history::segment_leaves(v, &mut Vec::new(), &mut out);
    out
}

/// The value of one leaf as `domain` holds it in `state`; `None` when it's unset or unknown.
pub fn read_leaf(state: &State, domain: &str, path: &[String]) -> Option<Value> {
    let v = match (kind_of(domain)?, path) {
        (DomainKind::Appearance, [leaf]) if leaf == "theme" => Value::String(state.theme.clone()),
        (DomainKind::Appearance, [leaf]) if leaf == "variant" => {
            Value::String(state.variant.as_str().into())
        }
        (DomainKind::Section | DomainKind::Appearance, _) => {
            history::get_path(&state.section(domain).ok()?, path)?.clone()
        }
        (DomainKind::Rules, [id]) => Value::Bool(enabled_of(state, id)?),
        (DomainKind::Keybinds, [id]) => Value::Bool(keybinds::enabled_of(state, id)?),
        (DomainKind::Env | DomainKind::Compat | DomainKind::Autostart, _) => {
            startup::read_leaf(state, domain, path)?
        }
        _ => return None,
    };
    (!v.is_null() && !v.is_object()).then_some(v)
}

/// Write every leaf of one domain through a single `State::set_section` (`None` removes it).
/// On an error `state` is unchanged.
pub fn write_leaves(
    state: &mut State,
    domain: &str,
    leaves: &[(Vec<String>, Option<Value>)],
) -> Result<(), SettingError> {
    let kind = kind_of(domain).ok_or_else(|| SettingError::UnknownKey(domain.to_string()))?;
    if kind == DomainKind::Rules {
        return write_rule_leaves(state, leaves);
    }
    if kind == DomainKind::Keybinds {
        return keybinds::write_leaves(state, leaves);
    }
    if matches!(
        kind,
        DomainKind::Env | DomainKind::Compat | DomainKind::Autostart
    ) {
        return startup::write_leaves(state, domain, leaves);
    }
    let (mut theme, mut variant) = (None, None);
    let mut section_leaves = Vec::new();
    for (path, value) in leaves {
        let special = kind == DomainKind::Appearance
            && matches!(path.as_slice(), [p] if p == "theme" || p == "variant");
        if !special {
            section_leaves.push((path, value));
            continue;
        }
        let key = format!("{domain}.{}", path[0]);
        let text = value
            .as_ref()
            .and_then(Value::as_str)
            .ok_or_else(|| SettingError::Invalid {
                key: key.clone(),
                reason: "must be set to a name".into(),
            })?;
        if path[0] == "theme" {
            if !valid_theme_name(text) {
                return Err(SettingError::Invalid {
                    key,
                    reason: "isn't a valid theme name".into(),
                });
            }
            theme = Some(text.to_string());
        } else {
            variant = Some(match text {
                "dark" => Variant::Dark,
                "light" => Variant::Light,
                _ => {
                    return Err(SettingError::Invalid {
                        key,
                        reason: "must be dark or light".into(),
                    });
                }
            });
        }
    }
    if !matches!(kind, DomainKind::Section | DomainKind::Appearance) {
        return Err(SettingError::UnknownKey(domain.to_string()));
    }
    if !section_leaves.is_empty() {
        let mut section = state.section(domain)?;
        for (path, value) in section_leaves {
            history::put_path(&mut section, path, value.clone());
        }
        state.set_section(domain, section)?;
    }
    if let Some(t) = theme {
        state.theme = t;
    }
    if let Some(v) = variant {
        state.variant = v;
    }
    Ok(())
}

/// Set rule switches. A non-bool, a removal, or an id with no rule is refused (callers leave
/// out ids that aren't here); on an error `state` is unchanged.
pub(crate) fn write_rule_leaves(
    state: &mut State,
    leaves: &[(Vec<String>, Option<Value>)],
) -> Result<(), SettingError> {
    let mut next = state.clone();
    for (path, value) in leaves {
        let key = format!("rules.{}", plain(&path.join(".")));
        let refuse = |reason: &str| SettingError::Invalid {
            key: key.clone(),
            reason: reason.into(),
        };
        let ([id], Some(Value::Bool(on))) = (path.as_slice(), value) else {
            return Err(refuse(rules::ONLY_ON_OFF));
        };
        if !set_enabled(&mut next, id, *on) {
            return Err(refuse("isn't a rule on this computer"));
        }
    }
    *state = next;
    Ok(())
}

/// `("look", ["gaps_in"])` → "Gaps between windows". A rule switch is "A rule" here: naming it
/// needs the rules ([`rule_label`]).
pub fn key_label(domain: &str, path: &[String]) -> String {
    let dotted = path.join(".");
    match (domain, dotted.as_str()) {
        ("rules", _) if path.len() == 1 => Some("A rule".to_string()),
        ("keybinds", _) if path.len() == 1 => Some("A shortcut".to_string()),
        ("appearance", "theme" | "variant") => history::label("theme", &dotted),
        ("power", "profile") => Some("Power mode".to_string()),
        ("env" | "compat" | "autostart", _) => Some(startup::fallback_label(domain, path)),
        _ => history::label(domain, &dotted),
    }
    .unwrap_or(dotted)
}

pub fn power_label(mode: &str) -> &str {
    match mode {
        "power-saver" => "Power saver",
        "balanced" => "Balanced",
        "performance" => "Performance",
        other => other,
    }
}

/// A value as a person reads it.
pub fn value_text(domain: &str, path: &[String], v: &Value) -> String {
    if let Some(text) = startup::startup_value_text(domain, path, v) {
        return text;
    }
    match v {
        Value::String(s) if domain == "power" && path == ["profile"] => power_label(s).to_string(),
        Value::String(s) => s.clone(),
        Value::Bool(true) => "On".into(),
        Value::Bool(false) => "Off".into(),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => format!("{} items", a.len()),
        other => other.to_string(),
    }
}

pub fn power_notice(reason: &str) -> String {
    format!("Power mode wasn't changed: {reason}.")
}
