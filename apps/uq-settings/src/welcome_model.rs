//! Welcome's QML object: [`WelcomeState`] driven by `crate::model_core`, plus the fingerprint
//! enrolment's progress, which arrives on its own as the user touches the reader.

use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::welcome::{Op, WelcomeState};
use crate::session;

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
        /// `{flow, status, marked, monitors, reader, enroll, apps, bindle, setup_problem,
        /// session_log, keybinds}` (see `pages::welcome`).
        #[qproperty(QString, state_json)]
        /// The fingerprint enrolment as it goes: `{phase, stage, stages, message}`.
        #[qproperty(QString, enroll_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request is in flight.
        #[qproperty(bool, busy)]
        type WelcomeModel = super::WelcomeModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut WelcomeModel>);

        /// Welcome is on show: work out its steps and mark it done. `full` shows every step.
        #[qinvokable]
        fn start(self: Pin<&mut WelcomeModel>, full: bool);

        #[qinvokable]
        fn next(self: Pin<&mut WelcomeModel>);

        #[qinvokable]
        fn back(self: Pin<&mut WelcomeModel>);

        #[qinvokable]
        fn skip(self: Pin<&mut WelcomeModel>);

        /// Start bindle's app picker.
        #[qinvokable]
        fn open_bindle(self: Pin<&mut WelcomeModel>);

        /// Enrol the right index finger with fprintd.
        #[qinvokable]
        fn enroll(self: Pin<&mut WelcomeModel>);

        #[qinvokable]
        fn cancel_enroll(self: Pin<&mut WelcomeModel>);
    }

    impl cxx_qt::Threading for WelcomeModel {}
    impl cxx_qt::Initialize for WelcomeModel {}
}

use qobject::WelcomeModel;

pub struct WelcomeModelRust {
    state_json: QString,
    enroll_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<WelcomeState>,
    enroll_watch: Option<tokio::task::JoinHandle<()>>,
}

impl Default for WelcomeModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            enroll_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(WelcomeState::new),
            enroll_watch: None,
        }
    }
}

impl Drop for WelcomeModelRust {
    fn drop(&mut self) {
        if let Some(task) = self.enroll_watch.take() {
            task.abort();
        }
        // A scan left running would keep the reader claimed.
        if let Some(logic) = self.core.logic() {
            logic.enrolment.cancel();
        }
    }
}

impl PageModel for WelcomeModel {
    type Logic = WelcomeState;
    const FOLLOW: Follow = Follow {
        sections: &[],
        theme: false,
        settings: false,
        history: false,
        monitors: true,
        profiles: false,
        autostart: false,
    };

    fn core(&self) -> &ModelCore<WelcomeState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<WelcomeState> {
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

impl cxx_qt::Initialize for WelcomeModel {
    fn initialize(mut self: Pin<&mut Self>) {
        // Follow the enrolment: each change is queued to the Qt thread as it happens.
        if let (Some(logic), Some((handle, _))) = (self.core().logic(), session::bus()) {
            let enrolment = logic.enrolment.clone();
            let qt = self.qt_thread();
            let task = handle.spawn(async move {
                loop {
                    // Registered before the state is read, so no change is missed.
                    let changed = enrolment.changed.notified();
                    let json = serde_json::to_string(&enrolment.get()).unwrap_or_default();
                    if qt
                        .queue(move |m| m.set_enroll_json(QString::from(&json)))
                        .is_err()
                    {
                        return;
                    }
                    changed.await;
                }
            });
            self.as_mut().rust_mut().get_mut().enroll_watch = Some(task);
        }
        model_core::initialize(self);
    }
}

impl WelcomeModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn start(self: Pin<&mut Self>, full: bool) {
        model_core::start(self, Op::Start { full });
    }

    fn next(self: Pin<&mut Self>) {
        model_core::start(self, Op::Next);
    }

    fn back(self: Pin<&mut Self>) {
        model_core::start(self, Op::Back);
    }

    fn skip(self: Pin<&mut Self>) {
        model_core::start(self, Op::Skip);
    }

    fn open_bindle(self: Pin<&mut Self>) {
        model_core::start(self, Op::OpenBindle);
    }

    fn enroll(self: Pin<&mut Self>) {
        model_core::start(self, Op::Enroll);
    }

    fn cancel_enroll(self: Pin<&mut Self>) {
        model_core::start(self, Op::CancelEnroll);
    }
}
