//! The history line for a `keybinds` change: plain words, by label, never JSON or a command's
//! text. Pairs binds by id.

use super::{combo, defaults};
use serde_json::Value;

const MAX_NAMED: usize = 3;
const MAX_NAME_CHARS: usize = 40;

fn items<'a>(v: &'a Value, list: &str) -> &'a [Value] {
    v.get(list)
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
}

fn text<'a>(item: &'a Value, key: &str) -> &'a str {
    item.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Off only when `enabled` is `false` (absent means on).
fn on(item: &Value) -> bool {
    item.get("enabled").and_then(Value::as_bool) != Some(false)
}

fn cut(s: &str) -> String {
    // A name is the user's own words, but braces and straight quotes could make a line look
    // like JSON, so they're left out or softened.
    let clean: String = s
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '{' | '}'))
        .map(|c| if c == '"' { '\'' } else { c })
        .collect();
    let clean = clean.trim();
    if clean.chars().count() > MAX_NAME_CHARS {
        let head: String = clean.chars().take(MAX_NAME_CHARS).collect();
        format!("{head}…")
    } else {
        clean.to_string()
    }
}

fn combo_words(t: &str) -> String {
    combo::parse(t).map_or_else(|_| "other keys".into(), combo::text)
}

/// The label of one of uQ's shortcuts; an id this version doesn't know is shown as it is.
fn default_label(id: &str) -> String {
    defaults::default_of(id).map_or_else(|| cut(id), |d| d.label.to_string())
}

/// What the override `o` of the default `id` does now, as the end of a sentence about it.
fn override_state(o: &Value) -> String {
    if !on(o) {
        "turned off".into()
    } else if let Some(c) = o.get("combo").and_then(Value::as_str) {
        format!("now uses {}", combo_words(c))
    } else {
        "back to its default".into()
    }
}

fn override_lines(before: &Value, after: &Value, lines: &mut Vec<String>) {
    let (b, a) = (items(before, "overrides"), items(after, "overrides"));
    let find = |side: &[Value], id: &str| side.iter().position(|o| text(o, "id") == id);
    // Every override dropped at once reads as one line.
    if b.len() >= 2 && a.is_empty() {
        lines.push("uQ's shortcuts put back to how they came".into());
        return;
    }
    for o in a {
        let id = text(o, "id");
        let changed = match find(b, id) {
            None => true,
            Some(i) => &b[i] != o,
        };
        if changed {
            let state = match find(b, id).map(|i| &b[i]) {
                // Turned back on, with the keys it had: say so rather than repeat the keys.
                Some(old) if !on(old) && on(o) && o.get("combo").is_none() => {
                    "back to its default".to_string()
                }
                _ => override_state(o),
            };
            lines.push(format!("“{}” {state}", default_label(id)));
        }
    }
    for o in b {
        let id = text(o, "id");
        if find(a, id).is_none() {
            lines.push(format!("“{}” back to its default", default_label(id)));
        }
    }
}

fn custom_lines(before: &Value, after: &Value, lines: &mut Vec<String>) {
    let (b, a) = (items(before, "custom"), items(after, "custom"));
    let find = |side: &'_ [Value], id: &str| side.iter().find(|o| text(o, "id") == id).cloned();
    for n in a {
        let name = cut(text(n, "name"));
        match find(b, text(n, "id")) {
            None => lines.push(format!("Shortcut “{name}” added")),
            Some(old) if &old != n => {
                // Only the switch differs: say turned on or off, not changed.
                let same = |v: &Value| {
                    let mut v = v.clone();
                    v["enabled"] = Value::Bool(true);
                    v
                };
                let verb = if same(&old) == same(n) && on(&old) != on(n) {
                    if on(n) { "turned on" } else { "turned off" }
                } else {
                    "changed"
                };
                lines.push(format!("Shortcut “{name}” {verb}"));
            }
            Some(_) => {}
        }
    }
    for o in b {
        if find(a, text(o, "id")).is_none() {
            lines.push(format!("Shortcut “{}” removed", cut(text(o, "name"))));
        }
    }
}

/// "“Open Spotlight” now uses Super + K", up to three, then ", and N more".
pub fn summary(before: &Value, after: &Value) -> String {
    let mut lines = Vec::new();
    override_lines(before, after, &mut lines);
    custom_lines(before, after, &mut lines);
    let mut line = lines
        .iter()
        .take(MAX_NAMED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if lines.len() > MAX_NAMED {
        line.push_str(&format!(", and {} more", lines.len() - MAX_NAMED));
    }
    if line.is_empty() {
        line = "Keyboard shortcuts".into();
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn custom(id: &str, name: &str, action: Value) -> Value {
        json!({"id": id, "name": name, "combo": "SUPER + F1", "enabled": true,
               "locked": false, "repeating": false, "action": action})
    }

    #[test]
    fn rebind_off_reset_and_custom_lines() {
        let none = json!({});
        let s = |b: &Value, a: &Value| summary(b, a);
        assert_eq!(
            s(
                &none,
                &json!({"overrides": [{"id": "uq.spotlight", "combo": "SUPER + K"}]})
            ),
            "“Open Spotlight” now uses Super + K"
        );
        assert_eq!(
            s(
                &none,
                &json!({"overrides": [{"id": "uq.close", "enabled": false}]})
            ),
            "“Close the window” turned off"
        );
        let with = json!({"overrides": [{"id": "uq.spotlight", "combo": "SUPER + K"}]});
        assert_eq!(
            s(&with, &json!({"overrides": []})),
            "“Open Spotlight” back to its default"
        );
        assert_eq!(
            s(
                &json!({"overrides": [{"id": "uq.close", "enabled": false}]}),
                &json!({"overrides": []})
            ),
            "“Close the window” back to its default"
        );
        let term = json!({"kind": "uq", "name": "terminal"});
        let shot = custom("k-0001", "Screenshot", term.clone());
        assert_eq!(
            s(&none, &json!({"custom": [shot.clone()]})),
            "Shortcut “Screenshot” added"
        );
        assert_eq!(
            s(&json!({"custom": [shot.clone()]}), &none),
            "Shortcut “Screenshot” removed"
        );
        let mut moved = shot.clone();
        moved["combo"] = "SUPER + F2".into();
        assert_eq!(
            s(
                &json!({"custom": [shot.clone()]}),
                &json!({"custom": [moved]})
            ),
            "Shortcut “Screenshot” changed"
        );
        let mut off = shot.clone();
        off["enabled"] = false.into();
        assert_eq!(
            s(
                &json!({"custom": [shot.clone()]}),
                &json!({"custom": [off.clone()]})
            ),
            "Shortcut “Screenshot” turned off"
        );
        assert_eq!(
            s(&json!({"custom": [off]}), &json!({"custom": [shot]})),
            "Shortcut “Screenshot” turned on"
        );
    }

    #[test]
    fn reset_all_reads_plainly() {
        let before = json!({"overrides": [{"id": "uq.spotlight", "combo": "SUPER + K"},
                                          {"id": "uq.close", "enabled": false}]});
        assert_eq!(
            summary(&before, &json!({"overrides": []})),
            "uQ's shortcuts put back to how they came"
        );
    }

    #[test]
    fn caps_at_three() {
        let after = json!({"custom": [
            custom("k-0001", "A", json!({"kind": "uq", "name": "terminal"})),
            custom("k-0002", "B", json!({"kind": "uq", "name": "terminal"})),
            custom("k-0003", "C", json!({"kind": "uq", "name": "terminal"})),
            custom("k-0004", "D", json!({"kind": "uq", "name": "terminal"})),
            custom("k-0005", "E", json!({"kind": "uq", "name": "terminal"})),
        ]});
        assert_eq!(
            summary(&json!({}), &after),
            "Shortcut “A” added, Shortcut “B” added, Shortcut “C” added, and 2 more"
        );
    }

    #[test]
    fn long_names_are_cut_and_control_characters_dropped() {
        let name = format!("{}\u{1b}[31m", "n".repeat(60));
        let after =
            json!({"custom": [custom("k-0001", &name, json!({"kind": "uq", "name": "exit"}))]});
        let line = summary(&json!({}), &after);
        assert!(line.contains('…') && !line.contains('\u{1b}'), "{line}");
    }

    /// Review Focus 1: a command, an app id or JSON never reaches a history line, whatever the
    /// kind of action and whatever happened to it.
    #[test]
    fn never_prints_commands_or_json() {
        const HOSTILE: &str = "rm -rf ~ ; $(echo pwned)";
        let actions = [
            json!({"kind": "shell", "name": "lock"}),
            json!({"kind": "window", "name": "focus", "direction": "left"}),
            json!({"kind": "workspace", "name": "focus", "workspace": 3}),
            json!({"kind": "uq", "name": "layout", "layout": "master"}),
            json!({"kind": "launch", "app": HOSTILE}),
            json!({"kind": "command", "command": HOSTILE}),
        ];
        let names = ["plain", "br{ace}", "sq[uare]", "quo\"te", "{\"action\": 1}"];
        for a in &actions {
            for name in names {
                let on = custom("k-0001", name, a.clone());
                let mut off = on.clone();
                off["enabled"] = false.into();
                let mut other = on.clone();
                other["action"] = json!({"kind": "command", "command": HOSTILE});
                let empty = json!({});
                let one = |c: &Value| json!({"custom": [c.clone()]});
                for (b, af) in [
                    (&empty, one(&on)),
                    (&one(&on), empty.clone()),
                    (&one(&on), one(&other)),
                    (&one(&on), one(&off)),
                    (&one(&off), one(&on)),
                ] {
                    let line = summary(b, &af);
                    assert!(!line.is_empty());
                    assert!(!line.contains(HOSTILE), "{line}");
                    assert!(!line.contains("pwned"), "{line}");
                    assert!(
                        !line.contains("\"action\"") && !line.contains("\"kind\""),
                        "{line}"
                    );
                    assert!(!line.contains("\"id\"") && !line.contains("hl."), "{line}");
                    assert!(!line.contains(['{', '}', '"']), "{line}");
                }
            }
        }
    }
}
