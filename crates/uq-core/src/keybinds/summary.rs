//! The history line for a `keybinds` change. Never prints JSON or a command's text.

use serde_json::Value;

/// A neutral line for now: names which list changed. Task 7 replaces it with per-shortcut
/// wording.
pub fn summary(before: &Value, after: &Value) -> String {
    let changed = |k: &str| before.get(k) != after.get(k);
    match (changed("overrides"), changed("custom")) {
        (true, true) => "uQ's shortcuts and your own shortcuts changed".into(),
        (true, false) => "uQ's shortcuts changed".into(),
        (false, true) => "Your own shortcuts changed".into(),
        (false, false) => String::new(),
    }
}
