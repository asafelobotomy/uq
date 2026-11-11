//! A screen's identity (`make`, `model`, `serial`), the key a saved arrangement and the main
//! display are matched by. Connector names (`DP-1`) move between ports; these don't.

use super::{LiveOutput, SettingError, clean_text, invalid};
use serde::{Deserialize, Serialize};

/// Longest identity field, in characters.
pub const MAX_ID_CHARS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MonitorId {
    pub make: String,
    pub model: String,
    pub serial: String,
}

impl MonitorId {
    pub const FIELDS: [&str; 3] = ["make", "model", "serial"];

    /// The identity of a live output, cut to the stored length.
    pub fn of(o: &LiveOutput) -> MonitorId {
        MonitorId {
            make: clean_text(&o.make, MAX_ID_CHARS),
            model: clean_text(&o.model, MAX_ID_CHARS),
            serial: clean_text(&o.serial, MAX_ID_CHARS),
        }
    }

    /// A name for people: "Dell U2720Q" (make and model), "A display" when both are empty.
    pub fn label(&self) -> String {
        let (make, model) = (self.make.trim(), self.model.trim());
        let first = make.split_whitespace().next().unwrap_or("");
        let model_has_make = !first.is_empty()
            && model
                .to_lowercase()
                .starts_with(&first.to_lowercase().trim_end_matches('.').to_string());
        match (make.is_empty(), model.is_empty()) {
            (true, true) => "A display".into(),
            (_, true) => make.into(),
            (true, false) => model.into(),
            (false, false) if model_has_make => model.into(),
            (false, false) => format!("{make} {model}"),
        }
    }

    /// Every field at most 64 characters with no control characters; all-empty is allowed
    /// (some screens report nothing).
    pub fn check(&self, key: &str) -> Result<(), SettingError> {
        for (name, v) in [
            ("make", &self.make),
            ("model", &self.model),
            ("serial", &self.serial),
        ] {
            if v.chars().count() > MAX_ID_CHARS {
                return Err(invalid(
                    key,
                    format!("the screen's {name} is over {MAX_ID_CHARS} characters"),
                ));
            }
            if v.chars().any(char::is_control) {
                return Err(invalid(
                    key,
                    format!("the screen's {name} has a control character"),
                ));
            }
        }
        Ok(())
    }
}

/// Whether an output takes part in a connected set: Hyprland's `FALLBACK` and headless
/// outputs are not screens.
pub fn counts_in_sets(name: &str) -> bool {
    name != "FALLBACK" && !name.starts_with("HEADLESS-")
}
