//! The `layouts` section's option tables (dwindle, master, scrolling) and the default layout.
//! Per-workspace rules stay in `State::layouts`; `State::section` joins the two.

use super::look::ranged_f64;
use super::{SettingError, check_known, invalid, round2};
use crate::lua::{Layout, workspace_selector};
use serde::de::{DeserializeOwned, IntoDeserializer};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

const MAX_WORKSPACES: usize = 64;
const MAX_SELECTOR_CHARS: usize = 64;

/// Load-time leniency: a value this version doesn't know (a newer uqd's) loads as unset. The
/// API stays strict through the `check_values` methods.
fn lenient<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let s = Option::<String>::deserialize(d)?;
    Ok(s.and_then(|s| {
        T::deserialize(IntoDeserializer::<serde::de::value::Error>::into_deserializer(s.as_str()))
            .ok()
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForceSplit {
    FollowMouse,
    Left,
    Right,
}

impl ForceSplit {
    /// Hyprland's `dwindle:force_split` int.
    pub fn as_int(self) -> u8 {
        match self {
            ForceSplit::FollowMouse => 0,
            ForceSplit::Left => 1,
            ForceSplit::Right => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NewStatus {
    Master,
    Slave,
    Inherit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

impl NewStatus {
    /// The snake_case name, which is also Hyprland's value.
    pub fn as_str(self) -> &'static str {
        match self {
            NewStatus::Master => "master",
            NewStatus::Slave => "slave",
            NewStatus::Inherit => "inherit",
        }
    }
}

impl Orientation {
    /// The snake_case name, which is also Hyprland's value.
    pub fn as_str(self) -> &'static str {
        match self {
            Orientation::Left => "left",
            Orientation::Right => "right",
            Orientation::Top => "top",
            Orientation::Bottom => "bottom",
            Orientation::Center => "center",
        }
    }
}

/// Reject a present, non-null `key` of `v` that isn't a valid `T`.
fn check_enum<T: DeserializeOwned>(
    section: &str,
    v: &Value,
    key: &str,
) -> Result<(), SettingError> {
    match v.get(key) {
        Some(x) if !x.is_null() && serde_json::from_value::<T>(x.clone()).is_err() => {
            Err(invalid(&format!("{section}.{key}"), "unknown value"))
        }
        _ => Ok(()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DwindleOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preserve_split: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smart_split: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub force_split: Option<ForceSplit>,
}

impl DwindleOptions {
    pub const FIELDS: [&str; 3] = ["preserve_split", "smart_split", "force_split"];
    fn check_values(v: &Value) -> Result<(), SettingError> {
        check_enum::<ForceSplit>("dwindle", v, "force_split")
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MasterOptions {
    /// Master area share, 0.1 to 0.9.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mfact: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub new_status: Option<NewStatus>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub orientation: Option<Orientation>,
}

impl MasterOptions {
    pub const FIELDS: [&str; 3] = ["mfact", "new_status", "orientation"];
    fn check_values(v: &Value) -> Result<(), SettingError> {
        check_enum::<NewStatus>("master", v, "new_status")?;
        check_enum::<Orientation>("master", v, "orientation")
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrollingOptions {
    /// Column width as a share of the screen, 0.1 to 1.0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fullscreen_on_one_column: Option<bool>,
}

impl ScrollingOptions {
    pub const FIELDS: [&str; 2] = ["column_width", "fullscreen_on_one_column"];
}

/// The default layout and per-layout options. Stored as `[layout_options]` because the
/// top-level `[layouts]` table already holds workspace rules.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutOptions {
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub default: Option<Layout>,
    pub dwindle: DwindleOptions,
    pub master: MasterOptions,
    pub scrolling: ScrollingOptions,
}

impl LayoutOptions {
    /// Every key of the `layouts` section.
    pub const FIELDS: [&str; 5] = ["default", "workspaces", "dwindle", "master", "scrolling"];

    pub fn validate(&self) -> Result<(), SettingError> {
        ranged_f64("layouts.master.mfact", self.master.mfact, 0.1, 0.9)?;
        ranged_f64(
            "layouts.scrolling.column_width",
            self.scrolling.column_width,
            0.1,
            1.0,
        )
    }

    /// [`LayoutOptions::validate`], then round the fractions to two places.
    pub fn normalised(mut self) -> Result<Self, SettingError> {
        self.validate()?;
        self.master.mfact = self.master.mfact.map(round2);
        self.scrolling.column_width = self.scrolling.column_width.map(round2);
        Ok(self)
    }

    /// Parse a whole `layouts` section: the options, and the workspace rules with their
    /// selectors normalised. Unknown keys and values are errors.
    pub fn from_section(v: &Value) -> Result<(Self, BTreeMap<String, Layout>), SettingError> {
        const KEY: &str = "layouts";
        check_known(KEY, v, &Self::FIELDS)?;
        for (name, known) in [
            ("dwindle", &DwindleOptions::FIELDS[..]),
            ("master", &MasterOptions::FIELDS[..]),
            ("scrolling", &ScrollingOptions::FIELDS[..]),
        ] {
            if let Some(sub) = v.get(name) {
                check_known(&format!("{KEY}.{name}"), sub, known)?;
            }
        }
        check_enum::<Layout>(KEY, v, "default")?;
        if let Some(d) = v.get("dwindle") {
            DwindleOptions::check_values(d)?;
        }
        if let Some(m) = v.get("master") {
            MasterOptions::check_values(m)?;
        }
        let rules = Self::rules(v.get("workspaces"))?;
        let mut opts = v.clone();
        if let Some(o) = opts.as_object_mut() {
            o.remove("workspaces");
        }
        let opts: LayoutOptions =
            serde_json::from_value(opts).map_err(|e| invalid(KEY, e.to_string()))?;
        Ok((opts.normalised()?, rules))
    }

    fn rules(v: Option<&Value>) -> Result<BTreeMap<String, Layout>, SettingError> {
        const KEY: &str = "layouts.workspaces";
        let empty = Map::new();
        let obj = match v {
            None | Some(Value::Null) => &empty,
            Some(Value::Object(o)) => o,
            Some(_) => return Err(invalid(KEY, "expected an object")),
        };
        if obj.len() > MAX_WORKSPACES {
            return Err(invalid(KEY, format!("at most {MAX_WORKSPACES} rules")));
        }
        let mut out = BTreeMap::new();
        for (ws, layout) in obj {
            let sel = workspace_selector(ws);
            let n = sel.chars().count();
            if !(1..=MAX_SELECTOR_CHARS).contains(&n) {
                return Err(invalid(
                    KEY,
                    format!("selector must be 1 to {MAX_SELECTOR_CHARS} characters"),
                ));
            }
            if sel.chars().any(char::is_control) {
                return Err(invalid(KEY, "selector must not contain control characters"));
            }
            let layout: Layout = serde_json::from_value(layout.clone())
                .map_err(|_| invalid(KEY, "expected dwindle, master or scrolling"))?;
            if out.insert(sel, layout).is_some() {
                return Err(invalid(KEY, format!("{ws:?} names a workspace twice")));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn enums_parse_snake_case() {
        let d: DwindleOptions =
            serde_json::from_value(json!({"force_split": "follow_mouse"})).unwrap();
        assert_eq!(d.force_split, Some(ForceSplit::FollowMouse));
        assert_eq!(
            [ForceSplit::FollowMouse, ForceSplit::Left, ForceSplit::Right].map(ForceSplit::as_int),
            [0, 1, 2]
        );
        let m: MasterOptions =
            serde_json::from_value(json!({"new_status": "inherit", "orientation": "center"}))
                .unwrap();
        assert_eq!(
            (m.new_status, m.orientation),
            (Some(NewStatus::Inherit), Some(Orientation::Center))
        );
        assert_eq!(Orientation::Bottom.as_str(), "bottom");
        // Load is lenient: an unknown name is unset. The API is strict.
        let m: MasterOptions = serde_json::from_value(json!({"orientation": "diagonal"})).unwrap();
        assert_eq!(m.orientation, None);
        for bad in [
            json!({"dwindle": {"force_split": "followMouse"}}),
            json!({"master": {"orientation": "diagonal"}}),
            json!({"master": {"new_status": 3}}),
            json!({"default": "monocle"}),
        ] {
            assert!(LayoutOptions::from_section(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn ranges() {
        let ok = |v: Value| LayoutOptions::from_section(&v).map(|(o, _)| o);
        assert_eq!(
            ok(json!({"master": {"mfact": 0.555}}))
                .unwrap()
                .master
                .mfact,
            Some(0.56)
        );
        assert!(ok(json!({"master": {"mfact": 0.1}})).is_ok());
        assert!(ok(json!({"master": {"mfact": 0.9}})).is_ok());
        for bad in [
            json!({"master": {"mfact": 0.95}}),
            json!({"master": {"mfact": 0.05}}),
            json!({"scrolling": {"column_width": 1.5}}),
            json!({"scrolling": {"column_width": 0.0}}),
            json!({"scrolling": {"bogus": 1}}),
            json!({"dwindle": {"bogus": 1}}),
            json!({"bogus": 1}),
            json!({"workspaces": []}),
        ] {
            assert!(ok(bad.clone()).is_err(), "{bad}");
        }
        assert!(ok(json!({"scrolling": {"column_width": 1.0}})).is_ok());
        let nan = LayoutOptions {
            master: MasterOptions {
                mfact: Some(f64::NAN),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(nan.validate().is_err());
    }
}
