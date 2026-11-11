//! The About page's QML object: [`AboutState`] driven by `crate::model_core`.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::about::{AboutState, Op};

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
        /// `{hostname, os, kernel, cpu, memory, gpus, versions, notice}` (see `pages::about`).
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request is in flight (a password prompt may be up).
        #[qproperty(bool, busy)]
        type AboutModel = super::AboutModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut AboutModel>);

        /// Change the computer's name (asks for an administrator password).
        #[qinvokable]
        fn set_hostname(self: Pin<&mut AboutModel>, name: &QString);
    }

    impl cxx_qt::Threading for AboutModel {}
    impl cxx_qt::Initialize for AboutModel {}
}

use qobject::AboutModel;

pub struct AboutModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<AboutState>,
}

impl Default for AboutModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(AboutState::new),
        }
    }
}

impl PageModel for AboutModel {
    type Logic = AboutState;
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

    fn core(&self) -> &ModelCore<AboutState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<AboutState> {
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

impl cxx_qt::Initialize for AboutModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl AboutModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_hostname(self: Pin<&mut Self>, name: &QString) {
        model_core::start(
            self,
            Op::SetHostname {
                name: name.to_string(),
            },
        );
    }
}
