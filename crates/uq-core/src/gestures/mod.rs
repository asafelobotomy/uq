//! Touchpad gestures (slice F §7.2): the model, uQ's defaults, validation and conflicts.
//! They live in the `input` section (`input.gestures`, `input.swipe`) and render as
//! `hl.gesture` calls and the `gestures` option table. Every Hyprland spelling is in
//! [`hypr`]; shortcut-style conflict messages come from [`crate::keybinds::conflict`].

use crate::keybinds::conflict::{Conflict, find, message};
use crate::keybinds::hypr::{MODIFIERS, REFUSED_MODIFIERS};
use crate::settings::{SettingError, check_known, invalid, lenient, round2};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::borrow::Cow;
use std::fmt;

pub mod hypr;
mod summary;
pub use summary::summary;

#[cfg(test)]
mod tests;

pub const MAX_GESTURES: usize = 24;
/// The special workspace `base.lua`'s SUPER+S uses; the default name for the `special` action.
pub const DEFAULT_SPECIAL: &str = "magic";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Horizontal,
    Vertical,
    Left,
    Right,
    Up,
    Down,
    PinchIn,
    PinchOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    #[serde(rename = "workspace")]
    Workspace,
    #[serde(rename = "move")]
    Move,
    #[serde(rename = "resize")]
    Resize,
    #[serde(rename = "special")]
    Special,
    #[serde(rename = "close")]
    Close,
    #[serde(rename = "fullscreen")]
    Fullscreen,
    #[serde(rename = "float")]
    Float,
    #[serde(rename = "uq:spotlight")]
    Spotlight,
    #[serde(rename = "uq:quick-settings")]
    QuickSettings,
}

/// The directions a direction stands for once an axis is split: Hyprland refuses a gesture
/// whose direction is one of an earlier gesture's (same fingers and modifier), or its axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Atom {
    Left,
    Right,
    Up,
    Down,
    PinchIn,
    PinchOut,
}

impl Direction {
    pub fn atoms(self) -> &'static [Atom] {
        match self {
            Direction::Horizontal => &[Atom::Left, Atom::Right],
            Direction::Vertical => &[Atom::Up, Atom::Down],
            Direction::Left => &[Atom::Left],
            Direction::Right => &[Atom::Right],
            Direction::Up => &[Atom::Up],
            Direction::Down => &[Atom::Down],
            Direction::PinchIn => &[Atom::PinchIn],
            Direction::PinchOut => &[Atom::PinchOut],
        }
    }
}

/// A set of modifier keys (SUPER, SHIFT, CTRL, ALT), as Hyprland's bits. Reads and writes the
/// names from `keybinds::hypr::MODIFIERS`, in that order, space-separated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Mods(pub u8);

impl Mods {
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn names(self) -> Vec<&'static str> {
        MODIFIERS
            .iter()
            .filter(|(_, bit, _)| self.0 & bit != 0)
            .map(|(n, _, _)| *n)
            .collect()
    }

    /// Strict: only the names and aliases uQ offers; `+` or spaces between them.
    pub fn parse(s: &str) -> Result<Mods, String> {
        let mut bits = 0;
        for word in s.split(|c: char| c.is_whitespace() || c == '+') {
            if word.is_empty() {
                continue;
            }
            let up = word.to_uppercase();
            if REFUSED_MODIFIERS.contains(&up.as_str()) {
                return Err(format!("{word} isn't a key a gesture can use"));
            }
            match MODIFIERS
                .iter()
                .find(|(n, _, aliases)| *n == up || aliases.contains(&up.as_str()))
            {
                Some((_, bit, _)) => bits |= bit,
                None => return Err(format!("{word} isn't a modifier key")),
            }
        }
        Ok(Mods(bits))
    }
}

impl fmt::Display for Mods {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.names().join(" "))
    }
}

impl Serialize for Mods {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Mods {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Mods::parse(&s).map_err(serde::de::Error::custom)
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gesture {
    pub fingers: u8,
    pub direction: Direction,
    pub action: Action,
    #[serde(default, skip_serializing_if = "Mods::is_empty")]
    pub modifier: Mods,
    /// The special workspace's name, for `special` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// `[A-Za-z0-9_-]{1,32}`
pub fn valid_workspace_name(s: &str) -> bool {
    (1..=32).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

impl Gesture {
    pub const FIELDS: [&str; 6] = [
        "fingers",
        "direction",
        "action",
        "modifier",
        "workspace",
        "enabled",
    ];

    pub fn action_label(&self) -> &'static str {
        hypr::action(self.action).label
    }

    /// "3 fingers, swipe sideways", "SUPER + 4 fingers, swipe left".
    pub fn describe(&self) -> String {
        let mods = if self.modifier.is_empty() {
            String::new()
        } else {
            format!("{} + ", self.modifier.names().join(" + "))
        };
        format!(
            "{mods}{} fingers, {}",
            self.fingers,
            hypr::direction(self.direction).label
        )
    }

    /// The special workspace's name as rendered.
    pub fn special_name(&self) -> &str {
        self.workspace.as_deref().unwrap_or(DEFAULT_SPECIAL)
    }

    /// Strict parse of one entry written through the API.
    pub fn from_value(key: &str, v: &Value) -> Result<Gesture, SettingError> {
        if !v.is_object() {
            return Err(invalid(key, "a gesture is an object"));
        }
        check_known(key, v, &Self::FIELDS)?;
        let at = |f: &str| format!("{key}.{f}");
        let get = |f: &str| v.get(f).filter(|x| !x.is_null());
        let fingers = get("fingers")
            .and_then(Value::as_u64)
            .and_then(|n| u8::try_from(n).ok())
            .ok_or_else(|| invalid(&at("fingers"), "fingers is a whole number from 3 to 5"))?;
        let direction: Direction = serde_json::from_value(
            get("direction")
                .cloned()
                .ok_or_else(|| invalid(&at("direction"), "a gesture needs a direction"))?,
        )
        .map_err(|_| invalid(&at("direction"), "not a direction uQ knows"))?;
        let action: Action = serde_json::from_value(
            get("action")
                .cloned()
                .ok_or_else(|| invalid(&at("action"), "a gesture needs an action"))?,
        )
        .map_err(|_| invalid(&at("action"), "not an action uQ knows"))?;
        let modifier = match get("modifier") {
            None => Mods::default(),
            Some(Value::String(s)) => Mods::parse(s).map_err(|e| invalid(&at("modifier"), e))?,
            Some(_) => return Err(invalid(&at("modifier"), "modifier is text")),
        };
        let workspace = match get("workspace") {
            None => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(_) => return Err(invalid(&at("workspace"), "the workspace name is text")),
        };
        let enabled = match get("enabled") {
            None => true,
            Some(Value::Bool(b)) => *b,
            Some(_) => return Err(invalid(&at("enabled"), "enabled is true or false")),
        };
        let g = Gesture {
            fingers,
            direction,
            action,
            modifier,
            workspace,
            enabled,
        };
        g.validate(key)?;
        Ok(g)
    }

    /// The rules for one gesture; `key` is `input.gestures[i]`.
    pub fn validate(&self, key: &str) -> Result<(), SettingError> {
        if !(3..=5).contains(&self.fingers) {
            return Err(invalid(
                &format!("{key}.fingers"),
                "a gesture uses 3, 4 or 5 fingers",
            ));
        }
        let a = hypr::action(self.action);
        if !a.allowed.contains(&self.direction) {
            return Err(invalid(&format!("{key}.direction"), a.only_with));
        }
        if let Some(w) = &self.workspace {
            if self.action != Action::Special {
                return Err(invalid(
                    &format!("{key}.workspace"),
                    "only the scratch workspace gesture has a workspace name",
                ));
            }
            if !valid_workspace_name(w) {
                return Err(invalid(
                    &format!("{key}.workspace"),
                    "a workspace name is 1 to 32 letters, digits, - or _",
                ));
            }
        }
        Ok(())
    }
}

/// uQ's own two gestures: three fingers sideways switch workspace, four fingers up open
/// Spotlight.
pub fn defaults() -> Vec<Gesture> {
    vec![
        Gesture {
            fingers: 3,
            direction: Direction::Horizontal,
            action: Action::Workspace,
            modifier: Mods::default(),
            workspace: None,
            enabled: true,
        },
        Gesture {
            fingers: 4,
            direction: Direction::Up,
            action: Action::Spotlight,
            modifier: Mods::default(),
            workspace: None,
            enabled: true,
        },
    ]
}

/// `None` (never set) means uQ's defaults; a list, even an empty one, is exactly that list.
pub fn effective(g: &Option<Vec<Gesture>>) -> Cow<'_, [Gesture]> {
    match g {
        Some(list) => Cow::Borrowed(list),
        None => Cow::Owned(defaults()),
    }
}

/// Every pair of enabled gestures Hyprland would refuse together (same fingers and modifier,
/// and a shared direction, an axis counting as both of its directions), later one first in the
/// message. `first` and `second` are list indexes as text.
pub fn clashes_of(list: &[Gesture]) -> Vec<Conflict> {
    let labels: Vec<&str> = list.iter().map(Gesture::action_label).collect();
    let ids: Vec<String> = (0..list.len()).map(|i| i.to_string()).collect();
    let mut items = Vec::new();
    for (i, g) in list.iter().enumerate().filter(|(_, g)| g.enabled) {
        for atom in g.direction.atoms() {
            items.push((
                (g.fingers, g.modifier.bits(), *atom),
                ids[i].as_str(),
                labels[i],
            ));
        }
    }
    let mut out: Vec<Conflict> = Vec::new();
    for mut c in find(&items) {
        if out
            .iter()
            .any(|o| o.first == c.first && o.second == c.second)
        {
            continue;
        }
        let at = |id: &str| &list[id.parse::<usize>().unwrap_or(0)];
        let (a, b) = (at(&c.first), at(&c.second));
        // The shared trigger is the narrower gesture's own.
        let narrower = if b.direction.atoms().len() < a.direction.atoms().len() {
            b
        } else {
            a
        };
        c.combo_text = narrower.describe();
        out.push(c);
    }
    out
}

/// Every refusal for a list (spec §7.2); the first conflict is reported on the later gesture's
/// `direction`.
pub fn validate(list: &[Gesture]) -> Result<(), SettingError> {
    if list.len() > MAX_GESTURES {
        return Err(invalid(
            "input.gestures",
            format!("at most {MAX_GESTURES} gestures"),
        ));
    }
    for (i, g) in list.iter().enumerate() {
        g.validate(&format!("input.gestures[{i}]"))?;
    }
    if let Some(c) = clashes_of(list).into_iter().next() {
        return Err(invalid(
            &format!("input.gestures[{}].direction", c.second),
            message(&c),
        ));
    }
    Ok(())
}

/// `distance` 0 to 2000, `cancel_ratio` 0 to 1 (rounded to 2 places); `None` is Hyprland's
/// own default.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SwipeOptions {
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub distance: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub invert: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub create_new: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub cancel_ratio: Option<f64>,
}

impl SwipeOptions {
    pub const FIELDS: [&str; 4] = ["distance", "invert", "create_new", "cancel_ratio"];

    pub fn is_default(&self) -> bool {
        *self == SwipeOptions::default()
    }

    pub fn validate(&self) -> Result<(), SettingError> {
        if self.distance.is_some_and(|d| d > 2000) {
            return Err(invalid(
                "input.swipe.distance",
                "Swipe distance must be between 0 and 2000",
            ));
        }
        if self
            .cancel_ratio
            .is_some_and(|r| !r.is_finite() || !(0.0..=1.0).contains(&r))
        {
            return Err(invalid(
                "input.swipe.cancel_ratio",
                "How far to swipe must be between 0 and 1",
            ));
        }
        Ok(())
    }

    pub fn normalised(mut self) -> Self {
        self.cancel_ratio = self.cancel_ratio.map(round2);
        self
    }
}
