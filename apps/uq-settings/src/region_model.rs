//! The Region page's QML object: [`RegionState`] driven by `crate::model_core`.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::region::{Op, RegionState};

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
        /// `{localed, locales, keymaps, notice}` (see `pages::region`).
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request is in flight (a password prompt may be up).
        #[qproperty(bool, busy)]
        type RegionModel = super::RegionModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut RegionModel>);

        /// The system language (`LANG`).
        #[qinvokable]
        fn set_language(self: Pin<&mut RegionModel>, locale: &QString);

        /// How dates, numbers, money, measurements and paper look.
        #[qinvokable]
        fn set_formats(self: Pin<&mut RegionModel>, locale: &QString);

        /// The text console's keyboard layout.
        #[qinvokable]
        fn set_keymap(self: Pin<&mut RegionModel>, keymap: &QString);
    }

    impl cxx_qt::Threading for RegionModel {}
    impl cxx_qt::Initialize for RegionModel {}
}

use qobject::RegionModel;

pub struct RegionModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<RegionState>,
}

impl Default for RegionModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(RegionState::new),
        }
    }
}

impl PageModel for RegionModel {
    type Logic = RegionState;
    const FOLLOW: Follow = Follow {
        sections: &[],
        theme: false,
        settings: false,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
    };

    fn core(&self) -> &ModelCore<RegionState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<RegionState> {
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

impl cxx_qt::Initialize for RegionModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl RegionModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_language(self: Pin<&mut Self>, locale: &QString) {
        model_core::start(
            self,
            Op::SetLanguage {
                locale: locale.to_string(),
            },
        );
    }

    fn set_formats(self: Pin<&mut Self>, locale: &QString) {
        model_core::start(
            self,
            Op::SetFormats {
                locale: locale.to_string(),
            },
        );
    }

    fn set_keymap(self: Pin<&mut Self>, keymap: &QString) {
        model_core::start(
            self,
            Op::SetKeymap {
                keymap: keymap.to_string(),
            },
        );
    }
}
