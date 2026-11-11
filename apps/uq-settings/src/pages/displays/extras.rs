//! The Displays page's main display, bar, saved arrangements and workspace planner: what the
//! page asks for ([`apply`]) and what it shows about them ([`snapshot_parts`]).
//!
//! Every write but "remember" is one `SetSection("monitors", …)` of the stored section with the
//! one change, checked here first so a refusal reads in plain words. The layout (`outputs`) is
//! never part of these writes: it changes only through apply and keep. Output names come from
//! the page; each is turned into the screen's identity (make, model, serial) from the live
//! list, because the connector (`DP-1`) moves between ports.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};
use uq_core::client::{self, DesktopProxy};
use uq_core::settings::{
    BarShow, LiveOutput, ManualSlot, MonitorId, Monitors, Strategy, clean_name, counts_in_sets,
    plan,
};

use super::{Op, Outputs, refuse};
use crate::pages::runner::{ORIGIN, message, plain_refusal};

const NOT_CONNECTED: &str = "That display isn't connected";
pub(super) const NOT_THERE: &str = "That arrangement isn't there any more.";
pub(super) const NEEDS_NAME: &str = "Give this arrangement a name.";

/// The live outputs `ListOutputs` sent, as the core functions take them.
pub(super) fn live_outputs(live: &[Value]) -> Vec<LiveOutput> {
    let text = |o: &Value, k: &str| o[k].as_str().unwrap_or_default().to_string();
    live.iter()
        .map(|o| LiveOutput {
            name: text(o, "name"),
            description: text(o, "description"),
            make: text(o, "make"),
            model: text(o, "model"),
            serial: text(o, "serial"),
            width: o["width"].as_u64().unwrap_or(0) as u32,
            height: o["height"].as_u64().unwrap_or(0) as u32,
            refresh_rate: o["refresh_rate"].as_f64().unwrap_or(0.0),
            x: o["x"].as_i64().unwrap_or(0) as i32,
            y: o["y"].as_i64().unwrap_or(0) as i32,
            scale: o["scale"].as_f64().unwrap_or(1.0),
            transform: o["transform"].as_u64().unwrap_or(0) as u8,
            disabled: o["disabled"].as_bool().unwrap_or(false),
            mirror_of: o["mirror_of"].as_str().map(str::to_string),
            vrr: o["vrr"].as_bool().unwrap_or(false),
            available_modes: Vec::new(),
        })
        .collect()
}

/// What a screen is called on the page: its description, else its connector.
fn label_of(o: &LiveOutput) -> String {
    if o.description.is_empty() {
        o.name.clone()
    } else {
        o.description.clone()
    }
}

/// The identity of the connected screen named `name`.
fn id_of(live: &[LiveOutput], name: &str) -> zbus::Result<MonitorId> {
    live.iter()
        .find(|o| o.name == name && counts_in_sets(&o.name))
        .map(MonitorId::of)
        .ok_or_else(|| zbus::Error::Failure(NOT_CONNECTED.into()))
}

/// The identities of the connected screens named in `names`, each once.
fn ids_of(live: &[LiveOutput], names: &[String]) -> zbus::Result<Vec<MonitorId>> {
    let mut seen = BTreeSet::new();
    for n in names {
        seen.insert(id_of(live, n)?);
    }
    Ok(seen.into_iter().collect())
}

fn parse_bar(show: &str) -> zbus::Result<BarShow> {
    match show {
        "all" => Ok(BarShow::All),
        "main" => Ok(BarShow::Main),
        "chosen" => Ok(BarShow::Chosen),
        _ => refuse("Choose where the bar shows."),
    }
}

fn bar_name(show: BarShow) -> &'static str {
    match show {
        BarShow::All => "all",
        BarShow::Main => "main",
        BarShow::Chosen => "chosen",
    }
}

fn parse_strategy(strategy: &str) -> zbus::Result<Strategy> {
    match strategy {
        "off" => Ok(Strategy::Off),
        "sequential" => Ok(Strategy::Sequential),
        "interleave" => Ok(Strategy::Interleave),
        "manual" => Ok(Strategy::Manual),
        _ => refuse("Choose how to spread the workspaces."),
    }
}

fn strategy_name(s: Strategy) -> &'static str {
    match s {
        Strategy::Off => "off",
        Strategy::Sequential => "sequential",
        Strategy::Interleave => "interleave",
        Strategy::Manual => "manual",
    }
}

/// Do `op`, one of the ops this file owns; any other is left to the caller.
pub(super) async fn apply(proxy: &DesktopProxy<'_>, o: &Outputs, op: Op) -> zbus::Result<()> {
    let live = live_outputs(&o.live);
    let mut m = o.stored.clone();
    match op {
        Op::Remember { name } => return remember(proxy, &name).await,
        Op::SetMain { output } => {
            m.primary = output.map(|n| id_of(&live, &n)).transpose()?;
        }
        Op::SetBar { show, outputs } => {
            m.bar.show = parse_bar(&show)?;
            let on = ids_of(&live, &outputs)?;
            if m.bar.show == BarShow::Chosen && on.is_empty() {
                return refuse("Choose at least one display for the bar.");
            }
            if !on.is_empty() {
                // Screens that aren't plugged in keep their place in the choice.
                let mut kept: Vec<MonitorId> = m
                    .bar
                    .on
                    .iter()
                    .filter(|id| !live.iter().any(|o| &MonitorId::of(o) == *id))
                    .cloned()
                    .collect();
                kept.extend(on);
                m.bar.on = kept;
            }
        }
        Op::SetAutoSwitch(on) => m.auto_profiles = on,
        Op::SetPlan {
            strategy,
            count,
            move_back,
        } => {
            if let Some(s) = m.workspaces.manual.iter().find(|s| s.workspace > count) {
                return refuse(format!(
                    "Workspace {} is placed on a display. Set it to Not chosen first, or keep {} workspaces.",
                    s.workspace, s.workspace
                ));
            }
            m.workspaces.strategy = parse_strategy(&strategy)?;
            m.workspaces.count = count;
            m.workspaces.move_back = move_back;
        }
        Op::SetManual { workspace, output } => {
            let monitor = output.map(|n| id_of(&live, &n)).transpose()?;
            m.workspaces.manual.retain(|s| s.workspace != workspace);
            if let Some(monitor) = monitor {
                m.workspaces.manual.push(ManualSlot { workspace, monitor });
            }
            m.workspaces.manual.sort_by_key(|s| s.workspace);
        }
        Op::Rename { id, name } => {
            let name = clean_name(&name);
            if name.is_empty() {
                return refuse(NEEDS_NAME);
            }
            let Some(p) = m.profiles.iter_mut().find(|p| p.id == id) else {
                return refuse(NOT_THERE);
            };
            p.name = name;
        }
        Op::Forget { id } => {
            let before = m.profiles.len();
            m.profiles.retain(|p| p.id != id);
            if m.profiles.len() == before {
                return refuse(NOT_THERE);
            }
        }
        _ => return Ok(()),
    }
    write(proxy, &m).await
}

/// `SetSection("monitors", m)`, checked first. uqd's own refusal loses its key.
async fn write(proxy: &DesktopProxy<'_>, m: &Monitors) -> zbus::Result<()> {
    let section = serde_json::to_value(m).expect("monitors serialise");
    if let Err(e) = Monitors::from_section(&section) {
        return refuse(plain_refusal(&e.to_string()));
    }
    proxy
        .set_section("monitors", &section.to_string(), ORIGIN)
        .await
        .map_err(plain_error)
}

/// Save the screens connected now, with their layout, as an arrangement called `name`.
async fn remember(proxy: &DesktopProxy<'_>, name: &str) -> zbus::Result<()> {
    let name = clean_name(name);
    if name.is_empty() {
        return refuse(NEEDS_NAME);
    }
    proxy
        .remember_monitors(&name, ORIGIN)
        .await
        .map(drop)
        .map_err(plain_error)
}

/// A refusal from uqd without the key it names; a missing service stays as it is.
fn plain_error(e: zbus::Error) -> zbus::Error {
    if client::no_owner(&e) {
        e
    } else {
        zbus::Error::Failure(plain_refusal(&message(&e)))
    }
}

/// A saved screen as the page names it: the description recorded with it, else make and model.
fn saved_label(s: &uq_core::settings::SavedOutput) -> String {
    s.config
        .description
        .clone()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| s.monitor.label())
}

/// What the page shows about the roles, the planner and the saved arrangements. `raw` is the
/// `ListOutputs` answer (its `arrangement` says whether the connected set is saved, whether that
/// is what is in place, and whether the last automatic switch failed).
pub(super) fn snapshot_parts(raw: &Value, stored: &Monitors, live: &[Value]) -> Map<String, Value> {
    let live = live_outputs(live);
    let label_for = |id: &MonitorId| {
        live.iter()
            .find(|o| &MonitorId::of(o) == id)
            .map_or_else(|| id.label(), label_of)
    };

    // The main display: the connector it is on today, if it is there.
    let primary = raw["roles"]["primary"].as_str().filter(|n| !n.is_empty());
    let main = stored.primary.as_ref().map(|id| {
        json!({
            "output": primary,
            "label": label_for(id),
            "connected": primary.is_some(),
        })
    });

    let chosen: Vec<&str> = live
        .iter()
        .filter(|o| stored.bar.on.contains(&MonitorId::of(o)))
        .map(|o| o.name.as_str())
        .collect();
    let bar = json!({"show": bar_name(stored.bar.show), "outputs": chosen});

    // The planner, previewed over the screens it would use.
    let screens = plan::screens(stored, &live);
    let planned = plan::plan(&stored.workspaces, &screens);
    let preview: Vec<Value> = screens
        .iter()
        .filter_map(|s| {
            let mut ws: Vec<u8> = planned
                .iter()
                .filter(|p| p.output == s.name)
                .map(|p| p.workspace)
                .collect();
            ws.sort_unstable();
            (!ws.is_empty())
                .then(|| json!({"output": s.name, "label": label_of(s), "workspaces": ws}))
        })
        .collect();
    let manual: Vec<Value> = stored
        .workspaces
        .manual
        .iter()
        .map(|s| {
            let output = live
                .iter()
                .find(|o| MonitorId::of(o) == s.monitor && counts_in_sets(&o.name))
                .map(|o| o.name.clone());
            json!({"workspace": s.workspace, "output": output, "label": label_for(&s.monitor)})
        })
        .collect();
    let screen_list: Vec<Value> = screens
        .iter()
        .map(|s| json!({"output": s.name, "label": label_of(s)}))
        .collect();
    let workspaces = json!({
        "strategy": strategy_name(stored.workspaces.strategy),
        "count": stored.workspaces.count,
        "move_back": stored.workspaces.move_back,
        "manual": manual,
        "screens": screen_list,
        "preview": preview,
    });

    // Saved arrangements, and whether to offer saving the screens connected now.
    let found = raw["arrangement"]
        .as_object()
        .filter(|_| !raw["arrangement"].is_null());
    let current = found.and_then(|a| a["id"].as_str());
    let arrangements: Vec<Value> = stored
        .profiles
        .iter()
        .map(|p| {
            let screens: Vec<String> = p.outputs.iter().map(saved_label).collect();
            json!({"id": p.id, "name": p.name, "screens": screens, "current": current == Some(p.id.as_str())})
        })
        .collect();
    let waiting = !raw["pending"].is_null();
    let any_screen = live.iter().any(|o| counts_in_sets(&o.name));
    let offer = match found {
        _ if waiting || !any_screen => None,
        None => Some("remember"),
        Some(a) if a["applied"].as_bool() == Some(false) => Some("update"),
        Some(_) => None,
    };
    let offer_name = found
        .filter(|_| offer == Some("update"))
        .and_then(|a| a["name"].as_str());
    let failed = found.is_some_and(|a| a["failed"].as_bool() == Some(true));

    let mut parts = Map::new();
    parts.insert("main".into(), main.unwrap_or(Value::Null));
    parts.insert("bar".into(), bar);
    parts.insert("auto_switch".into(), json!(stored.auto_profiles));
    parts.insert("plan".into(), workspaces);
    parts.insert("arrangements".into(), json!(arrangements));
    parts.insert("offer".into(), json!(offer));
    parts.insert("offer_name".into(), json!(offer_name));
    parts.insert("arrangement_failed".into(), json!(failed));
    parts
}
