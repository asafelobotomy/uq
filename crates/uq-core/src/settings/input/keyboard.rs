//! Keyboard layouts, layout-switch combo and key repeat.

use super::{parse_lenient, parse_strict};
use crate::settings::{SettingError, invalid, lenient};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_LAYOUTS: usize = 4;

fn ascii_in(s: &str, len: std::ops::RangeInclusive<usize>, ok: impl Fn(u8) -> bool) -> bool {
    len.contains(&s.len()) && s.bytes().all(ok)
}

/// `^[a-z]{2,8}$`
pub fn valid_layout(s: &str) -> bool {
    ascii_in(s, 2..=8, |b| b.is_ascii_lowercase())
}

/// `^[A-Za-z0-9_-]{1,32}$`
pub fn valid_variant(s: &str) -> bool {
    ascii_in(s, 1..=32, |b| {
        b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')
    })
}

/// `^grp:[a-z0-9_]{1,40}$`
pub fn valid_switch_combo(s: &str) -> bool {
    s.strip_prefix("grp:").is_some_and(|r| {
        ascii_in(r, 1..=40, |b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'
        })
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KbLayout {
    pub layout: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

impl KbLayout {
    pub const FIELDS: [&str; 2] = ["layout", "variant"];

    fn check(&self) -> Result<(), SettingError> {
        if !valid_layout(&self.layout) {
            return Err(invalid(
                "input.keyboard.layouts",
                format!("layout {:?} must be 2 to 8 lowercase letters", self.layout),
            ));
        }
        if let Some(v) = &self.variant
            && !valid_variant(v)
        {
            return Err(invalid(
                "input.keyboard.layouts",
                format!("variant {v:?} must be 1 to 32 letters, digits, _ or -"),
            ));
        }
        Ok(())
    }
}

/// Entries load one by one; a bad one is dropped.
fn lenient_layouts<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<KbLayout>, D::Error> {
    let Value::Array(items) = Value::deserialize(d)? else {
        return Ok(Vec::new());
    };
    Ok(items
        .iter()
        .filter_map(|v| parse_lenient("input.keyboard.layouts", v, &KbLayout::FIELDS))
        .collect())
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyboardSettings {
    /// 0 to 4 layouts; empty means base.lua's `us`.
    #[serde(deserialize_with = "lenient_layouts")]
    pub layouts: Vec<KbLayout>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub switch_combo: Option<String>,
    /// Keys per second, 1 to 100.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub repeat_rate: Option<u8>,
    /// Milliseconds, 100 to 2000.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub repeat_delay: Option<u16>,
}

impl KeyboardSettings {
    pub const FIELDS: [&str; 4] = ["layouts", "switch_combo", "repeat_rate", "repeat_delay"];

    fn rate_ok(&self) -> bool {
        self.repeat_rate.is_none_or(|r| (1..=100).contains(&r))
    }
    fn delay_ok(&self) -> bool {
        self.repeat_delay.is_none_or(|d| (100..=2000).contains(&d))
    }
    fn combo_ok(&self) -> bool {
        self.switch_combo.as_deref().is_none_or(valid_switch_combo)
    }

    pub fn validate(&self) -> Result<(), SettingError> {
        const KEY: &str = "input.keyboard";
        if self.layouts.len() > MAX_LAYOUTS {
            return Err(invalid(KEY, format!("at most {MAX_LAYOUTS} layouts")));
        }
        for (i, l) in self.layouts.iter().enumerate() {
            l.check()?;
            if self.layouts[..i].contains(l) {
                return Err(invalid(KEY, "the same layout and variant twice"));
            }
        }
        if !self.combo_ok() {
            return Err(invalid(
                KEY,
                "switch_combo must look like grp:alt_shift_toggle",
            ));
        }
        if !self.rate_ok() {
            return Err(invalid(KEY, "repeat_rate must be between 1 and 100"));
        }
        if !self.delay_ok() {
            return Err(invalid(KEY, "repeat_delay must be between 100 and 2000"));
        }
        Ok(())
    }

    pub(super) fn from_value(v: &Value) -> Result<Self, SettingError> {
        const KEY: &str = "input.keyboard";
        let Some(obj) = v.as_object() else {
            return Err(invalid(KEY, "expected an object"));
        };
        let mut rest = obj.clone();
        let layouts = rest.remove("layouts");
        let mut k: Self = parse_strict(KEY, &Value::Object(rest), &Self::FIELDS)?;
        k.layouts = match layouts {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .map(|e| parse_strict("input.keyboard.layouts", e, &KbLayout::FIELDS))
                .collect::<Result<_, _>>()?,
            Some(_) => return Err(invalid("input.keyboard.layouts", "expected an array")),
        };
        k.validate()?;
        Ok(k)
    }

    pub(super) fn sanitised(mut self, errors: &mut Vec<SettingError>) -> Self {
        const KEY: &str = "input.keyboard";
        let mut layouts: Vec<KbLayout> = Vec::new();
        for l in std::mem::take(&mut self.layouts) {
            match l.check() {
                Err(e) => errors.push(e),
                Ok(()) if layouts.contains(&l) => {
                    errors.push(invalid(KEY, "dropped a repeated layout"));
                }
                Ok(()) if layouts.len() >= MAX_LAYOUTS => {
                    errors.push(invalid(KEY, "dropped a layout: more than 4"));
                }
                Ok(()) => layouts.push(l),
            }
        }
        self.layouts = layouts;
        if !self.combo_ok() {
            errors.push(invalid(KEY, "switch_combo cleared: not a grp: option"));
            self.switch_combo = None;
        }
        if !self.rate_ok() {
            errors.push(invalid(KEY, "repeat_rate cleared: out of range"));
            self.repeat_rate = None;
        }
        if !self.delay_ok() {
            errors.push(invalid(KEY, "repeat_delay cleared: out of range"));
            self.repeat_delay = None;
        }
        self
    }
}
