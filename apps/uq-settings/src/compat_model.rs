//! The Apps and scaling page's QML object: [`CompatState`] driven by `crate::model_core`.
//!
//! Like `rules_model.rs`, this file sits in `src/` because cxx-qt-build supports one directory
//! of bridge files per QML module (QTBUG-93443).

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde_json::{Value, json};

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::compat::{self, CompatState, Op};

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
        /// `{switches, browsers, blurry, fixes, installed, fields, reopen, section}` (see
        /// `pages::compat`); `{}` until uqd has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        type CompatModel = super::CompatModelRust;

        /// Read the switches, the fixes and the open windows again.
        #[qinvokable]
        fn refresh(self: Pin<&mut CompatModel>);

        /// Read only the open windows again (for "Blurry right now").
        #[qinvokable]
        fn refresh_windows(self: Pin<&mut CompatModel>);

        /// Turn one of the five switches on or off; `key` is its catalogue key.
        #[qinvokable]
        fn set_switch(self: Pin<&mut CompatModel>, key: &QString, on: bool);

        /// Add a fix, or replace the one for the same app; `fix_json` is the fix as JSON.
        #[qinvokable]
        fn save_fix(self: Pin<&mut CompatModel>, fix_json: &QString);

        /// Remove the fix for app `id`.
        #[qinvokable]
        fn remove_fix(self: Pin<&mut CompatModel>, id: &QString);

        /// "Also when started from other launchers" for the fix of app `id`.
        #[qinvokable]
        fn set_elsewhere(self: Pin<&mut CompatModel>, id: &QString, on: bool);

        /// What a draft fix adds to the app's command: `{env, args, problem}`. Synchronous, no
        /// request to uqd, never busy, never sets `error`.
        #[qinvokable]
        fn preview(self: &CompatModel, fix_json: &QString) -> QString;
    }

    impl cxx_qt::Threading for CompatModel {}
    impl cxx_qt::Initialize for CompatModel {}
}

use qobject::CompatModel;

pub struct CompatModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<CompatState>,
}

impl Default for CompatModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(CompatState::new),
        }
    }
}

impl PageModel for CompatModel {
    type Logic = CompatState;
    // An `env` change can change a switch's source; history covers undo and restore. Coming
    // back to the window re-reads through the page's `refresh()`.
    const FOLLOW: Follow = Follow {
        sections: &["compat", "env"],
        theme: false,
        settings: false,
        history: true,
        monitors: false,
        profiles: false,
        autostart: false,
    };

    fn core(&self) -> &ModelCore<CompatState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<CompatState> {
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

impl cxx_qt::Initialize for CompatModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

/// The error for a request from the page whose JSON argument doesn't parse (a programming error).
fn unreadable(e: serde_json::Error) -> QString {
    QString::from(&format!("That request couldn't be read ({e})."))
}

impl CompatModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn refresh_windows(self: Pin<&mut Self>) {
        model_core::start(self, Op::RefreshWindows);
    }

    fn set_switch(self: Pin<&mut Self>, key: &QString, on: bool) {
        model_core::start(
            self,
            Op::SetSwitch {
                key: key.to_string(),
                on,
            },
        );
    }

    fn save_fix(self: Pin<&mut Self>, fix_json: &QString) {
        match serde_json::from_str(&fix_json.to_string()) {
            Ok(fix) => model_core::start(self, Op::SaveFix { fix }),
            Err(e) => self.set_error(unreadable(e)),
        }
    }

    fn remove_fix(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::RemoveFix { id: id.to_string() });
    }

    fn set_elsewhere(self: Pin<&mut Self>, id: &QString, on: bool) {
        model_core::start(
            self,
            Op::SetElsewhere {
                id: id.to_string(),
                on,
            },
        );
    }

    fn preview(&self, fix_json: &QString) -> QString {
        let answer = match serde_json::from_str::<Value>(&fix_json.to_string()) {
            Ok(fix) => compat::preview(&fix),
            Err(_) => json!({"env": [], "args": [], "problem": "this fix can't be read"}),
        };
        QString::from(&answer.to_string())
    }
}
