//! The Window rules page's QML object: [`RulesState`] driven by `crate::model_core`.
//!
//! Like `profiles_model.rs`, this file sits in `src/` because cxx-qt-build supports one directory
//! of bridge files per QML module (QTBUG-93443).

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde_json::{Value, json};

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::rules::{self, Op, RulesState};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        /// `{window, layer, workspace, section, windows, why, pick, reopen, templates, fields,
        /// limits, notice}` (see `pages::rules`); `{}` until uqd has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight (a pick stays in flight until it ends).
        #[qproperty(bool, busy)]
        type RulesModel = super::RulesModelRust;

        /// Read the rules and the open windows again.
        #[qinvokable]
        fn refresh(self: Pin<&mut RulesModel>);

        /// Read only the open windows again (for "Matches now").
        #[qinvokable]
        fn refresh_windows(self: Pin<&mut RulesModel>);

        /// Add or replace a rule; `kind` is "window", "layer" or "workspace" and `rule_json` the
        /// rule as JSON. `replaces` is "" for a new rule, else the id (or workspace) of the rule
        /// the editor was opened on.
        #[qinvokable]
        fn save(
            self: Pin<&mut RulesModel>,
            kind: &QString,
            rule_json: &QString,
            replaces: &QString,
        );

        /// Whether a draft can be saved as `save` would save it (against the rules last read):
        /// `{valid, problems}`, problems in plain words. Synchronous, no request to uqd, never
        /// busy, never sets `error`.
        #[qinvokable]
        fn check(
            self: &RulesModel,
            kind: &QString,
            draft_json: &QString,
            replaces: &QString,
        ) -> QString;

        /// Delete the rule with this id (or workspace selector).
        #[qinvokable]
        fn remove(self: Pin<&mut RulesModel>, kind: &QString, key: &QString);

        /// Move a rule to position `to` of its list.
        #[qinvokable]
        fn move_to(self: Pin<&mut RulesModel>, kind: &QString, key: &QString, to: i32);

        /// Turn a window or panel rule on or off.
        #[qinvokable]
        fn set_enabled(self: Pin<&mut RulesModel>, kind: &QString, key: &QString, on: bool);

        /// Add a rule from template `id`; "" for an app or workspace it doesn't need.
        #[qinvokable]
        fn from_template(
            self: Pin<&mut RulesModel>,
            id: &QString,
            app: &QString,
            workspace: &QString,
            silent: bool,
        );

        /// Whether a draft window rule can be saved and which open windows it matches:
        /// `{valid, error, matches}`. Synchronous, no request to uqd, never busy, never sets
        /// `error`.
        #[qinvokable]
        fn preview(self: &RulesModel, draft_json: &QString) -> QString;

        /// A pattern that matches exactly `text`.
        #[qinvokable]
        fn exact(self: &RulesModel, text: &QString) -> QString;

        /// Let the person click a window: `purpose` is "match" (fill a rule) or "why".
        #[qinvokable]
        fn pick(self: Pin<&mut RulesModel>, purpose: &QString);
    }

    impl cxx_qt::Threading for RulesModel {}
    impl cxx_qt::Initialize for RulesModel {}
}

use qobject::RulesModel;

pub struct RulesModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<RulesState>,
}

impl Default for RulesModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(RulesState::new),
        }
    }
}

impl PageModel for RulesModel {
    type Logic = RulesState;
    // A workspace rule row shows the layout from `layouts`; history covers undo and restore.
    // Coming back to the window (after a pick, too) re-reads through the page's `refresh()`.
    const FOLLOW: Follow = Follow {
        sections: &["rules", "layouts"],
        theme: false,
        settings: false,
        history: true,
        monitors: false,
        // "From the X profile" follows a profile turning on or off.
        profiles: true,
    };

    fn core(&self) -> &ModelCore<RulesState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<RulesState> {
        &mut self.rust_mut().get_mut().core
    }
    fn show_busy(self: Pin<&mut Self>, busy: bool) {
        self.set_busy(busy);
    }
    fn show_error(self: Pin<&mut Self>, e: QString) {
        self.set_error(e);
    }
    fn show_state(self: Pin<&mut Self>, json: QString) {
        self.set_state_json(json);
    }
}

impl cxx_qt::Initialize for RulesModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

/// The error for a request from the page whose JSON argument doesn't parse (a programming error).
fn unreadable(e: serde_json::Error) -> QString {
    QString::from(&format!("That request couldn't be read ({e})."))
}

/// An empty text field means "not given".
fn given(text: &QString) -> Option<String> {
    let s = text.to_string();
    (!s.trim().is_empty()).then_some(s)
}

impl RulesModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn refresh_windows(self: Pin<&mut Self>) {
        model_core::start(self, Op::RefreshWindows);
    }

    fn save(self: Pin<&mut Self>, kind: &QString, rule_json: &QString, replaces: &QString) {
        match serde_json::from_str(&rule_json.to_string()) {
            Ok(rule) => model_core::start(
                self,
                Op::Save {
                    kind: kind.to_string(),
                    rule,
                    replaces: given(replaces),
                },
            ),
            Err(e) => self.set_error(unreadable(e)),
        }
    }

    fn check(&self, kind: &QString, draft_json: &QString, replaces: &QString) -> QString {
        let answer = match serde_json::from_str::<Value>(&draft_json.to_string()) {
            Ok(draft) => match self.core().logic() {
                Some(logic) => logic.check(&kind.to_string(), &draft, given(replaces)),
                None => json!({"valid": false, "problems": ["uQ isn't running"]}),
            },
            Err(_) => json!({"valid": false, "problems": ["this rule can't be read"]}),
        };
        QString::from(&answer.to_string())
    }

    fn remove(self: Pin<&mut Self>, kind: &QString, key: &QString) {
        model_core::start(
            self,
            Op::Delete {
                kind: kind.to_string(),
                key: key.to_string(),
            },
        );
    }

    fn move_to(self: Pin<&mut Self>, kind: &QString, key: &QString, to: i32) {
        model_core::start(
            self,
            Op::Move {
                kind: kind.to_string(),
                key: key.to_string(),
                to: usize::try_from(to).unwrap_or(0),
            },
        );
    }

    fn set_enabled(self: Pin<&mut Self>, kind: &QString, key: &QString, on: bool) {
        model_core::start(
            self,
            Op::SetEnabled {
                kind: kind.to_string(),
                key: key.to_string(),
                on,
            },
        );
    }

    fn from_template(
        self: Pin<&mut Self>,
        id: &QString,
        app: &QString,
        workspace: &QString,
        silent: bool,
    ) {
        model_core::start(
            self,
            Op::FromTemplate {
                template: id.to_string(),
                app: given(app),
                workspace: given(workspace),
                silent,
            },
        );
    }

    fn preview(&self, draft_json: &QString) -> QString {
        let answer = match serde_json::from_str::<Value>(&draft_json.to_string()) {
            Ok(draft) => match self.core().logic() {
                Some(logic) => logic.preview(&draft),
                None => json!({"valid": false, "error": "uQ isn't running", "matches": []}),
            },
            Err(_) => json!({"valid": false, "error": "this rule can't be read", "matches": []}),
        };
        QString::from(&answer.to_string())
    }

    fn exact(&self, text: &QString) -> QString {
        QString::from(&rules::exact(&text.to_string()))
    }

    fn pick(self: Pin<&mut Self>, purpose: &QString) {
        model_core::start(
            self,
            Op::Pick {
                purpose: purpose.to_string(),
            },
        );
    }
}
