//! The Displays page's logic: [`DisplaysState`] keeps a draft layout, applies it through uqd's
//! apply / keep / revert calls, and is plain Rust tested against the fake uqd.
//! `crate::displays_model` is the thin QML object around it.
//!
//! Nothing here saves a layout directly: uqd stores a display change only when the user keeps
//! it (`ConfirmMonitors`), and puts the old one back by itself otherwise.

pub mod arrange;
pub mod extras;
#[cfg(test)]
mod extras_tests;
#[cfg(test)]
mod tests;

use std::sync::Mutex as StdMutex;

use serde_json::{Map, Value, json};
use tokio::sync::Mutex;
use uq_core::client::DesktopProxy;
use uq_core::settings::{Monitors, OutputConfig, Position};

use crate::pages::runner::{ORIGIN, PageLogic, parse_json};
use arrange::{Rect, logical_size, normalise, parse_mode, place, scale_choices};

/// Shown after uqd put the old layout back because nobody kept the new one.
pub const RESTORED: &str = "The previous display settings were restored.";

/// What the page asks for.
#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    /// Change one field of one output in the draft (`null` = Hyprland's default).
    Edit {
        output: String,
        field: String,
        value: Value,
    },
    /// Drop an output at `(x, y)` on the arrangement, in logical pixels.
    Move {
        output: String,
        x: i32,
        y: i32,
    },
    /// Forget the draft.
    Discard,
    /// Try the draft live; uqd reverts it unless it is kept.
    Apply,
    /// Keep the change being tried.
    Keep,
    /// Put the old layout back now; the draft stays, to be fixed.
    Revert,
    /// Choose the main display by its connector name (`None` = none chosen).
    #[cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 12"))]
    SetMain {
        output: Option<String>,
    },
    /// Where the bar shows: `all`, `main` or `chosen` (then on the named displays).
    #[cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 12"))]
    SetBar {
        show: String,
        outputs: Vec<String>,
    },
    /// Switch to a saved arrangement when the screens change.
    #[cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 12"))]
    SetAutoSwitch(bool),
    /// How workspaces spread over the screens: `off`, `sequential`, `interleave` or `manual`.
    #[cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 12"))]
    SetPlan {
        strategy: String,
        count: u8,
        move_back: bool,
    },
    /// Put one workspace on a display, or back to no choice (`None`).
    #[cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 12"))]
    SetManual {
        workspace: u8,
        output: Option<String>,
    },
    /// Save the screens connected now, with their layout, under `name`.
    #[cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 12"))]
    Remember {
        name: String,
    },
    #[cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 12"))]
    Rename {
        id: String,
        name: String,
    },
    #[cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 12"))]
    Forget {
        id: String,
    },
}

pub struct DisplaysState {
    conn: zbus::Connection,
    write: Mutex<()>,
    /// The layout being edited; `None` means the stored one.
    draft: StdMutex<Option<Monitors>>,
    /// The token of the apply this page made and hasn't kept or reverted.
    awaiting: StdMutex<Option<String>>,
}

impl DisplaysState {
    pub fn new(conn: zbus::Connection) -> Self {
        Self {
            conn,
            write: Mutex::new(()),
            draft: StdMutex::new(None),
            awaiting: StdMutex::new(None),
        }
    }
}

fn refuse<T>(text: impl Into<String>) -> zbus::Result<T> {
    Err(zbus::Error::Failure(text.into()))
}

/// The fields the page edits: every [`OutputConfig`] field but the recorded model.
fn editable(field: &str) -> bool {
    field != "description" && OutputConfig::FIELDS.contains(&field)
}

/// `ListOutputs`, read.
struct Outputs {
    live: Vec<Value>,
    stored: Monitors,
    pending: Option<String>,
    raw: Value,
}

async fn outputs(proxy: &DesktopProxy<'_>) -> zbus::Result<Outputs> {
    let raw = parse_json(&proxy.list_outputs().await?)?;
    let stored = serde_json::from_value(raw["stored"].clone()).unwrap_or_default();
    let live = raw["live"].as_array().cloned().unwrap_or_default();
    let pending = raw["pending"]["token"].as_str().map(str::to_string);
    Ok(Outputs {
        live,
        stored,
        pending,
        raw,
    })
}

/// One output as it would be with `cfg`: size, place, on or off. Unset values are the live ones.
fn rect(live: &Value, cfg: Option<&OutputConfig>) -> Rect {
    let name = live["name"].as_str().unwrap_or_default().to_string();
    let lw = live["width"].as_u64().unwrap_or(1920) as u32;
    let lh = live["height"].as_u64().unwrap_or(1080) as u32;
    let (mw, mh) = cfg
        .and_then(|c| c.mode.as_deref())
        .and_then(parse_mode)
        .map_or((lw, lh), |(w, h, _)| (w, h));
    let scale = cfg
        .and_then(|c| c.scale)
        .unwrap_or_else(|| live["scale"].as_f64().unwrap_or(1.0));
    let transform = cfg
        .and_then(|c| c.transform)
        .unwrap_or_else(|| live["transform"].as_u64().unwrap_or(0) as u8);
    let (w, h) = logical_size(mw, mh, scale, transform);
    let (x, y) = cfg.and_then(|c| c.position).map_or(
        (
            live["x"].as_i64().unwrap_or(0) as i32,
            live["y"].as_i64().unwrap_or(0) as i32,
        ),
        |p| (p.x, p.y),
    );
    Rect { name, x, y, w, h }
}

/// The sizes an output offers, largest first, each with its refresh rates (highest first).
fn modes(live: &Value) -> Vec<Value> {
    let mut sizes: Vec<((u32, u32), Vec<f64>)> = Vec::new();
    for m in live["available_modes"].as_array().into_iter().flatten() {
        let Some((w, h, rate)) = m.as_str().and_then(parse_mode) else {
            continue;
        };
        let rates = match sizes.iter_mut().find(|(s, _)| *s == (w, h)) {
            Some((_, r)) => r,
            None => {
                sizes.push(((w, h), Vec::new()));
                &mut sizes.last_mut().expect("just pushed").1
            }
        };
        if let Some(r) = rate.map(|r| (r * 100.0).round() / 100.0)
            && !rates.contains(&r)
        {
            rates.push(r);
        }
    }
    sizes.sort_by_key(|((w, h), _)| std::cmp::Reverse(u64::from(*w) * u64::from(*h)));
    sizes
        .into_iter()
        .map(|((w, h), mut rates)| {
            rates.sort_by(|a, b| b.total_cmp(a));
            json!({"w": w, "h": h, "rates": rates, "scale_choices": scale_choices(w, h)})
        })
        .collect()
}

/// Every live output's place in `layout`, as the arrangement shows it.
fn view(live: &[Value], layout: &Monitors) -> Vec<Value> {
    live.iter()
        .map(|o| {
            let name = o["name"].as_str().unwrap_or_default();
            let cfg = layout.outputs.get(name);
            let r = rect(o, cfg);
            let description = o["description"].as_str().unwrap_or_default();
            let label = if description.is_empty() {
                name
            } else {
                description
            };
            json!({
                "name": name,
                "label": label,
                "description": description,
                "rect": {"x": r.x, "y": r.y, "w": r.w, "h": r.h},
                "enabled": cfg.and_then(|c| c.enabled).unwrap_or(true),
                "mirror_of": cfg.and_then(|c| c.mirror_of.clone()),
                "config": cfg.map_or(json!({}), |c| serde_json::to_value(c).unwrap_or_default()),
                "modes": modes(o),
                "live_mode": format!(
                    "{}x{}@{:.2}",
                    o["width"].as_u64().unwrap_or(0),
                    o["height"].as_u64().unwrap_or(0),
                    o["refresh_rate"].as_f64().unwrap_or(0.0)
                ),
                "live_scale": o["scale"].as_f64().unwrap_or(1.0),
            })
        })
        .collect()
}

impl DisplaysState {
    fn draft_or(&self, stored: &Monitors) -> Monitors {
        self.draft
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| stored.clone())
    }

    fn edit(&self, stored: &Monitors, output: &str, field: &str, value: Value) -> zbus::Result<()> {
        let mut layout = serde_json::to_value(self.draft_or(stored)).expect("monitors serialise");
        let outputs = layout["outputs"]
            .as_object_mut()
            .expect("monitors has outputs");
        let entry = outputs
            .entry(output.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        let entry = entry.as_object_mut().expect("an output entry is an object");
        if value.is_null() {
            entry.remove(field);
        } else {
            entry.insert(field.to_string(), value);
        }
        if entry.is_empty() {
            outputs.remove(output);
        }
        match Monitors::from_section(&layout) {
            Ok(m) => {
                *self.draft.lock().unwrap() = Some(m);
                Ok(())
            }
            Err(e) => refuse(e.to_string()),
        }
    }

    fn move_output(&self, o: &Outputs, output: &str, x: i32, y: i32) -> zbus::Result<()> {
        let mut layout = self.draft_or(&o.stored);
        let rects: Vec<Rect> = o
            .live
            .iter()
            .filter(|l| {
                let name = l["name"].as_str().unwrap_or_default();
                layout
                    .outputs
                    .get(name)
                    .is_none_or(|c| c.enabled != Some(false) && c.mirror_of.is_none())
            })
            .map(|l| {
                rect(
                    l,
                    layout.outputs.get(l["name"].as_str().unwrap_or_default()),
                )
            })
            .collect();
        let Some(moving) = rects.iter().find(|r| r.name == output).cloned() else {
            return refuse(format!("{output} isn't a display that's on"));
        };
        let mut others: Vec<Rect> = rects.into_iter().filter(|r| r.name != output).collect();
        let (nx, ny) = place(&others, &moving, x, y);
        others.push(Rect {
            x: nx,
            y: ny,
            ..moving
        });
        normalise(&mut others);
        for r in others {
            layout.outputs.entry(r.name).or_default().position = Some(Position { x: r.x, y: r.y });
        }
        *self.draft.lock().unwrap() = Some(layout);
        Ok(())
    }
}

impl PageLogic for DisplaysState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        if let Op::Edit { field, .. } = &op
            && !editable(field)
        {
            return refuse(format!("unknown field {field:?}"));
        }
        if let Op::Discard = op {
            *self.draft.lock().unwrap() = None;
            return Ok(());
        }
        let proxy = DesktopProxy::new(&self.conn).await?;
        let o = outputs(&proxy).await?;
        // uqd put the old layout back on its own: say so once.
        let lapsed = {
            let mut awaiting = self.awaiting.lock().unwrap();
            let lapsed = awaiting
                .as_ref()
                .is_some_and(|t| o.pending.as_ref() != Some(t));
            if lapsed {
                *awaiting = None;
            }
            lapsed
        };
        if lapsed && matches!(op, Op::Refresh) {
            return refuse(RESTORED);
        }
        match op {
            Op::Refresh | Op::Discard => Ok(()),
            Op::Edit {
                output,
                field,
                value,
            } => self.edit(&o.stored, &output, &field, value),
            Op::Move { output, x, y } => self.move_output(&o, &output, x, y),
            Op::Apply => {
                let layout = self.draft_or(&o.stored);
                let json = serde_json::to_string(&layout).expect("monitors serialise");
                let token = proxy.apply_monitors(&json).await?;
                *self.awaiting.lock().unwrap() = Some(token);
                Ok(())
            }
            Op::Keep => {
                let Some(token) = o.pending else {
                    return refuse(RESTORED);
                };
                proxy.confirm_monitors(&token, ORIGIN).await?;
                *self.draft.lock().unwrap() = None;
                *self.awaiting.lock().unwrap() = None;
                Ok(())
            }
            Op::Revert => {
                *self.awaiting.lock().unwrap() = None;
                match o.pending {
                    Some(token) => proxy.revert_monitors(&token).await,
                    None => Ok(()),
                }
            }
            other => extras::apply(&proxy, &o, other).await,
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let o = outputs(&proxy).await?;
        let draft = self.draft.lock().unwrap().clone();
        let dirty = draft.as_ref().is_some_and(|d| *d != o.stored);
        let shown = draft.clone().unwrap_or_else(|| o.stored.clone());
        let mut state = json!({
            "live": o.raw["live"],
            "stored": o.raw["stored"],
            "pending": o.raw["pending"],
            "draft": draft,
            "dirty": dirty,
            "view": view(&o.live, &shown),
        });
        if let Some(map) = state.as_object_mut() {
            map.extend(extras::snapshot_parts(&o.raw, &o.stored, &o.live));
        }
        Ok(state)
    }
}
