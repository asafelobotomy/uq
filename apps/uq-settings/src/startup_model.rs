//! The Startup page's QML object: [`StartupState`] driven by `crate::model_core`.
//!
//! Like `rules_model.rs`, this file sits in `src/` because cxx-qt-build supports one directory
//! of bridge files per QML module (QTBUG-93443).

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::startup::{Op, StartupState};

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
        /// `{apps, apps_error, vars, switches, section, installed, limits, notice}` (see
        /// `pages::startup`); `{}` until uqd has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        type StartupModel = super::StartupModelRust;

        /// Read the startup apps and the variables again.
        #[qinvokable]
        fn refresh(self: Pin<&mut StartupModel>);

        /// Turn a startup app on or off for the next login.
        #[qinvokable]
        fn set_enabled(self: Pin<&mut StartupModel>, id: &QString, on: bool);

        /// Set how many seconds after login an app starts; -1 clears the choice.
        #[qinvokable]
        fn set_delay(self: Pin<&mut StartupModel>, id: &QString, secs: i32);

        /// Start an installed app (by its desktop id) at login.
        #[qinvokable]
        fn add_app(self: Pin<&mut StartupModel>, id: &QString);

        /// Run a typed command at login; an empty `name` names it after its program.
        #[qinvokable]
        fn add_command(self: Pin<&mut StartupModel>, name: &QString, command: &QString);

        /// Remove an app added here.
        #[qinvokable]
        fn remove(self: Pin<&mut StartupModel>, id: &QString);

        /// Replace the variable at `index`, or add one at the end with -1.
        #[qinvokable]
        fn set_var(self: Pin<&mut StartupModel>, index: i32, name: &QString, value: &QString);

        /// Remove the variable called `name`.
        #[qinvokable]
        fn remove_var(self: Pin<&mut StartupModel>, name: &QString);

        /// Move the variable called `name` to position `to` of the list.
        #[qinvokable]
        fn move_var(self: Pin<&mut StartupModel>, name: &QString, to: i32);
    }

    impl cxx_qt::Threading for StartupModel {}
    impl cxx_qt::Initialize for StartupModel {}
}

use qobject::StartupModel;

pub struct StartupModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<StartupState>,
}

impl Default for StartupModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(StartupState::new),
        }
    }
}

impl PageModel for StartupModel {
    type Logic = StartupState;
    // The switches are shown read-only from `compat`; history covers undo and restore. A
    // startup app's status follows `AutostartChanged`. Coming back to the window re-reads
    // through the page's `refresh()`.
    const FOLLOW: Follow = Follow {
        sections: &["env", "autostart", "compat"],
        theme: false,
        settings: false,
        history: true,
        monitors: false,
        profiles: false,
        autostart: true,
    };

    fn core(&self) -> &ModelCore<StartupState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<StartupState> {
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

impl cxx_qt::Initialize for StartupModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl StartupModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_enabled(self: Pin<&mut Self>, id: &QString, on: bool) {
        model_core::start(
            self,
            Op::SetEnabled {
                id: id.to_string(),
                on,
            },
        );
    }

    fn set_delay(self: Pin<&mut Self>, id: &QString, secs: i32) {
        model_core::start(
            self,
            Op::SetDelay {
                id: id.to_string(),
                delay: u32::try_from(secs).ok(),
            },
        );
    }

    fn add_app(self: Pin<&mut Self>, id: &QString) {
        model_core::start(
            self,
            Op::AddApp {
                desktop_id: id.to_string(),
            },
        );
    }

    fn add_command(self: Pin<&mut Self>, name: &QString, command: &QString) {
        model_core::start(
            self,
            Op::AddCommand {
                name: name.to_string(),
                command: command.to_string(),
            },
        );
    }

    fn remove(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::Remove { id: id.to_string() });
    }

    fn set_var(self: Pin<&mut Self>, index: i32, name: &QString, value: &QString) {
        model_core::start(
            self,
            Op::SetVar {
                index: usize::try_from(index).ok(),
                name: name.to_string(),
                value: value.to_string(),
            },
        );
    }

    fn remove_var(self: Pin<&mut Self>, name: &QString) {
        model_core::start(
            self,
            Op::RemoveVar {
                name: name.to_string(),
            },
        );
    }

    fn move_var(self: Pin<&mut Self>, name: &QString, to: i32) {
        model_core::start(
            self,
            Op::MoveVar {
                name: name.to_string(),
                to: usize::try_from(to).unwrap_or(0),
            },
        );
    }
}
