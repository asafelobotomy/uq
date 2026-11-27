//! Theme swatches: the colour files a theme ships for the page's previews.

use super::*;

/// The colours a theme's tile is drawn with: `[background, panels, text, accent, success, warning,
/// error]` from its pre-rendered `<dir>/<id>-<variant>/tokens.json` (the other variant when that
/// one isn't there), or none for a theme that isn't pre-rendered (the user's own).
pub fn swatches(dir: &Path, id: &str, variant: &str) -> Vec<String> {
    const KEYS: [&str; 7] = [
        "bg", "surface", "text", "accent", "success", "warning", "error",
    ];
    const MAX: u64 = 1024 * 1024;
    if id.is_empty() || id.contains('/') || id.starts_with('.') {
        return Vec::new();
    }
    let other = if variant == "light" { "dark" } else { "light" };
    for v in [variant, other] {
        let file = dir.join(format!("{id}-{v}")).join("tokens.json");
        let Ok(meta) = std::fs::metadata(&file) else {
            continue;
        };
        if meta.len() > MAX {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Ok(tokens) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let colours: Vec<String> = KEYS
            .iter()
            .filter_map(|k| tokens["colors"][*k].as_str())
            .filter(|c| {
                c.starts_with('#')
                    && c.len() <= 9
                    && c[1..].chars().all(|ch| ch.is_ascii_hexdigit())
            })
            .map(str::to_string)
            .collect();
        if colours.len() >= 4 {
            return colours;
        }
    }
    Vec::new()
}
