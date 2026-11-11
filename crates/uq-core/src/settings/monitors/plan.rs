//! The pure matcher, resolver, planner and roles over the live screens. No I/O: uqd reads
//! `monitors all` and calls these. A screen is matched by [`MonitorId`], never by connector;
//! the connector (`DP-1`) is only where it happens to be plugged in today.

use super::{
    BarShow, LiveOutput, MonitorId, MonitorProfile, Monitors, OutputConfig, Position, SavedOutput,
    SettingError, Strategy, WorkspacePlan, clean_name, clean_text, counts_in_sets, extras, invalid,
    valid_output_name, valid_profile_id,
};
use crate::rules::fresh_prefixed;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// The live outputs with the stored layout laid over them: stored position, scale and on/off
/// win (mirroring too, which `generated.lua` renders only from the stored entry), so a stored
/// value counts before Hyprland has it.
pub fn overlay(live: &[LiveOutput], stored: &Monitors) -> Vec<LiveOutput> {
    live.iter()
        .cloned()
        .map(|mut o| {
            if let Some(c) = stored.outputs.get(&o.name) {
                if let Some(p) = &c.position {
                    (o.x, o.y) = (p.x, p.y);
                }
                if let Some(s) = c.scale {
                    o.scale = s;
                }
                if let Some(on) = c.enabled {
                    o.disabled = !on;
                }
                // The stored entry decides mirroring after the reload.
                o.mirror_of = c.mirror_of.clone();
            }
            o
        })
        .collect()
}

/// The connected screens as a sorted multiset of identities, enabled or not. `FALLBACK` and
/// headless outputs are not screens.
pub fn connected_set(live: &[LiveOutput]) -> Vec<MonitorId> {
    let mut set: Vec<MonitorId> = live
        .iter()
        .filter(|o| counts_in_sets(&o.name))
        .map(MonitorId::of)
        .collect();
    set.sort();
    set
}

/// The arrangement saved for exactly the screens that are connected now.
pub fn find_arrangement<'a>(m: &'a Monitors, live: &[LiveOutput]) -> Option<&'a MonitorProfile> {
    let set = connected_set(live);
    if set.is_empty() {
        return None;
    }
    m.profiles.iter().find(|p| p.set() == set)
}

/// Give each saved screen a distinct live output with the same identity, preferring the one
/// on its old connector, then the lowest connector name. `None` when one can't be placed.
/// The result is in the saved order.
pub fn resolve(saved: &[SavedOutput], live: &[LiveOutput]) -> Option<Vec<(String, OutputConfig)>> {
    let mut free: BTreeMap<MonitorId, BTreeSet<&str>> = BTreeMap::new();
    for o in live.iter().filter(|o| counts_in_sets(&o.name)) {
        free.entry(MonitorId::of(o)).or_default().insert(&o.name);
    }
    let mut placed: Vec<Option<&str>> = vec![None; saved.len()];
    for (i, s) in saved.iter().enumerate() {
        if let Some(pool) = free.get_mut(&s.monitor)
            && pool.remove(s.hint.as_str())
        {
            placed[i] = live
                .iter()
                .find(|o| o.name == s.hint)
                .map(|o| o.name.as_str());
        }
    }
    for (i, s) in saved.iter().enumerate() {
        if placed[i].is_some() {
            continue;
        }
        let pool = free.get_mut(&s.monitor)?;
        let name = *pool.iter().next()?;
        pool.remove(name);
        placed[i] = Some(name);
    }
    Some(
        saved
            .iter()
            .zip(placed)
            .map(|(s, name)| (name.unwrap_or_default().to_string(), s.config.clone()))
            .collect(),
    )
}

/// `m` with the arrangement applied to the connected screens (R5): the resolved entries
/// replace those connectors' entries (their description refreshed from the live screen), other
/// entries are kept, and the arrangement's own planner, when it has one, replaces `m`'s.
pub fn arranged(m: &Monitors, p: &MonitorProfile, live: &[LiveOutput]) -> Option<Monitors> {
    let resolved = resolve(&p.outputs, live)?;
    // A mirror target names the connector the screen was on when saved; follow it.
    let moved: BTreeMap<String, String> = p
        .outputs
        .iter()
        .zip(&resolved)
        .filter(|(s, _)| !s.hint.is_empty())
        .map(|(s, (name, _))| (s.hint.clone(), name.clone()))
        .collect();
    let mut next = m.clone();
    for (name, cfg) in resolved {
        let mut cfg = cfg;
        if let Some(t) = cfg.mirror_of.as_deref().and_then(|t| moved.get(t)) {
            cfg.mirror_of = Some(t.clone());
        }
        if let Some(o) = live.iter().find(|o| o.name == name)
            && !o.description.is_empty()
        {
            cfg.description = Some(o.description.clone());
        }
        next.outputs.insert(name, cfg.normalised());
    }
    if let Some(w) = &p.workspaces {
        next.workspaces = w.clone();
    }
    Some(next)
}

/// The stored entry for `o`, or, with none, its live mode, position and scale.
fn config_for(m: &Monitors, o: &LiveOutput) -> Result<OutputConfig, SettingError> {
    let mut cfg = match m.outputs.get(&o.name) {
        Some(c) => c.clone(),
        None if o.width == 0 || o.height == 0 => {
            return Err(invalid(
                "monitors.profiles",
                "arrange these screens first: one has no layout yet",
            ));
        }
        None => OutputConfig {
            mode: Some(format!("{}x{}@{:.2}", o.width, o.height, o.refresh_rate)),
            position: Some(Position { x: o.x, y: o.y }),
            scale: Some(o.scale),
            enabled: o.disabled.then_some(false),
            ..OutputConfig::default()
        },
    };
    if cfg.description.is_none() && !o.description.is_empty() {
        cfg.description = Some(clean_text(&o.description, extras::MAX_DESCRIPTION));
    }
    Ok(cfg.normalised())
}

/// An arrangement of the screens connected now. One for the same set already there keeps its
/// id (and, when `name` is empty, its name, and its planner); a new one takes `id` or a fresh
/// one not in `taken`.
pub fn remember(
    m: &Monitors,
    live: &[LiveOutput],
    name: &str,
    id: Option<String>,
    taken: &BTreeSet<String>,
) -> Result<MonitorProfile, SettingError> {
    let outputs = live
        .iter()
        .filter(|o| counts_in_sets(&o.name))
        .map(|o| {
            Ok(SavedOutput {
                monitor: MonitorId::of(o),
                hint: o.name.clone(),
                config: config_for(m, o)?,
            })
        })
        .collect::<Result<Vec<_>, SettingError>>()?;
    let mut p = MonitorProfile {
        outputs,
        ..MonitorProfile::default()
    };
    let name = clean_name(name);
    match m.profiles.iter().find(|e| e.set() == p.set()) {
        Some(e) => {
            p.id = e.id.clone();
            p.name = if name.is_empty() {
                e.name.clone()
            } else {
                name
            };
            p.workspaces = e.workspaces.clone();
        }
        None => {
            if m.profiles.len() >= extras::MAX_PROFILES {
                return Err(invalid(
                    "monitors.profiles",
                    "at most 32 arrangements: remove one first",
                ));
            }
            p.id = match id {
                Some(id) if valid_profile_id(&id) => id,
                Some(_) => return Err(invalid("monitors.profiles", "not a valid arrangement id")),
                None => fresh_prefixed("m-", taken),
            };
            p.name = name;
        }
    }
    p.validate("monitors.profiles")?;
    Ok(p)
}

/// The screens to plan workspaces for: enabled, not mirroring, with the stored layout laid
/// over them, left to right, then top to bottom, then by name.
pub fn screens(m: &Monitors, live: &[LiveOutput]) -> Vec<LiveOutput> {
    let mut s: Vec<LiveOutput> = overlay(live, m)
        .into_iter()
        .filter(|o| {
            !o.disabled
                && o.mirror_of.is_none()
                && counts_in_sets(&o.name)
                && valid_output_name(&o.name)
        })
        .collect();
    s.sort_by(|a, b| (a.x, a.y, &a.name).cmp(&(b.x, b.y, &b.name)));
    s
}

/// One workspace the planner puts on one screen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Planned {
    pub workspace: u8,
    pub output: String,
    /// The first workspace of the screen's share: the one it opens with.
    pub default: bool,
}

/// Where each workspace goes. Nothing with fewer than two screens or with the planner off.
pub fn plan(p: &WorkspacePlan, screens: &[LiveOutput]) -> Vec<Planned> {
    let k = screens.len();
    if p.strategy == Strategy::Off || k < 2 {
        return Vec::new();
    }
    let n = usize::from(p.count);
    let mut owner: Vec<(u8, usize)> = Vec::new();
    match p.strategy {
        Strategy::Off => {}
        Strategy::Sequential => {
            let (base, extra) = (n / k, n % k);
            let mut ws = 1u8;
            for i in 0..k {
                for _ in 0..base + usize::from(i < extra) {
                    owner.push((ws, i));
                    ws += 1;
                }
            }
        }
        Strategy::Interleave => owner.extend((1..=p.count).map(|w| (w, usize::from(w - 1) % k))),
        Strategy::Manual => {
            let mut slots: Vec<_> = p.manual.iter().collect();
            slots.sort_by_key(|s| s.workspace);
            owner.extend(slots.into_iter().filter_map(|s| {
                let i = screens.iter().position(|o| MonitorId::of(o) == s.monitor)?;
                Some((s.workspace, i))
            }));
        }
    }
    let mut seen = BTreeSet::new();
    owner
        .into_iter()
        .map(|(workspace, i)| Planned {
            workspace,
            output: screens[i].name.clone(),
            default: p.strategy != Strategy::Manual && seen.insert(i),
        })
        .collect()
}

/// The main screen when none is chosen: the enabled output at 0,0 (the first by name if
/// several sit there), else the first enabled output by name. `None` with no enabled output.
pub fn fallback_main(live: &[LiveOutput]) -> Option<&LiveOutput> {
    fn first<'a>(it: impl Iterator<Item = &'a LiveOutput>) -> Option<&'a LiveOutput> {
        it.min_by(|a, b| a.name.cmp(&b.name))
    }
    let on = || live.iter().filter(|o| !o.disabled);
    first(on().filter(|o| o.x == 0 && o.y == 0)).or_else(|| first(on()))
}

/// The chosen main display when it is connected, on and not mirroring.
fn chosen<'a>(m: &Monitors, live: &'a [LiveOutput]) -> Option<&'a LiveOutput> {
    let id = m.primary.as_ref()?;
    live.iter()
        .filter(|o| !o.disabled && o.mirror_of.is_none() && counts_in_sets(&o.name))
        .filter(|o| &MonitorId::of(o) == id)
        .min_by(|a, b| a.name.cmp(&b.name))
}

/// The main display: the chosen one when it is there, else [`fallback_main`].
pub fn main_output<'a>(m: &Monitors, live: &'a [LiveOutput]) -> Option<&'a LiveOutput> {
    chosen(m, live).or_else(|| fallback_main(live))
}

/// What the shell and uqd call each screen (connector names only).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Roles {
    /// The chosen main display's connector; empty unless one is chosen and found.
    pub primary: String,
    /// The main display's connector with the fallback rule; empty with no screen.
    pub main: String,
    /// The connectors that get a bar; `None` means every screen. Never an empty list.
    pub bar: Option<Vec<String>>,
}

pub fn roles(m: &Monitors, live: &[LiveOutput]) -> Roles {
    let main = main_output(m, live).map(|o| o.name.clone());
    let bar = match m.bar.show {
        BarShow::All => None,
        BarShow::Main => main.clone().map(|n| vec![n]),
        BarShow::Chosen => {
            let on: Vec<String> = live
                .iter()
                .filter(|o| {
                    !o.disabled
                        && o.mirror_of.is_none()
                        && counts_in_sets(&o.name)
                        && m.bar.on.contains(&MonitorId::of(o))
                })
                .map(|o| o.name.clone())
                .collect();
            (!on.is_empty()).then_some(on)
        }
    };
    Roles {
        primary: chosen(m, live).map(|o| o.name.clone()).unwrap_or_default(),
        main: main.unwrap_or_default(),
        bar,
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
