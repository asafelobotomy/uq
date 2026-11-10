//! The `look` section: gaps, border, rounding, blur, shadow and animations. `None` means "not
//! overridden" (the theme's `ui` token, or base.lua's value where the theme has none).

use super::appearance::ranged;
use super::{SettingError, invalid, lenient, round2};
use serde::{Deserialize, Serialize};

/// Unknown keys are ignored on load and rejected by [`super::Settings::set_section`] via
/// [`Look::FIELDS`]. A wrong-typed value loads as unset.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub gaps_in: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub gaps_out: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub border_width: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub rounding: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub blur: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub blur_size: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub blur_passes: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub shadow: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub animations: Option<bool>,
    /// 0.25 to 4.0; 2.0 is twice as fast.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub animation_speed: Option<f64>,
}

/// `v` within `lo..=hi` (finite), else an error naming `key`.
pub(super) fn ranged_f64(key: &str, v: Option<f64>, lo: f64, hi: f64) -> Result<(), SettingError> {
    match v {
        Some(n) if !n.is_finite() || !(lo..=hi).contains(&n) => {
            Err(invalid(key, format!("must be between {lo} and {hi}")))
        }
        _ => Ok(()),
    }
}

impl Look {
    /// Every key the API accepts.
    pub const FIELDS: [&str; 10] = [
        "gaps_in",
        "gaps_out",
        "border_width",
        "rounding",
        "blur",
        "blur_size",
        "blur_passes",
        "shadow",
        "animations",
        "animation_speed",
    ];

    pub fn validate(&self) -> Result<(), SettingError> {
        ranged("look.gaps_in", self.gaps_in, 0, 50)?;
        ranged("look.gaps_out", self.gaps_out, 0, 100)?;
        ranged("look.border_width", self.border_width, 0, 20)?;
        ranged("look.rounding", self.rounding, 0, 40)?;
        ranged("look.blur_size", self.blur_size, 1, 20)?;
        ranged("look.blur_passes", self.blur_passes, 1, 4)?;
        ranged_f64("look.animation_speed", self.animation_speed, 0.25, 4.0)
    }

    /// [`Look::validate`], then round `animation_speed` to two places.
    pub fn normalised(mut self) -> Result<Self, SettingError> {
        self.validate()?;
        self.animation_speed = self.animation_speed.map(round2);
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Settings;
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_all_unset() {
        let l = Look::default();
        assert_eq!(l, serde_json::from_value(json!({})).unwrap());
        assert_eq!(serde_json::to_value(&l).unwrap(), json!({}));
        assert!(l.validate().is_ok());
        assert_eq!(Look::FIELDS.len(), 10);
    }

    #[test]
    fn ranges_and_rounding() {
        let mut s = Settings::default();
        s.set_section("look", json!({"animation_speed": 1.234, "gaps_in": 50}))
            .unwrap();
        assert_eq!(s.look.animation_speed, Some(1.23));
        for bad in [
            json!({"animation_speed": 0.2}),
            json!({"animation_speed": 5}),
            json!({"blur_passes": 0}),
            json!({"blur_passes": 5}),
            json!({"blur_size": 0}),
            json!({"gaps_in": 51}),
            json!({"gaps_out": 101}),
            json!({"border_width": 21}),
            json!({"rounding": 41}),
            json!({"rounding": -1}),
            json!({"blur": "yes"}),
            json!({"gaps_in": 300}),
            json!({"animation_speed": "fast"}),
        ] {
            assert!(s.set_section("look", bad.clone()).is_err(), "{bad}");
        }
        let nan = Look {
            animation_speed: Some(f64::NAN),
            ..Look::default()
        };
        assert!(nan.validate().is_err());
        assert_eq!(s.look.gaps_in, Some(50), "errors leave the section alone");
        let top = json!({"gaps_in": 0, "gaps_out": 100, "border_width": 20, "rounding": 40,
            "blur_size": 20, "blur_passes": 4, "animation_speed": 4.0});
        s.set_section("look", top).unwrap();
    }
}
