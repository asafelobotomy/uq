//! The Look and layouts page's QML object: [`LookState`] driven by `crate::model_core`.
//!
//! This file sits in `src/` because cxx-qt-build supports one directory of bridge files per QML
//! module (QTBUG-93443), and `app.rs` and `window.rs` are there.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::look::{LookState, Op};

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
        /// `{look, layouts, defaults, sources}`; `{}` until uqd has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused change, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        type LookModel = super::LookModelRust;

        /// Read everything from uqd again.
        #[qinvokable]
        fn refresh(self: Pin<&mut LookModel>);

        /// Set one field of the `look` section to `json_value` (`null` = use the theme).
        #[qinvokable]
        fn set_look(self: Pin<&mut LookModel>, field: &QString, json_value: &QString);

        /// Set one layout option: `group` is "" (then `field` is `default`), `dwindle`, `master`
        /// or `scrolling`.
        #[qinvokable]
        fn set_layout_option(
            self: Pin<&mut LookModel>,
            group: &QString,
            field: &QString,
            json_value: &QString,
        );

        /// Give a workspace a layout; an empty `layout` removes its rule.
        #[qinvokable]
        fn set_workspace(self: Pin<&mut LookModel>, selector: &QString, layout: &QString);

        /// Apply `look` or `layouts` to the session again, over what changed it while running.
        #[qinvokable]
        fn reapply(self: Pin<&mut LookModel>, domain: &QString);
    }

    impl cxx_qt::Threading for LookModel {}
    impl cxx_qt::Initialize for LookModel {}
}

use qobject::LookModel;

pub struct LookModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<LookState>,
}

impl Default for LookModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(LookState::new),
        }
    }
}

impl PageModel for LookModel {
    type Logic = LookState;
    const FOLLOW: Follow = Follow {
        sections: &["look", "layouts"],
        theme: true,
        settings: false,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
        themes: false,
    };

    fn core(&self) -> &ModelCore<LookState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<LookState> {
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

impl cxx_qt::Initialize for LookModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl LookModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn reapply(self: Pin<&mut Self>, domain: &QString) {
        model_core::reapply(self, domain.to_string());
    }

    fn set_look(self: Pin<&mut Self>, field: &QString, json_value: &QString) {
        match serde_json::from_str(&json_value.to_string()) {
            Ok(value) => model_core::start(
                self,
                Op::SetLook {
                    field: field.to_string(),
                    value,
                },
            ),
            Err(e) => self.set_error(QString::from(&format!("{field}: not valid JSON ({e})"))),
        }
    }

    fn set_layout_option(
        self: Pin<&mut Self>,
        group: &QString,
        field: &QString,
        json_value: &QString,
    ) {
        match serde_json::from_str(&json_value.to_string()) {
            Ok(value) => model_core::start(
                self,
                Op::SetLayoutOption {
                    group: group.to_string(),
                    field: field.to_string(),
                    value,
                },
            ),
            Err(e) => self.set_error(QString::from(&format!("{field}: not valid JSON ({e})"))),
        }
    }

    fn set_workspace(self: Pin<&mut Self>, selector: &QString, layout: &QString) {
        let layout = layout.to_string();
        model_core::start(
            self,
            Op::SetWorkspace {
                selector: selector.to_string(),
                layout: (!layout.is_empty()).then_some(layout),
            },
        );
    }
}
