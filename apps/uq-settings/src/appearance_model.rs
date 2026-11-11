//! The Appearance page's QML object: [`AppearanceState`] driven by `crate::model_core`.
//!
//! This file sits in `src/` because cxx-qt-build supports one directory of bridge files per QML
//! module (QTBUG-93443), and `app.rs` and `window.rs` are there.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::appearance::{AppearanceState, Op};
use crate::system_lists;

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
        /// `{themes, theme, variant, schedule, appearance, cursorThemes, iconThemes, sources}`;
        /// `{}` until uqd has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused change, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        type AppearanceModel = super::AppearanceModelRust;

        /// Read everything from uqd again.
        #[qinvokable]
        fn refresh(self: Pin<&mut AppearanceModel>);

        #[qinvokable]
        fn set_theme(self: Pin<&mut AppearanceModel>, id: &QString, variant: &QString);

        /// `json`: `{"mode":"off"}`, `{"mode":"fixed",...}` or `{"mode":"sun",...}`.
        #[qinvokable]
        fn set_schedule(self: Pin<&mut AppearanceModel>, json: &QString);

        /// Set one field of the `appearance` section to `json_value` (`null` = use the theme).
        #[qinvokable]
        fn set_appearance(self: Pin<&mut AppearanceModel>, field: &QString, json_value: &QString);

        /// Apply `appearance` to the session again, over what changed it while running.
        #[qinvokable]
        fn reapply(self: Pin<&mut AppearanceModel>, domain: &QString);
    }

    impl cxx_qt::Threading for AppearanceModel {}
    impl cxx_qt::Initialize for AppearanceModel {}
}

use qobject::AppearanceModel;

pub struct AppearanceModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<AppearanceState>,
}

impl Default for AppearanceModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(|conn| AppearanceState::new(conn, system_lists::real_dirs())),
        }
    }
}

impl PageModel for AppearanceModel {
    type Logic = AppearanceState;
    const FOLLOW: Follow = Follow {
        sections: &["appearance"],
        theme: true,
        settings: true,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
        themes: true,
    };

    fn core(&self) -> &ModelCore<AppearanceState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<AppearanceState> {
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

impl cxx_qt::Initialize for AppearanceModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl AppearanceModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn reapply(self: Pin<&mut Self>, domain: &QString) {
        model_core::reapply(self, domain.to_string());
    }

    fn set_theme(self: Pin<&mut Self>, id: &QString, variant: &QString) {
        model_core::start(
            self,
            Op::SetTheme {
                id: id.to_string(),
                variant: variant.to_string(),
            },
        );
    }

    fn set_schedule(self: Pin<&mut Self>, json: &QString) {
        model_core::start(self, Op::SetSchedule(json.to_string()));
    }

    fn set_appearance(self: Pin<&mut Self>, field: &QString, json_value: &QString) {
        match serde_json::from_str(&json_value.to_string()) {
            Ok(value) => model_core::start(
                self,
                Op::SetField {
                    field: field.to_string(),
                    value,
                },
            ),
            Err(e) => self.set_error(QString::from(&format!("{field}: not valid JSON ({e})"))),
        }
    }
}
