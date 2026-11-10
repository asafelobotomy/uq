//! One plain line for a change to the rules, for History. Never prints rule JSON.

use serde_json::Value;

const MAX_NAMED: usize = 3;
const MAX_NAME_CHARS: usize = 40;

/// "Window rule “Name” added", up to three, then ", and N more".
pub fn summary(before: &Value, after: &Value) -> String {
    let mut lines = Vec::new();
    for (list, noun, key) in [
        ("window", "Window rule", "id"),
        ("layer", "Panel rule", "id"),
        ("workspace", "Workspace", "workspace"),
    ] {
        list_changes(before, after, list, noun, key, &mut lines);
    }
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
        line = "Rules".to_string();
    }
    line
}

fn items<'a>(v: &'a Value, list: &str) -> &'a [Value] {
    v.get(list)
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
}

fn key_of<'a>(item: &'a Value, key: &str) -> &'a str {
    item.get(key).and_then(Value::as_str).unwrap_or("")
}

fn cut(s: &str) -> String {
    if s.chars().count() > MAX_NAME_CHARS {
        let head: String = s.chars().take(MAX_NAME_CHARS).collect();
        format!("{head}…")
    } else {
        s.to_string()
    }
}

/// How a changed rule is named: window and panel rules by name, workspace rules by selector.
fn subject(noun: &str, item: &Value, key: &str) -> String {
    if key == "workspace" {
        let sel = key_of(item, key);
        format!("{noun} {}", cut(sel.strip_prefix("name:").unwrap_or(sel)))
    } else {
        let name = match key_of(item, "name") {
            "" => key_of(item, key),
            n => n,
        };
        format!("{noun} “{}”", cut(name))
    }
}

fn phrase(noun: &str, item: &Value, key: &str, what: &str) -> String {
    if key == "workspace" {
        format!("{} rule {what}", subject(noun, item, key))
    } else {
        format!("{} {what}", subject(noun, item, key))
    }
}

fn find<'a>(side: &'a [Value], key: &str, id: &str) -> Option<&'a Value> {
    side.iter().find(|r| key_of(r, key) == id)
}

fn list_changes(
    before: &Value,
    after: &Value,
    list: &str,
    noun: &str,
    key: &str,
    out: &mut Vec<String>,
) {
    let (b, a) = (items(before, list), items(after, list));
    for r in a {
        let id = key_of(r, key);
        match find(b, key, id) {
            None => out.push(phrase(noun, r, key, "added")),
            Some(old) if old != r => {
                let flipped = old.get("enabled") != r.get("enabled");
                let mut same = old.clone();
                if let (Some(m), Some(e)) = (same.as_object_mut(), r.get("enabled")) {
                    m.insert("enabled".into(), e.clone());
                }
                let what = match (flipped && same == *r, r.get("enabled")) {
                    (true, Some(Value::Bool(true))) => "turned on",
                    (true, _) => "turned off",
                    _ => "changed",
                };
                out.push(phrase(noun, r, key, what));
            }
            Some(_) => {}
        }
    }
    for r in b {
        if find(a, key, key_of(r, key)).is_none() {
            out.push(phrase(noun, r, key, "removed"));
        }
    }
    let ids = |side: &[Value]| {
        side.iter()
            .map(|r| key_of(r, key).to_string())
            .collect::<Vec<_>>()
    };
    let (bi, ai) = (ids(b), ids(a));
    if bi != ai {
        let (mut bs, mut as_) = (bi, ai);
        bs.sort();
        as_.sort();
        if bs == as_ {
            out.push(match key {
                "workspace" => "Workspace rules reordered".to_string(),
                _ => format!("{noun}s reordered"),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn w(id: &str, name: &str, on: bool, class: &str) -> Value {
        json!({"id": id, "name": name, "enabled": on,
            "match": {"class": class}, "effects": {"float": true}})
    }
    fn sect(window: Vec<Value>) -> Value {
        json!({"window": window, "layer": [], "workspace": []})
    }

    #[test]
    fn added_removed_changed_toggled() {
        let e = sect(vec![]);
        let one = sect(vec![w("r-0001", "Pavucontrol floats", true, "a")]);
        assert_eq!(summary(&e, &one), "Window rule “Pavucontrol floats” added");
        assert_eq!(
            summary(&one, &e),
            "Window rule “Pavucontrol floats” removed"
        );
        let off = sect(vec![w("r-0001", "Pavucontrol floats", false, "a")]);
        assert_eq!(
            summary(&one, &off),
            "Window rule “Pavucontrol floats” turned off"
        );
        assert_eq!(
            summary(&off, &one),
            "Window rule “Pavucontrol floats” turned on"
        );
        let edited = sect(vec![w("r-0001", "Pavucontrol floats", true, "b")]);
        assert_eq!(
            summary(&one, &edited),
            "Window rule “Pavucontrol floats” changed"
        );
        // Toggled and edited at once is a change.
        let both = sect(vec![w("r-0001", "Pavucontrol floats", false, "b")]);
        assert_eq!(
            summary(&one, &both),
            "Window rule “Pavucontrol floats” changed"
        );
        let layer = json!({"window": [], "layer": [{"id": "r-0002", "name": "Blur notifications",
            "enabled": true, "match": {"namespace": "n"}, "effects": {"blur": true}}],
            "workspace": []});
        assert_eq!(summary(&e, &layer), "Panel rule “Blur notifications” added");
    }

    #[test]
    fn reorder_only() {
        let a = w("r-0001", "A", true, "a");
        let b = w("r-0002", "B", true, "b");
        assert_eq!(
            summary(&sect(vec![a.clone(), b.clone()]), &sect(vec![b, a])),
            "Window rules reordered"
        );
    }

    #[test]
    fn caps_at_three() {
        let rules: Vec<Value> = (1..=5)
            .map(|i| w(&format!("r-000{i}"), &format!("R{i}"), true, "a"))
            .collect();
        assert_eq!(
            summary(&sect(vec![]), &sect(rules)),
            "Window rule “R1” added, Window rule “R2” added, Window rule “R3” added, and 2 more"
        );
    }

    #[test]
    fn named_workspace_reads_plain() {
        let none = sect(vec![]);
        let mut with = none.clone();
        with["workspace"] = json!([{"workspace": "name:web", "persistent": true}]);
        assert_eq!(summary(&none, &with), "Workspace web rule added");
        let mut edited = with.clone();
        edited["workspace"] = json!([{"workspace": "name:web", "persistent": false}]);
        assert_eq!(summary(&with, &edited), "Workspace web rule changed");
        let mut three = none.clone();
        three["workspace"] = json!([{"workspace": "3", "persistent": true}]);
        assert_eq!(summary(&none, &three), "Workspace 3 rule added");
    }

    #[test]
    fn long_names_are_cut() {
        let name = "x".repeat(60);
        let line = summary(&sect(vec![]), &sect(vec![w("r-0001", &name, true, "a")]));
        assert_eq!(line, format!("Window rule “{}…” added", "x".repeat(40)));
    }

    #[test]
    fn never_prints_json() {
        let names = [
            "plain",
            "has {brace}",
            "has [bracket]",
            "has \"quote\"",
            "{\"a\": [1]}",
        ];
        let rule = |i: usize, on: bool, class: &str| w(&format!("r-000{i}"), names[i], on, class);
        let all: Vec<Value> = (0..names.len()).map(|i| rule(i, true, "a")).collect();
        let e = sect(vec![]);
        let off: Vec<Value> = (0..names.len()).map(|i| rule(i, false, "a")).collect();
        let edited: Vec<Value> = (0..names.len()).map(|i| rule(i, true, "b")).collect();
        let mut rev = all.clone();
        rev.reverse();
        let layer = |rules: &[Value]| {
            json!({"window": [], "layer": rules.iter().map(|r| json!({
                "id": r["id"], "name": r["name"], "enabled": r["enabled"],
                "match": {"namespace": "n"}, "effects": {"blur": true}})).collect::<Vec<_>>(),
                "workspace": []})
        };
        let ws = |on: bool| {
            json!({"window": [], "layer": [],
            "workspace": [{"workspace": "3", "persistent": on}]})
        };
        let pairs = [
            (e.clone(), sect(all.clone())),
            (sect(all.clone()), e.clone()),
            (sect(all.clone()), sect(off.clone())),
            (sect(off.clone()), sect(all.clone())),
            (sect(all.clone()), sect(edited.clone())),
            (sect(all.clone()), sect(rev.clone())),
            (layer(&[]), layer(&all)),
            (layer(&all), layer(&[])),
            (layer(&all), layer(&off)),
            (layer(&off), layer(&all)),
            (layer(&all), layer(&rev)),
            (ws(true), ws(false)),
            (e.clone(), ws(true)),
            (ws(true), e.clone()),
        ];
        for (b, a) in pairs {
            let line = summary(&b, &a);
            assert!(!line.is_empty());
            for bad in [
                "\"match\"",
                "\"effects\"",
                "\"id\"",
                "\"enabled\"",
                "\"workspace\"",
                "[{",
                "{\"id",
            ] {
                assert!(!line.contains(bad), "{line}");
            }
        }
        let line = summary(&e, &sect(all));
        assert!(line.contains("has {brace}"), "{line}");
    }
}
