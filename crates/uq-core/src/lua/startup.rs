//! The xwayland option and the session `hl.env` lines (Startup, and Apps and scaling pages).

use super::lua_string;
use crate::env;
use crate::state::State;
use std::fmt::Write as _;

/// What the Startup and Apps and scaling pages add to `generated.lua`.
#[derive(Debug, Clone, PartialEq)]
pub struct StartupRender {
    /// `xwayland.force_zero_scaling` (Sharp X11 apps).
    pub zero_scaling: bool,
    /// The session variables, in order (`env::session_env`).
    pub env: Vec<(String, String)>,
    /// The third `hl.env` argument: whether children of Hyprland see them. Off in a dev session.
    pub export: bool,
}

impl Default for StartupRender {
    fn default() -> Self {
        StartupRender {
            zero_scaling: false,
            env: Vec::new(),
            export: true,
        }
    }
}

pub fn startup_render(state: &State, main: Option<f64>, dev_session: bool) -> StartupRender {
    StartupRender {
        zero_scaling: state
            .compat
            .switch(crate::compat::hypr::SwitchKey::SharpX11),
        env: env::session_env(&state.env, &state.compat, main),
        export: !dev_session,
    }
}

/// Nothing when there are no variables.
pub fn emit_startup_env(out: &mut String, r: &StartupRender) {
    if r.env.is_empty() {
        return;
    }
    let _ = writeln!(out, "\n-- Environment (Startup and Apps and scaling pages)");
    for (name, value) in &r.env {
        let _ = writeln!(
            out,
            "hl.env({}, {}, {})",
            lua_string(name),
            lua_string(value),
            r.export
        );
    }
}

/// Inside `hl.config({ … })`.
pub fn emit_xwayland(out: &mut String, r: &StartupRender) {
    if r.zero_scaling {
        out.push_str("    xwayland = {\n        force_zero_scaling = true,\n    },\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(env: &[(&str, &str)]) -> StartupRender {
        StartupRender {
            env: env
                .iter()
                .map(|(n, v)| (n.to_string(), v.to_string()))
                .collect(),
            ..StartupRender::default()
        }
    }

    #[test]
    fn xwayland_only_when_on() {
        let mut out = String::new();
        emit_xwayland(&mut out, &StartupRender::default());
        assert_eq!(out, "");
        let on = StartupRender {
            zero_scaling: true,
            ..StartupRender::default()
        };
        emit_xwayland(&mut out, &on);
        assert_eq!(
            out,
            "    xwayland = {\n        force_zero_scaling = true,\n    },\n"
        );
    }

    #[test]
    fn env_lines_in_order_compat_last() {
        let mut out = String::new();
        emit_startup_env(
            &mut out,
            &r(&[("A", "1"), ("QT_QPA_PLATFORM", "wayland;xcb")]),
        );
        assert_eq!(
            out,
            "\n-- Environment (Startup and Apps and scaling pages)\n\
             hl.env(\"A\", \"1\", true)\n\
             hl.env(\"QT_QPA_PLATFORM\", \"wayland;xcb\", true)\n"
        );
    }

    #[test]
    fn export_false_in_dev_session() {
        let mut x = r(&[("A", "1")]);
        x.export = false;
        let mut out = String::new();
        emit_startup_env(&mut out, &x);
        assert!(out.contains("hl.env(\"A\", \"1\", false)\n"));
        let st = State::default();
        assert!(!startup_render(&st, None, true).export);
        assert!(startup_render(&st, None, false).export);
    }

    #[test]
    fn startup_render_follows_state() {
        use crate::compat::hypr::SwitchKey;
        let mut st = State::default();
        st.env.vars.push(crate::env::Var {
            name: "A".into(),
            value: "1".into(),
        });
        let off = startup_render(&st, Some(1.5), false);
        assert!(!off.zero_scaling);
        assert_eq!(off.env, vec![("A".to_string(), "1".to_string())]);
        st.compat.set_switch(SwitchKey::SharpX11, true);
        let on = startup_render(&st, Some(1.5), true);
        assert!(on.zero_scaling);
        assert!(!on.export);
        assert_eq!(on.env, env::session_env(&st.env, &st.compat, Some(1.5)));
        let gdk = crate::compat::launch::gdk_scale(1.5).to_string();
        assert!(on.env.contains(&("GDK_SCALE".to_string(), gdk)));
    }

    #[test]
    fn no_env_block_when_empty() {
        let mut out = String::new();
        emit_startup_env(&mut out, &StartupRender::default());
        assert_eq!(out, "");
    }

    #[test]
    fn hostile_env_stays_inside_lua_strings() {
        let mut out = String::new();
        emit_startup_env(
            &mut out,
            &r(&[("A", "\") hl.exec_cmd(\"x"), ("B", "]]--"), ("C", "a\\")]),
        );
        assert_eq!(out.matches("hl.env(").count(), 3, "{out}");
        assert!(!out.contains("hl.exec_cmd(\"x\")"));
        assert!(out.contains("hl.env(\"A\", \"\\\") hl.exec_cmd(\\\"x\", true)\n"));
        assert!(out.contains("hl.env(\"B\", \"]]--\", true)\n"));
        assert!(out.contains("hl.env(\"C\", \"a\\\\\", true)\n"));
        // Every line is one call: nothing the values hold starts a new statement.
        assert_eq!(out.lines().filter(|l| l.starts_with("hl.")).count(), 3);
    }
}
