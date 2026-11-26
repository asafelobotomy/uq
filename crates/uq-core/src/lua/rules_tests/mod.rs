use super::*;
use crate::lua::{StartupRender, emit_generated, emit_workspace_rule};
use crate::rules::{LayerMatch, Rules, WindowMatch};
use crate::settings::Settings;
use crate::theme::{Theme, Variant, tests::FIXTURE};

fn theme() -> Theme {
    Theme::from_toml(FIXTURE, "fixture", Variant::Dark).unwrap()
}

fn window(id: &str, matches: WindowMatch, effects: WindowEffects) -> WindowRule {
    WindowRule {
        id: id.into(),
        name: "A name".into(),
        enabled: true,
        matches,
        effects,
    }
}

fn pavucontrol() -> WindowRule {
    window(
        "r-7f2c",
        WindowMatch {
            class: Some(r"^(org\.pulseaudio\.pavucontrol)$".into()),
            ..Default::default()
        },
        WindowEffects {
            float: Some(true),
            size: Some([800, 600]),
            center: Some(true),
            ..Default::default()
        },
    )
}

fn layer(id: &str, ns: &str, effects: LayerEffects) -> LayerRule {
    LayerRule {
        id: id.into(),
        name: "Blur notifications".into(),
        enabled: true,
        matches: LayerMatch {
            namespace: ns.into(),
        },
        effects,
    }
}

/// Undo `lua_string` for one string literal at the start of `s`; returns the value and the rest.
fn unescape(s: &str) -> (String, &str) {
    let b = s.as_bytes();
    assert_eq!(b[0], b'"', "{s}");
    let mut out = Vec::new();
    let mut i = 1;
    loop {
        match b[i] {
            b'"' => return (String::from_utf8(out).unwrap(), &s[i + 1..]),
            b'\\' => {
                i += 1;
                match b[i] {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b'\\' => out.push(b'\\'),
                    b'"' => out.push(b'"'),
                    d if d.is_ascii_digit() => {
                        out.push(s[i..i + 3].parse::<u8>().unwrap());
                        i += 2;
                    }
                    other => panic!("bad escape {}", other as char),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
}

/// Every `key = "…"` string value in a line, unescaped, in order.
fn string_values(line: &str) -> Vec<String> {
    let mut vals = Vec::new();
    let mut rest = line;
    while let Some(i) = rest.find(" = \"") {
        let (v, after) = unescape(&rest[i + 3..]);
        vals.push(v);
        rest = after;
    }
    vals
}

mod window_layer;
mod workspace;
