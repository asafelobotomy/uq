//! The `-- Keyboard shortcuts` block of `generated.lua` (R1, R5, R11).

use super::lua_string;
use crate::keybinds::combo::canonical;
use crate::keybinds::hypr::{
    BIND_OPTIONS, CAPTURE_PLACEHOLDER, CAPTURE_SUBMAP, DESCRIPTION_PREFIX, UNBIND_ALL,
};
use crate::keybinds::{Effective, Keybinds};
use std::fmt::Write as _;

/// The block: a comment, `hl.unbind("all")` (so `base.lua`'s fallback copies go), the empty
/// capture submap, then one `hl.bind` per enabled shortcut with keys. Defaults come first in
/// table order with overrides applied, then your own in list order. Always emitted. Ends with a
/// newline.
pub fn emit_binds(kb: &Keybinds) -> String {
    let mut out = String::from(
        "-- Keyboard shortcuts (uQ Settings). These replace base.lua's fallback copies.\n",
    );
    let _ = writeln!(out, "{UNBIND_ALL}");
    let _ = writeln!(
        out,
        "hl.define_submap({}, function() {} end)",
        lua_string(CAPTURE_SUBMAP),
        CAPTURE_PLACEHOLDER
    );
    for e in kb.effective() {
        if e.enabled && e.combo.is_some() {
            out.push_str(&emit_bind(&e));
            out.push('\n');
        }
    }
    out
}

/// One `hl.bind` line (no newline). Options come in `BIND_OPTIONS` order and only when true;
/// `description` is always `uq:<id>`. A shortcut's name never reaches the file.
pub fn emit_bind(e: &Effective) -> String {
    let mut opts = Vec::new();
    for o in BIND_OPTIONS {
        let on = match o.lua {
            "locked" => e.locked,
            "repeating" => e.repeating,
            "release" => e.release,
            "mouse" => e.mouse,
            "description" => false,
            other => unreachable!("bind option {other} has no state to render"),
        };
        if on {
            opts.push(format!("{} = true", o.lua));
        } else if o.lua == "description" {
            opts.push(format!(
                "description = {}",
                lua_string(&format!("{DESCRIPTION_PREFIX}{}", e.id))
            ));
        }
    }
    let keys = e.combo.map(canonical).unwrap_or_default();
    format!(
        "hl.bind({}, {}, {{ {} }})",
        lua_string(&keys),
        e.lua_action,
        opts.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybinds::{Action, CustomBind, Override};

    fn lines(s: &str) -> Vec<&str> {
        s.lines().collect()
    }

    fn custom(id: &str, name: &str, combo: &str, action: Action) -> CustomBind {
        CustomBind {
            id: id.into(),
            name: name.into(),
            combo: combo.into(),
            enabled: true,
            locked: false,
            repeating: false,
            action,
        }
    }

    #[test]
    fn default_block_golden() {
        let out = emit_binds(&Keybinds::default());
        let l = lines(&out);
        assert_eq!(
            l[0],
            "-- Keyboard shortcuts (uQ Settings). These replace base.lua's fallback copies."
        );
        assert_eq!(l[1], "hl.unbind(\"all\")");
        assert_eq!(
            l[2],
            format!(
                "hl.define_submap(\"uq_capture\", function() {} end)",
                CAPTURE_PLACEHOLDER
            )
        );
        assert_eq!(
            l[3],
            "hl.bind(\"SUPER + Return\", hl.dsp.exec_cmd(\"kitty\"), { description = \"uq:uq.terminal\" })"
        );
        assert_eq!(out.matches("hl.bind(").count(), 56);
        assert!(!out.contains("uq.profile.next"));
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn options_render_in_table_order() {
        let out = emit_binds(&Keybinds::default());
        let find = |id: &str| {
            out.lines()
                .find(|l| l.contains(&format!("\"uq:{id}\"")))
                .unwrap()
                .to_string()
        };
        assert!(
            find("uq.vol-up")
                .ends_with("{ locked = true, repeating = true, description = \"uq:uq.vol-up\" })")
        );
        assert!(
            find("uq.appmenu").ends_with("{ release = true, description = \"uq:uq.appmenu\" })")
        );
        assert!(
            find("uq.window.drag")
                .ends_with("{ mouse = true, description = \"uq:uq.window.drag\" })")
        );
    }

    #[test]
    fn override_rebinds_and_disables() {
        let kb = Keybinds {
            overrides: vec![
                Override {
                    id: "uq.spotlight".into(),
                    combo: Some("SUPER + K".into()),
                    enabled: None,
                },
                Override {
                    id: "uq.close".into(),
                    combo: None,
                    enabled: Some(false),
                },
            ],
            custom: vec![],
        };
        let out = emit_binds(&kb);
        assert!(out.contains("hl.bind(\"SUPER + K\", hl.dsp.global(\"uq:spotlight\")"));
        assert!(!out.contains("\"SUPER + Space\""));
        assert!(!out.contains("uq:uq.close"));
        assert_eq!(out.matches("hl.bind(").count(), 55);
    }

    #[test]
    fn profile_next_renders_once_bound() {
        let kb = Keybinds {
            overrides: vec![Override {
                id: "uq.profile.next".into(),
                combo: Some("SUPER + ALT + P".into()),
                enabled: None,
            }],
            custom: vec![],
        };
        let out = emit_binds(&kb);
        assert!(out.contains(
            "hl.bind(\"SUPER + ALT + P\", hl.dsp.exec_cmd(\"uqctl profile next\"), { description = \"uq:uq.profile.next\" })"
        ));
        assert_eq!(out.matches("hl.bind(").count(), 57);
    }

    #[test]
    fn custom_binds_follow_defaults_in_list_order() {
        let t = |n: &str| Action::Uq {
            name: n.into(),
            layout: None,
            media: None,
        };
        let kb = Keybinds {
            overrides: vec![],
            custom: vec![
                custom("k-0002", "B", "SUPER + F2", t("terminal")),
                custom("k-0001", "A", "SUPER + F1", t("settings")),
            ],
        };
        let out = emit_binds(&kb);
        let l = lines(&out);
        let n = l.len();
        assert!(l[n - 2].contains("SUPER + F2") && l[n - 2].contains("uq:k-0002"));
        assert!(l[n - 1].contains("SUPER + F1") && l[n - 1].contains("uq:k-0001"));
        assert!(l[n - 3].contains("uq:uq.media.prev"));
    }

    /// Reads one Lua short string starting at the `"` at `s[0]`; returns it and the rest.
    fn unlua(s: &str) -> (String, &str) {
        let b = s.as_bytes();
        assert_eq!(b[0], b'"');
        let mut out = Vec::new();
        let mut i = 1;
        while b[i] != b'"' {
            if b[i] == b'\\' {
                i += 1;
                match b[i] {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b'0'..=b'9' => {
                        let d: u32 = s[i..i + 3].parse().unwrap();
                        out.push(d as u8);
                        i += 2;
                    }
                    c => out.push(c),
                }
            } else {
                out.push(b[i]);
            }
            i += 1;
        }
        (String::from_utf8(out).unwrap(), &s[i + 1..])
    }

    #[test]
    fn hostile_strings_stay_inside_lua_and_sh() {
        let hostile = [
            "x'; $(id) \"; ]] \\ \") hl.exec_cmd(\"x",
            "a'b\"c\\d]]e$(id);f",
            "two\nlines\r\u{7f}\u{1}9",
            "''",
        ];
        for h in hostile {
            let kb = Keybinds {
                overrides: vec![],
                custom: vec![
                    custom(
                        "k-0001",
                        h,
                        "SUPER + F1",
                        Action::Command { command: h.into() },
                    ),
                    custom(
                        "k-0002",
                        h,
                        "SUPER + F2",
                        // Built past validation on purpose.
                        Action::Launch {
                            app: format!("{h}.desktop; rm -rf ~"),
                        },
                    ),
                ],
            };
            let out = emit_binds(&kb);
            let l = lines(&out);
            let base = l.len() - 2;
            assert_eq!(out.matches("hl.bind(").count(), 58, "{h:?}");
            for (line, is_command) in [(l[base], true), (l[base + 1], false)] {
                assert!(line.starts_with("hl.bind(\"SUPER + F"), "{line}");
                let arg_at = line.find("hl.dsp.exec_cmd(").unwrap() + "hl.dsp.exec_cmd(".len();
                let (cmd, rest) = unlua(&line[arg_at..]);
                assert!(rest.starts_with("), { description = \"uq:k-000"), "{rest}");
                let word = if is_command {
                    cmd.strip_prefix(
                        "systemd-run --user --scope --quiet --collect --slice=app.slice -- sh -c ",
                    )
                    .expect(&cmd)
                } else {
                    cmd.strip_prefix("uqctl launch ").expect(&cmd)
                };
                let want = if is_command {
                    h.to_string()
                } else {
                    format!("{h}.desktop; rm -rf ~")
                };
                if std::process::Command::new("sh")
                    .arg("-c")
                    .arg("true")
                    .status()
                    .is_ok()
                {
                    // The shell reads the recovered word as one argument: `sh -c 'printf %s <word>'`.
                    let o = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(format!("printf %s {word}"))
                        .output()
                        .unwrap();
                    assert_eq!(String::from_utf8_lossy(&o.stdout), want);
                }
            }
            // The name appears nowhere (not even its safe-looking parts).
            assert!(
                !out.contains("two\nlines") && !out.contains("a'b\"c"),
                "{h:?}"
            );
        }
    }
}
