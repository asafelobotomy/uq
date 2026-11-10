//! Live outputs as Hyprland reports them (`hyprctl monitors all -j`), and the guard that a
//! stored layout leaves something on screen.

use super::{MAX_TEXT_CHARS, Monitors, clean_text as clean, lenient};

fn clean_text(s: &str) -> String {
    clean(s, MAX_TEXT_CHARS)
}
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_MODES: usize = 64;

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct LiveOutput {
    pub name: String,
    pub description: String,
    pub make: String,
    pub model: String,
    pub serial: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: f64,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub transform: u8,
    pub disabled: bool,
    pub mirror_of: Option<String>,
    pub vrr: bool,
    pub available_modes: Vec<String>,
}

/// Hyprland's shape: camelCase, `mirrorOf` is `"none"` when not mirroring.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Raw {
    #[serde(deserialize_with = "lenient")]
    name: String,
    #[serde(deserialize_with = "lenient")]
    description: String,
    #[serde(deserialize_with = "lenient")]
    make: String,
    #[serde(deserialize_with = "lenient")]
    model: String,
    #[serde(deserialize_with = "lenient")]
    serial: String,
    #[serde(deserialize_with = "lenient")]
    width: u32,
    #[serde(deserialize_with = "lenient")]
    height: u32,
    #[serde(deserialize_with = "lenient")]
    refresh_rate: f64,
    #[serde(deserialize_with = "lenient")]
    x: i32,
    #[serde(deserialize_with = "lenient")]
    y: i32,
    #[serde(deserialize_with = "lenient")]
    scale: f64,
    #[serde(deserialize_with = "lenient")]
    transform: u8,
    #[serde(deserialize_with = "lenient")]
    disabled: bool,
    #[serde(deserialize_with = "lenient")]
    mirror_of: String,
    vrr: Value,
    #[serde(deserialize_with = "lenient")]
    available_modes: Vec<Value>,
}

impl From<Raw> for LiveOutput {
    fn from(r: Raw) -> Self {
        let mirror = clean_text(&r.mirror_of);
        LiveOutput {
            name: clean_text(&r.name),
            description: clean_text(&r.description),
            make: clean_text(&r.make),
            model: clean_text(&r.model),
            serial: clean_text(&r.serial),
            width: r.width,
            height: r.height,
            refresh_rate: if r.refresh_rate.is_finite() {
                r.refresh_rate
            } else {
                0.0
            },
            x: r.x,
            y: r.y,
            scale: if r.scale.is_finite() { r.scale } else { 1.0 },
            transform: r.transform,
            disabled: r.disabled,
            mirror_of: (!mirror.is_empty() && mirror != "none").then_some(mirror),
            vrr: match &r.vrr {
                Value::Bool(b) => *b,
                Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
                _ => false,
            },
            available_modes: r
                .available_modes
                .iter()
                .filter_map(Value::as_str)
                .take(MAX_MODES)
                .map(clean_text)
                .collect(),
        }
    }
}

/// `monitors all -j` as outputs; bad JSON gives an empty list, and entries that aren't
/// objects or have no name are dropped.
pub fn parse_live(json: &str) -> Vec<LiveOutput> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter(Value::is_object)
        .filter_map(|v| serde_json::from_value::<Raw>(v).ok())
        .map(LiveOutput::from)
        .filter(|o| !o.name.is_empty())
        .collect()
}

/// At least one live output is enabled and not mirroring in `m`; an output with no entry
/// counts as enabled. Hyprland can't show a dialog on zero outputs.
pub fn stays_visible(m: &Monitors, live: &[LiveOutput]) -> bool {
    live.iter().any(|o| {
        m.outputs
            .get(&o.name)
            .is_none_or(|c| c.enabled != Some(false) && c.mirror_of.is_none())
    })
}
