use super::*;
use crate::keybinds::combo::parse;
use std::collections::BTreeSet;

fn base_lua() -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/hypr/base.lua");
    std::fs::read_to_string(p).expect("base.lua")
}

#[test]
fn the_terminal_runs_in_its_own_app_scope() {
    let t = defaults().iter().find(|d| d.id == "uq.terminal").unwrap();
    assert_eq!(t.action, DefaultAction::Exec(TERMINAL));
    // The chosen terminal first, through uqctl.
    assert!(
        TERMINAL.starts_with("uqctl launch --terminal 2>/dev/null || systemd-run --user --scope "),
        "{TERMINAL}"
    );
    assert!(TERMINAL.contains(" --slice=app.slice "), "{TERMINAL}");
    // kyko, and kitty where kyko is not installed.
    assert!(
        TERMINAL.ends_with(" -- sh -c 'command -v kyko >/dev/null && exec kyko; exec kitty'"),
        "{TERMINAL}"
    );
    assert!(base_lua().contains(&format!("local terminal = \"{TERMINAL}\"")));
}

#[test]
fn table_has_60_rows_58_bound() {
    assert_eq!(defaults().len(), 68);
    assert_eq!(defaults().iter().filter(|d| d.combo.is_some()).count(), 66);
}

#[test]
fn ids_unique_and_well_formed() {
    let mut seen = BTreeSet::new();
    for d in defaults() {
        assert!(seen.insert(d.id), "{}", d.id);
        let rest = d.id.strip_prefix("uq.").expect(d.id);
        assert!((1..=48).contains(&rest.len()), "{}", d.id);
        assert!(
            rest.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-')),
            "{}",
            d.id
        );
        assert!(!d.label.is_empty());
    }
    assert_eq!(
        default_of("uq.lookseedo").unwrap().label,
        "Open look=see>do_"
    );
    assert!(default_of("uq.nope").is_none());
}

#[test]
fn profile_next_is_unbound_and_runs_uqctl() {
    let d = default_of(PROFILE_NEXT).unwrap();
    assert_eq!(d.combo, None);
    assert_eq!(d.action.lua(), "hl.dsp.exec_cmd(\"uqctl profile next\")");
    assert_eq!(
        default_of(LOCK_RECOVERY).unwrap().combo,
        Some("SUPER + CTRL + L")
    );
}

#[test]
fn default_combos_parse_and_never_conflict() {
    let mut seen = BTreeSet::new();
    for d in defaults().iter().filter(|d| d.combo.is_some()) {
        let c = parse(d.combo.unwrap()).unwrap_or_else(|e| panic!("{}: {e}", d.id));
        assert!(seen.insert(c), "{} repeats a combo", d.id);
    }
    assert_eq!(seen.len(), 66);
}

#[test]
fn lua_forms_match_base_lua_text() {
    let base = base_lua();
    assert!(base.contains(&format!("local terminal = \"{TERMINAL}\"")));
    for d in defaults().iter().filter(|d| d.combo.is_some()) {
        let lua = d.action.lua();
        let (loop_row, text) = match d.id.split_once("uq.ws.") {
            Some((_, rest)) if rest.starts_with("focus.") || rest.starts_with("move.") => {
                let n = rest.rsplit('.').next().unwrap();
                (
                    true,
                    lua.replace(&format!("workspace = {n} "), "workspace = i "),
                )
            }
            _ => (false, lua.clone()),
        };
        let text = if d.id == "uq.terminal" {
            "hl.dsp.exec_cmd(terminal)".to_string()
        } else {
            text
        };
        assert!(base.contains(&text), "{}: {text} (loop: {loop_row})", d.id);
    }
}

#[test]
fn locked_and_repeating_as_base_lua() {
    let locked: Vec<&str> = defaults()
        .iter()
        .filter(|d| d.locked)
        .map(|d| d.id)
        .collect();
    assert_eq!(
        locked,
        [
            "uq.lock-recovery",
            "uq.vol-up",
            "uq.vol-down",
            "uq.mute",
            "uq.mic-mute",
            "uq.bright-up",
            "uq.bright-down",
            "uq.media.next",
            "uq.media.pause",
            "uq.media.play",
            "uq.media.prev"
        ]
    );
    let rep: Vec<&str> = defaults()
        .iter()
        .filter(|d| d.repeating)
        .map(|d| d.id)
        .collect();
    assert_eq!(
        rep,
        ["uq.vol-up", "uq.vol-down", "uq.bright-up", "uq.bright-down"]
    );
    // The launcher rows are never locked; base.lua says the same line by line.
    for id in [
        "uq.lookseedo",
        "uq.emoji",
        "uq.clip",
        "uq.appmenu",
        "uq.quick-settings",
        "uq.notif-centre",
        "uq.lock",
        "uq.power-menu",
    ] {
        assert!(!default_of(id).unwrap().locked, "{id}");
    }
    let base = base_lua();
    for d in defaults()
        .iter()
        .filter(|d| d.combo.is_some() && d.id != "uq.terminal")
    {
        let lua = d.action.lua();
        if d.id.starts_with("uq.ws.focus") || d.id.starts_with("uq.ws.move") {
            continue;
        }
        let line = base.lines().find(|l| l.contains(&lua)).expect(d.id);
        assert_eq!(line.contains("locked = true"), d.locked, "{}", d.id);
        assert_eq!(line.contains("repeating = true"), d.repeating, "{}", d.id);
        assert_eq!(line.contains("release = true"), d.release, "{}", d.id);
        assert_eq!(line.contains("mouse = true"), d.mouse, "{}", d.id);
    }
}
