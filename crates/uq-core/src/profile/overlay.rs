//! Applying a profile and turning it off again (spec §4, K3).
//!
//! `apply` and `turn_off` work on a `State` in memory and return the next one; the caller commits
//! it. A profile remembers, per key, what it replaced (`before`) and what it wrote (`set`), so
//! "Back to normal" only puts back keys that still hold what the profile set.

use super::{
    DomainKind, Profile, ProfileError, TOO_MANY_FIXES, TOO_MANY_VARS, emptied_elsewhere,
    enabled_of, key_label, kind_of, leaves, read_leaf, rule_label, rule_toggles, startup_keys,
    startup_label, write_leaves,
};
use crate::history::{self, Entry};
use crate::settings::SettingError;
use crate::state::State;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};

/// One key a profile set: what it held before (`None` = unset) and what the profile wrote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyMemo {
    pub domain: String,
    pub path: Vec<String>,
    pub before: Option<Value>,
    pub set: Value,
}

/// The power mode before the profile and the one it set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerMemo {
    pub before: String,
    pub set: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveProfile {
    pub id: String,
    pub name: String,
    pub since: i64,
    pub keys: Vec<KeyMemo>,
    pub power: Option<PowerMemo>,
}

/// A key "Back to normal" left alone because it was changed by hand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    pub domain: String,
    pub path: Vec<String>,
    pub label: String,
}

fn refused(e: SettingError) -> ProfileError {
    match e {
        SettingError::Invalid { key, reason } if key == "env" && reason == TOO_MANY_VARS => {
            ProfileError::TooManyVars
        }
        SettingError::Invalid { key, reason } if key == "compat" && reason == TOO_MANY_FIXES => {
            ProfileError::TooManyFixes
        }
        SettingError::Invalid { key, reason } => ProfileError::Invalid { key, reason },
        SettingError::UnknownKey(key) => ProfileError::Invalid {
            key,
            reason: "not a setting uQ knows".into(),
        },
    }
}

type Write = (Vec<String>, Option<Value>);

/// Write each domain's leaves with one `write_leaves` call, domains in first-seen order.
fn write_by_domain(state: &mut State, writes: Vec<(String, Write)>) -> Result<(), ProfileError> {
    let mut grouped: Vec<(String, Vec<Write>)> = Vec::new();
    for (domain, w) in writes {
        match grouped.iter_mut().find(|(d, _)| *d == domain) {
            Some((_, list)) => list.push(w),
            None => grouped.push((domain, vec![w])),
        }
    }
    for (domain, list) in grouped {
        write_leaves(state, &domain, &list).map_err(refused)?;
    }
    Ok(())
}

/// Set every non-power key of `profile` over `current`. `power` is left for the caller.
/// `known` holds this computer's startup ids (`autostart::scan::known_ids`).
pub fn apply(
    current: &State,
    profile: &Profile,
    known: &BTreeSet<String>,
    now: i64,
) -> Result<(State, ActiveProfile), ProfileError> {
    // A rule switch whose rule isn't here, or a startup app that isn't, is left out: no write
    // and no memo. So is a fix's key that would remove a fix working in other launchers.
    let kept_fixes = emptied_elsewhere(current, profile);
    let keys: Vec<_> = profile
        .keys()
        .into_iter()
        .filter(|k| match kind_of(&k.domain) {
            Some(DomainKind::Power) => false,
            Some(DomainKind::Rules) => {
                matches!(k.path.as_slice(), [id] if enabled_of(current, id).is_some())
            }
            Some(DomainKind::Autostart) => k.path.first().is_some_and(|id| known.contains(id)),
            Some(DomainKind::Compat) => {
                !matches!(k.path.as_slice(), [apps, id, _] if apps == "apps" && kept_fixes.contains(id))
            }
            _ => true,
        })
        .collect();
    let mut next = current.clone();
    write_by_domain(
        &mut next,
        keys.iter()
            .map(|k| (k.domain.clone(), (k.path.clone(), Some(k.value.clone()))))
            .collect(),
    )?;
    let mut memos = created_leaves(current, &next, &keys);
    memos.splice(
        0..0,
        keys.into_iter().map(|k| KeyMemo {
            before: read_leaf(current, &k.domain, &k.path),
            set: read_leaf(&next, &k.domain, &k.path).unwrap_or(k.value),
            domain: k.domain,
            path: k.path,
        }),
    );
    let active = ActiveProfile {
        id: profile.id.clone(),
        name: profile.name.clone(),
        since: now,
        keys: memos,
        power: None,
    };
    Ok((next, active))
}

/// Leaves the write created that the profile doesn't hold itself: the other fields of a map
/// entry it added (for example `fit` and `interval` of a new `wallpaper.outputs.<name>`), which
/// were filled in by default. They're remembered as unset, so "Back to normal" removes the whole
/// entry, and a field changed by hand since is kept (K3).
fn created_leaves(current: &State, next: &State, keys: &[super::Key]) -> Vec<KeyMemo> {
    let mut domains: Vec<&str> = Vec::new();
    for k in keys {
        if !domains.contains(&k.domain.as_str()) {
            domains.push(&k.domain);
        }
    }
    let mut out = Vec::new();
    for domain in domains {
        // A rule switch creates nothing; a startup key's memo is its own (a fix or a choice the
        // profile creates goes when its keys are put back).
        if !matches!(
            kind_of(domain),
            Some(DomainKind::Section | DomainKind::Appearance)
        ) {
            continue;
        }
        let (Ok(before), Ok(after)) = (current.section(domain), next.section(domain)) else {
            continue;
        };
        for (path, value) in leaves(&after) {
            let own = keys.iter().any(|k| k.domain == domain && k.path == path);
            if !own && history::get_path(&before, &path).is_none() {
                out.push(KeyMemo {
                    domain: domain.to_string(),
                    path,
                    before: None,
                    set: value,
                });
            }
        }
    }
    out
}

/// "Back to normal" (K3): put `before` back for keys that still hold what the profile set, keep
/// the rest. A key already back at `before` is neither written nor reported. Power isn't looked at.
pub fn turn_off(
    current: &State,
    active: &ActiveProfile,
) -> Result<(State, Vec<Kept>), ProfileError> {
    let mut writes = Vec::new();
    let mut kept = Vec::new();
    for m in &active.keys {
        let cur = read_leaf(current, &m.domain, &m.path);
        let kind = kind_of(&m.domain);
        // A rule deleted while the profile was on: nothing to put back, nothing kept.
        if kind == Some(DomainKind::Rules) && cur.is_none() {
            continue;
        }
        // Likewise an added startup entry deleted since.
        if kind == Some(DomainKind::Autostart)
            && m.path.first().is_some_and(|id| {
                !id.ends_with(".desktop") && !current.autostart.added.iter().any(|e| e.id() == id)
            })
        {
            continue;
        }
        // A created leaf can be null, which `read_leaf` reads as unset.
        let still_set = cur.as_ref() == Some(&m.set) || (m.set.is_null() && cur.is_none());
        if still_set {
            writes.push((m.domain.clone(), (m.path.clone(), m.before.clone())));
        } else if cur != m.before {
            kept.push(Kept {
                label: match (kind, m.path.as_slice()) {
                    (Some(DomainKind::Rules), [id]) => rule_label(&current.rules, id),
                    (Some(DomainKind::Env | DomainKind::Compat | DomainKind::Autostart), _) => {
                        startup_label(current, &m.domain, &m.path, &|_| None)
                    }
                    _ => key_label(&m.domain, &m.path),
                },
                domain: m.domain.clone(),
                path: m.path.clone(),
            });
        }
    }
    // Putting back what the profile removed would pass a limit: say what to do.
    let put_back = |writes: Vec<(String, Write)>| {
        let mut next = current.clone();
        write_by_domain(&mut next, writes).map_err(|e| match e {
            ProfileError::TooManyVars => ProfileError::OffTooManyVars,
            ProfileError::TooManyFixes => ProfileError::OffTooManyFixes,
            e => e,
        })?;
        Ok::<_, ProfileError>(next)
    };
    let next = put_back(writes.clone())?;
    // A fix the user has since made work in other launchers is never removed, as `apply`
    // leaves it alone (`emptied_elsewhere`): its keys are kept instead.
    let emptied: Vec<&str> = current
        .compat
        .apps
        .iter()
        .filter(|f| f.elsewhere && next.compat.fix_for(&f.id).is_none())
        .map(|f| f.id.as_str())
        .collect();
    if emptied.is_empty() {
        return Ok((next, kept));
    }
    let fix_key = |(domain, (path, _)): &(String, Write)| match path.as_slice() {
        [apps, id, _] if domain == "compat" && apps == "apps" => emptied.contains(&id.as_str()),
        _ => false,
    };
    let (skipped, writes): (Vec<_>, Vec<_>) = writes.into_iter().partition(fix_key);
    kept.extend(skipped.into_iter().map(|(domain, (path, _))| Kept {
        label: startup_label(current, &domain, &path, &|_| None),
        domain,
        path,
    }));
    Ok((put_back(writes)?, kept))
}

/// True when the profile set this key and it still has that value.
pub fn holds(active: &ActiveProfile, domain: &str, path: &[String], current: &Value) -> bool {
    active
        .keys
        .iter()
        .any(|m| m.domain == domain && m.path == path && m.set == *current)
}

/// The profile key a journal entry's leaf belongs to; `None` when a profile can't hold it.
pub fn from_journal(domain: &str, path: &[String]) -> Option<(String, Vec<String>)> {
    match (domain, path) {
        ("theme", [leaf]) if leaf == "theme" || leaf == "variant" => {
            Some(("appearance".into(), path.to_vec()))
        }
        _ => matches!(
            kind_of(domain),
            Some(DomainKind::Section | DomainKind::Appearance)
        )
        .then(|| (domain.to_string(), path.to_vec())),
    }
}

/// The profile keys changed in entries at or after `since`, in first-seen order, and how many
/// changed keys a profile can't hold.
pub fn changed_since(newest_first: &[Entry], since: i64) -> (Vec<(String, Vec<String>)>, usize) {
    let mut found: Vec<(String, Vec<String>)> = Vec::new();
    let mut skipped: HashSet<(String, Vec<String>)> = HashSet::new();
    for entry in newest_first.iter().filter(|e| e.time >= since) {
        if entry.domain == "rules" {
            // Journal leaves for rules are whole lists: only a rule's switch can be a key.
            let (toggles, other) = rule_toggles(&entry.before, &entry.after);
            for id in toggles {
                let key = ("rules".to_string(), vec![id]);
                if !found.contains(&key) {
                    found.push(key);
                }
            }
            skipped.extend(other.into_iter().map(|t| ("rules".to_string(), vec![t])));
            continue;
        }
        if matches!(entry.domain.as_str(), "env" | "compat" | "autostart") {
            // Whole lists here too: only values a profile can hold are keys.
            let (keys, other) = startup_keys(&entry.domain, &entry.before, &entry.after);
            for key in keys {
                if !found.contains(&key) {
                    found.push(key);
                }
            }
            skipped.extend(other.into_iter().map(|t| (entry.domain.clone(), vec![t])));
            continue;
        }
        let (before, after) = (leaves(&entry.before), leaves(&entry.after));
        let value_in = |all: &[(Vec<String>, Value)], p: &Vec<String>| {
            all.iter().find(|(q, _)| q == p).map(|(_, v)| v.clone())
        };
        let mut paths: Vec<&Vec<String>> = Vec::new();
        for (p, _) in before.iter().chain(after.iter()) {
            if !paths.contains(&p) {
                paths.push(p);
            }
        }
        for p in paths {
            if value_in(&before, p) == value_in(&after, p) {
                continue;
            }
            match from_journal(&entry.domain, p) {
                Some(key) if !found.contains(&key) => found.push(key),
                Some(_) => {}
                None => {
                    skipped.insert((entry.domain.clone(), p.clone()));
                }
            }
        }
    }
    (found, skipped.len())
}

/// The profile after `active` in `ids_in_order`; `None` means "back to normal".
pub fn next_after(ids_in_order: &[String], active: Option<&str>) -> Option<String> {
    match active.and_then(|a| ids_in_order.iter().position(|i| i == a)) {
        None => ids_in_order.first().cloned(),
        Some(i) => ids_in_order.get(i + 1).cloned(),
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod startup_tests;
