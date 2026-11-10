//! One-click rule templates. Each gives a ready window rule (no id yet); the page asks for the
//! app and workspace a template needs.

use super::{WindowEffects, WindowMatch, WindowRule, exact_pattern};
use crate::settings::{SettingError, clean_text_spaced, invalid};
use serde::Serialize;

/// What a template asks the person for.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Needs {
    App,
    AppAndWorkspace,
    Nothing,
}

pub struct Template {
    pub id: &'static str,
    pub name: &'static str,
    pub needs: Needs,
}

pub const TEMPLATES: [Template; 5] = [
    Template {
        id: "float_dialogs",
        name: "Float an app's dialogs",
        needs: Needs::App,
    },
    Template {
        id: "open_on_workspace",
        name: "Open an app on a workspace",
        needs: Needs::AppAndWorkspace,
    },
    Template {
        id: "no_blur",
        name: "No blur for an app",
        needs: Needs::App,
    },
    Template {
        id: "pip",
        name: "Picture-in-picture floats and stays on top",
        needs: Needs::Nothing,
    },
    Template {
        id: "keep_awake",
        name: "Keep the screen awake while an app is fullscreen",
        needs: Needs::App,
    },
];

const PIP_TITLE: &str = "^(Picture-in-Picture|Picture in picture)$";

/// The last dot-segment of an app ID, capitalised: `org.pulseaudio.pavucontrol` is "Pavucontrol".
fn app_word(app: &str) -> String {
    let seg = app.rsplit('.').find(|s| !s.is_empty()).unwrap_or(app);
    let seg = clean_text_spaced(seg, 40);
    let mut cs = seg.chars();
    match cs.next() {
        Some(c) => c.to_uppercase().chain(cs).collect(),
        None => seg,
    }
}

/// Builds the rule for template `id`. `app` is an app ID (taken exactly), `workspace` a
/// workspace selector, and `silent` opens the window without switching to its workspace.
pub fn build(
    id: &str,
    app: Option<&str>,
    workspace: Option<&str>,
    silent: bool,
) -> Result<WindowRule, SettingError> {
    let t = TEMPLATES
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| invalid("template", "no such template"))?;
    let app = match t.needs {
        Needs::Nothing => None,
        _ => match app.map(str::trim) {
            Some(a) if !a.is_empty() => Some(a),
            _ => return Err(invalid("app", "choose an app first")),
        },
    };
    let word = app.map(app_word).unwrap_or_default();
    let class = app.map(exact_pattern);
    let mut matches = WindowMatch {
        class,
        ..Default::default()
    };
    let mut effects = WindowEffects::default();
    let name = match id {
        "float_dialogs" => {
            matches.modal = Some(true);
            effects.float = Some(true);
            effects.center = Some(true);
            format!("{word} dialogs float")
        }
        "open_on_workspace" => {
            let ws = workspace.map(str::trim).filter(|w| !w.is_empty());
            let Some(ws) = ws else {
                return Err(invalid("workspace", "choose a workspace first"));
            };
            let sel = crate::settings::rule_selector(ws).map_err(|e| match e {
                SettingError::Invalid { reason, .. } => invalid("workspace", reason),
                other => other,
            })?;
            effects.workspace = Some(sel);
            effects.silent = silent.then_some(true);
            format!(
                "{word} opens on workspace {}",
                ws.trim_start_matches("name:")
            )
        }
        "no_blur" => {
            effects.no_blur = Some(true);
            format!("{word} has no blur")
        }
        "pip" => {
            matches.title = Some(PIP_TITLE.to_string());
            effects.float = Some(true);
            effects.pin = Some(true);
            effects.keep_aspect_ratio = Some(true);
            "Picture-in-picture floats".to_string()
        }
        _ => {
            effects.idle_inhibit = Some("fullscreen".to_string());
            format!("{word} keeps the screen awake in fullscreen")
        }
    };
    Ok(WindowRule {
        id: String::new(),
        name: clean_text_spaced(&name, super::MAX_NAME_CHARS),
        enabled: true,
        matches,
        effects,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Rules;
    use serde_json::json;

    #[test]
    fn each_template_builds_a_valid_rule() {
        for t in &TEMPLATES {
            let (app, ws) = match t.needs {
                Needs::App => (Some("org.example.App"), None),
                Needs::AppAndWorkspace => (Some("org.example.App"), Some("3")),
                Needs::Nothing => (None, None),
            };
            let rule = build(t.id, app, ws, true).unwrap();
            assert_eq!(rule.id, "");
            assert!(rule.enabled);
            assert!(!rule.name.is_empty());
            let v = json!({"window": [rule]});
            Rules::from_section(&v, &Rules::default()).unwrap_or_else(|e| panic!("{}: {e}", t.id));
        }
        assert_eq!(TEMPLATES.len(), 5);
    }

    #[test]
    fn templates_need_their_inputs() {
        assert!(build("float_dialogs", None, None, false).is_err());
        assert!(build("float_dialogs", Some("  "), None, false).is_err());
        assert!(build("open_on_workspace", Some("a"), None, false).is_err());
        assert!(build("open_on_workspace", None, Some("3"), false).is_err());
        assert!(build("nope", Some("a"), None, false).is_err());
        let r = build("open_on_workspace", Some("a"), Some("3"), true).unwrap();
        assert_eq!(r.effects.workspace.as_deref(), Some("3"));
        assert_eq!(r.effects.silent, Some(true));
        let r = build("open_on_workspace", Some("a"), Some("3"), false).unwrap();
        assert_eq!(r.effects.silent, None);
    }

    #[test]
    fn pip_template_needs_no_app() {
        let r = build("pip", None, None, false).unwrap();
        assert_eq!(
            r.matches.title.as_deref(),
            Some("^(Picture-in-Picture|Picture in picture)$")
        );
        assert!(r.matches.class.is_none());
        let e = r.effects;
        assert_eq!(
            (e.float, e.pin, e.keep_aspect_ratio),
            (Some(true), Some(true), Some(true))
        );
    }

    #[test]
    fn app_templates_use_exact_class() {
        let r = build(
            "float_dialogs",
            Some("org.pulseaudio.pavucontrol"),
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            r.matches.class.as_deref(),
            Some("^org\\.pulseaudio\\.pavucontrol$")
        );
        assert_eq!(r.matches.modal, Some(true));
        assert_eq!(
            (r.effects.float, r.effects.center),
            (Some(true), Some(true))
        );
        assert_eq!(r.name, "Pavucontrol dialogs float");
        let r = build("no_blur", Some("firefox"), None, false).unwrap();
        assert_eq!(r.effects.no_blur, Some(true));
        assert_eq!(r.name, "Firefox has no blur");
        let r = build("keep_awake", Some("mpv"), None, false).unwrap();
        assert_eq!(r.effects.idle_inhibit.as_deref(), Some("fullscreen"));
    }
}
