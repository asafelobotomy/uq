//! The Power page's QML object: [`PowerState`] driven by `crate::model_core`.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::power::{Op, PowerState};

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
        /// `{profiles, power, actions, notice}` (see `pages::power`).
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request is in flight (a password prompt may be up).
        #[qproperty(bool, busy)]
        type PowerModel = super::PowerModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut PowerModel>);

        /// Switch the power mode.
        #[qinvokable]
        fn set_profile(self: Pin<&mut PowerModel>, name: &QString);

        /// Save the lid, lid-on-mains and power-key actions together.
        #[qinvokable]
        fn set_actions(
            self: Pin<&mut PowerModel>,
            lid: &QString,
            lid_on_ac: &QString,
            power_key: &QString,
        );
    }

    impl cxx_qt::Threading for PowerModel {}
    impl cxx_qt::Initialize for PowerModel {}
}

use qobject::PowerModel;

pub struct PowerModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<PowerState>,
}

impl Default for PowerModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(PowerState::new),
        }
    }
}

impl PageModel for PowerModel {
    type Logic = PowerState;
    const FOLLOW: Follow = Follow {
        sections: &[],
        theme: false,
        settings: false,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
    };

    fn core(&self) -> &ModelCore<PowerState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<PowerState> {
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

impl cxx_qt::Initialize for PowerModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl PowerModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_profile(self: Pin<&mut Self>, name: &QString) {
        model_core::start(
            self,
            Op::SetProfile {
                name: name.to_string(),
            },
        );
    }

    fn set_actions(self: Pin<&mut Self>, lid: &QString, lid_on_ac: &QString, power_key: &QString) {
        model_core::start(
            self,
            Op::SetActions {
                lid: lid.to_string(),
                lid_on_ac: lid_on_ac.to_string(),
                power_key: power_key.to_string(),
            },
        );
    }
}
