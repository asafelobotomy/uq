//! The Default apps page's QML object: [`DefaultAppsState`] driven by `crate::model_core`.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::defaultapps::{DefaultAppsState, Op};

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
        /// `{kinds, terminal}` (see `pages::defaultapps`).
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request is in flight (a password prompt may be up).
        #[qproperty(bool, busy)]
        type DefaultAppsModel = super::DefaultAppsModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut DefaultAppsModel>);

        /// Open kind `kind` (browser, mail, files, text, images, video, music, pdf) with the
        /// app `app` (a desktop id).
        #[qinvokable]
        fn set_default(self: Pin<&mut DefaultAppsModel>, kind: &QString, app: &QString);

        /// The terminal for apps that run in one; "" is uQ's default.
        #[qinvokable]
        fn set_terminal(self: Pin<&mut DefaultAppsModel>, app: &QString);
    }

    impl cxx_qt::Threading for DefaultAppsModel {}
    impl cxx_qt::Initialize for DefaultAppsModel {}
}

use qobject::DefaultAppsModel;

pub struct DefaultAppsModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<DefaultAppsState>,
}

impl Default for DefaultAppsModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(DefaultAppsState::new),
        }
    }
}

impl PageModel for DefaultAppsModel {
    type Logic = DefaultAppsState;
    const FOLLOW: Follow = Follow {
        // The terminal is uQ's `shell` section.
        sections: &["shell"],
        theme: false,
        settings: false,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
        themes: false,
    };

    fn core(&self) -> &ModelCore<DefaultAppsState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<DefaultAppsState> {
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

impl cxx_qt::Initialize for DefaultAppsModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl DefaultAppsModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_default(self: Pin<&mut Self>, kind: &QString, app: &QString) {
        model_core::start(
            self,
            Op::SetDefault {
                kind: kind.to_string(),
                app: app.to_string(),
            },
        );
    }

    fn set_terminal(self: Pin<&mut Self>, app: &QString) {
        let app = app.to_string();
        model_core::start(
            self,
            Op::SetTerminal {
                app: (!app.is_empty()).then_some(app),
            },
        );
    }
}
