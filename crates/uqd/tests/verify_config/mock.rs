//! The Lua mock itself: what it rejects and sees.

use super::*;

/// Review Focus 2: Hyprland ignores an unknown `hl.bind` option, so the mock must not.
#[test]
fn mock_rejects_a_dashed_option() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("dashed.lua");
    std::fs::write(
        &f,
        "hl.bind(\"SUPER + Q\", hl.dsp.window.close(), { [\"non-consuming\"] = true })\n",
    )
    .unwrap();
    let (ok, out, err) = run_mock(&lua, dir.path(), true, &[&f]);
    assert!(!ok && err.contains("unknown hl.bind option"), "{out}{err}");
    // A misspelling with `_` is just as unknown.
    std::fs::write(
        &f,
        "hl.bind(\"SUPER + Q\", hl.dsp.window.close(), { repeting = true })\n",
    )
    .unwrap();
    let (ok, out, err) = run_mock(&lua, dir.path(), true, &[&f]);
    assert!(!ok && err.contains("unknown hl.bind option"), "{out}{err}");
}

/// The mock tells two binds apart that differ only in a table argument.
#[test]
fn mock_sees_table_arguments() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("t.lua");
    std::fs::write(
        &f,
        "hl.bind(\"SUPER + 3\", hl.dsp.focus({ workspace = 3 }))\n\
         hl.bind(\"SUPER + 4\", hl.dsp.focus({ workspace = 4 }))\n\
         hl.unbind(\"all\")\n\
         hl.bind(\"SUPER + 5\", hl.dsp.focus({ workspace = 5 }), { locked = true })\n",
    )
    .unwrap();
    let (ok, out, err) = run_mock(&lua, dir.path(), true, &[&f]);
    assert!(ok, "{out}{err}");
    assert_eq!(
        bind_lines(&out),
        ["bind SUPER + 5 | hl.dsp.focus{workspace=5} | locked=true"]
    );
}
