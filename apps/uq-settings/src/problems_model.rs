//! The Problems page's QML object: [`ProblemsState`] driven by `crate::model_core`.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::problems::{Op, ProblemsState};

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
        /// `{hours, crashes, problems, errors, other, notice}` (see `pages::problems::state`).
        #[qproperty(QString, state_json)]
        /// The message of the last failed request, "" when there is none.
        #[qproperty(QString, error)]
        #[qproperty(bool, busy)]
        type ProblemsModel = super::ProblemsModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut ProblemsModel>);

        /// Copy crash `id`'s report, or a report on now when `id` is "".
        #[qinvokable]
        fn copy_report(self: Pin<&mut ProblemsModel>, id: &QString);

        /// Open crash `id`'s report (or a new one on now, for "") in the text editor.
        #[qinvokable]
        fn open_report(self: Pin<&mut ProblemsModel>, id: &QString);

        /// Hide a crash from the page.
        #[qinvokable]
        fn dismiss(self: Pin<&mut ProblemsModel>, id: &QString);

        /// Look back this many hours.
        #[qinvokable]
        fn set_hours(self: Pin<&mut ProblemsModel>, hours: i32);
    }

    impl cxx_qt::Threading for ProblemsModel {}
    impl cxx_qt::Initialize for ProblemsModel {}
}

use qobject::ProblemsModel;

pub struct ProblemsModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<ProblemsState>,
}

impl Default for ProblemsModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(ProblemsState::new),
        }
    }
}

impl PageModel for ProblemsModel {
    type Logic = ProblemsState;
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

    fn core(&self) -> &ModelCore<ProblemsState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<ProblemsState> {
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

impl cxx_qt::Initialize for ProblemsModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl ProblemsModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn copy_report(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::Copy { id: id.to_string() });
    }

    fn open_report(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::Open { id: id.to_string() });
    }

    fn dismiss(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::Dismiss { id: id.to_string() });
    }

    fn set_hours(self: Pin<&mut Self>, hours: i32) {
        model_core::start(
            self,
            Op::SetHours {
                hours: hours.max(1) as u32,
            },
        );
    }
}
