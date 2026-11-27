//! Startup › Services' QML object: [`ServicesState`] driven by `crate::model_core`.
//!
//! This file sits in `src/` because cxx-qt-build supports one directory of bridge files per QML
//! module (QTBUG-93443).

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::services::{Op, ServicesState};

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
        /// `{services, hidden, show_all, log, error?}` (see `pages::services`).
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request is in flight.
        #[qproperty(bool, busy)]
        type ServicesModel = super::ServicesModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut ServicesModel>);

        /// List every service, not only the ones that can be turned on or off.
        #[qinvokable]
        fn set_show_all(self: Pin<&mut ServicesModel>, on: bool);

        /// Start `unit` at login, or not.
        #[qinvokable]
        fn set_enabled(self: Pin<&mut ServicesModel>, unit: &QString, on: bool);

        /// "start", "stop" or "restart" `unit` now.
        #[qinvokable]
        fn act(self: Pin<&mut ServicesModel>, unit: &QString, action: &QString);

        /// Show `unit`'s last log lines; "" closes the log.
        #[qinvokable]
        fn show_log(self: Pin<&mut ServicesModel>, unit: &QString);
    }

    impl cxx_qt::Threading for ServicesModel {}
    impl cxx_qt::Initialize for ServicesModel {}
}

use qobject::ServicesModel;

pub struct ServicesModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<ServicesState>,
}

impl Default for ServicesModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(ServicesState::new),
        }
    }
}

impl PageModel for ServicesModel {
    type Logic = ServicesState;
    const FOLLOW: Follow = Follow {
        sections: &[],
        theme: false,
        settings: false,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
        themes: false,
    };

    fn core(&self) -> &ModelCore<ServicesState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<ServicesState> {
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

impl cxx_qt::Initialize for ServicesModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl ServicesModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_show_all(self: Pin<&mut Self>, on: bool) {
        model_core::start(self, Op::SetShowAll { on });
    }

    fn set_enabled(self: Pin<&mut Self>, unit: &QString, on: bool) {
        let unit = unit.to_string();
        model_core::start(self, Op::SetEnabled { unit, on });
    }

    fn act(self: Pin<&mut Self>, unit: &QString, action: &QString) {
        let (unit, action) = (unit.to_string(), action.to_string());
        model_core::start(self, Op::Act { unit, action });
    }

    fn show_log(self: Pin<&mut Self>, unit: &QString) {
        let unit = unit.to_string();
        model_core::start(self, Op::ShowLog { unit });
    }
}
