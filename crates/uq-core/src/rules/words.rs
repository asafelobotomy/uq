//! Plain-words text for rules: what they look for and what they do. No Hyprland names.

use super::hypr;
use super::{LayerEffects, WindowEffects, WindowMatch, WorkspaceRule};

/// A selector as people say it: `name:web` is "web".
fn ws_words(sel: &str) -> &str {
    sel.strip_prefix("name:").unwrap_or(sel)
}

fn on_off(out: &mut Vec<String>, v: Option<bool>, yes: &str, no: &str) {
    if let Some(b) = v {
        out.push(if b { yes } else { no }.to_string());
    }
}

/// What a window rule does, in plain lines.
pub fn effects_text(e: &WindowEffects) -> Vec<String> {
    let mut o = Vec::new();
    on_off(&mut o, e.float, "Floats", "Doesn't float");
    on_off(&mut o, e.tile, "Tiles", "Doesn't tile");
    on_off(
        &mut o,
        e.fullscreen,
        "Opens fullscreen",
        "Doesn't open fullscreen",
    );
    on_off(
        &mut o,
        e.maximize,
        "Opens maximised",
        "Doesn't open maximised",
    );
    if let Some(ws) = &e.workspace {
        let quiet = if e.silent == Some(true) {
            ", without switching to it"
        } else {
            ""
        };
        o.push(format!("Opens on workspace {}{quiet}", ws_words(ws)));
    }
    if let Some(m) = &e.monitor {
        o.push(format!("Opens on screen {m}"));
    }
    if let Some([w, h]) = e.size {
        o.push(format!("Size: {w} by {h}"));
    }
    if let Some([x, y]) = e.position {
        o.push(format!("Position: {x}, {y}"));
    }
    on_off(
        &mut o,
        e.center,
        "Opens in the middle of the screen",
        "Isn't centred",
    );
    on_off(
        &mut o,
        e.pin,
        "Stays on top on every workspace",
        "Isn't kept on top",
    );
    on_off(
        &mut o,
        e.no_initial_focus,
        "Doesn't take focus when it opens",
        "Takes focus when it opens",
    );
    if let Some(x) = e.opacity {
        o.push(format!("Opacity: {} %", (x * 100.0).round() as i64));
    }
    on_off(&mut o, e.no_blur, "No blur", "Blur stays on");
    on_off(&mut o, e.no_shadow, "No shadow", "Shadow stays on");
    on_off(&mut o, e.no_anim, "No animations", "Animations stay on");
    if let Some(r) = e.rounding {
        o.push(format!("Corner rounding: {r}"));
    }
    if let Some(b) = e.border_size {
        o.push(format!("Border size: {b}"));
    }
    if let Some(i) = &e.idle_inhibit {
        o.push(
            match i.as_str() {
                "always" => "Keeps the screen awake",
                "focus" => "Keeps the screen awake while it has focus",
                "fullscreen" => "Keeps the screen awake while fullscreen",
                _ => "Doesn't keep the screen awake",
            }
            .to_string(),
        );
    }
    on_off(
        &mut o,
        e.keep_aspect_ratio,
        "Keeps its shape when resized",
        "Doesn't keep its shape when resized",
    );
    if let Some(t) = &e.tag {
        o.push(format!("Adds the tag {t}"));
    }
    o
}

/// What a panel rule does, in plain lines.
pub fn layer_effects_text(e: &LayerEffects) -> Vec<String> {
    let mut o = Vec::new();
    on_off(&mut o, e.blur, "Blurred", "Not blurred");
    if let Some(a) = e.ignore_alpha {
        o.push(format!(
            "Blur ignores faint pixels below {} %",
            (a * 100.0).round() as i64
        ));
    }
    on_off(&mut o, e.no_anim, "No animations", "Animations stay on");
    if let Some(a) = &e.animation {
        let style = if a == "popin" { "Pop in" } else { a.as_str() };
        o.push(format!("Animation style: {style}"));
    }
    o
}

/// What a workspace rule sets, in plain lines.
pub fn workspace_text(r: &WorkspaceRule) -> Vec<String> {
    let mut o = Vec::new();
    if let Some(m) = &r.monitor {
        o.push(format!("Lives on screen {m}"));
    }
    on_off(
        &mut o,
        r.default,
        "Is the default workspace",
        "Isn't the default workspace",
    );
    on_off(
        &mut o,
        r.persistent,
        "Is kept when empty",
        "Goes away when empty",
    );
    if let Some(g) = r.gaps_in {
        o.push(format!("Gap between windows: {g}"));
    }
    if let Some(g) = r.gaps_out {
        o.push(format!("Gap at the screen edge: {g}"));
    }
    if let Some(b) = r.border_size {
        o.push(format!("Border size: {b}"));
    }
    o
}

/// What a window rule looks for, in plain lines.
pub fn match_text(m: &WindowMatch) -> Vec<String> {
    let mut o = Vec::new();
    let label = |k: &'static str| hypr::match_key(k).map_or(k, |f| f.label);
    for (k, p) in [
        ("class", &m.class),
        ("title", &m.title),
        ("initial_class", &m.initial_class),
        ("initial_title", &m.initial_title),
    ] {
        if let Some(p) = p {
            o.push(format!("{} matches “{p}”", label(k)));
        }
    }
    if let Some(t) = &m.tag {
        o.push(format!("Has the tag {t}"));
    }
    if let Some(w) = &m.workspace {
        o.push(format!("Is on workspace {}", ws_words(w)));
    }
    on_off(
        &mut o,
        m.xwayland,
        "Is an older X11 app",
        "Isn't an older X11 app",
    );
    on_off(&mut o, m.floating, "Is floating", "Isn't floating");
    on_off(&mut o, m.fullscreen, "Is fullscreen", "Isn't fullscreen");
    on_off(&mut o, m.pinned, "Is pinned", "Isn't pinned");
    on_off(&mut o, m.modal, "Is a dialog", "Isn't a dialog");
    o
}

/// Whether any effect only applies to windows that open after the rule is saved.
pub fn has_static(e: &WindowEffects) -> bool {
    let set = [
        ("float", e.float.is_some()),
        ("tile", e.tile.is_some()),
        ("fullscreen", e.fullscreen.is_some()),
        ("maximize", e.maximize.is_some()),
        ("workspace", e.workspace.is_some()),
        ("monitor", e.monitor.is_some()),
        ("size", e.size.is_some()),
        ("move", e.position.is_some()),
        ("center", e.center.is_some()),
        ("pin", e.pin.is_some()),
        ("no_initial_focus", e.no_initial_focus.is_some()),
    ];
    set.iter().any(|(k, on)| *on && hypr::is_static(k))
}
