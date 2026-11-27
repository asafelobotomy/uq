//! The Restore page's QML object: [`RestoreState`] driven by `crate::model_core`. A recovery
//! run's output reaches the page through the logic's `watch`, which reads the state again as
//! each line comes.
//!
//! This file sits in `src/` because cxx-qt-build supports one directory of bridge files per QML
//! module (QTBUG-93443), and `app.rs` and `window.rs` are there.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::restore::{Op, RestoreState};

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
        /// `{bindle, snapshots, snapper, needsPassword, snapshotsError, run, notice}` (see
        /// `pages::restore`).
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request is in flight.
        #[qproperty(bool, busy)]
        type RestoreModel = super::RestoreModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut RestoreModel>);

        /// Ask Bindle again what this computer allows.
        #[qinvokable]
        fn probe(self: Pin<&mut RestoreModel>);

        /// Read the snapshots, asking for the password if snapper needs it.
        #[qinvokable]
        fn load_snapshots(self: Pin<&mut RestoreModel>);

        #[qinvokable]
        fn create_snapshot(self: Pin<&mut RestoreModel>, description: &QString);

        #[qinvokable]
        fn delete_snapshot(self: Pin<&mut RestoreModel>, number: i32);

        /// Change one automatic-snapshot setting (Bindle checks the key and value).
        #[qinvokable]
        fn set_snapper(self: Pin<&mut RestoreModel>, key: &QString, value: &QString);

        /// Start rollback (to `number`), rebase, restore, keep-on, keep-off or cancel.
        #[qinvokable]
        fn recover(self: Pin<&mut RestoreModel>, kind: &QString, number: i32);

        #[qinvokable]
        fn reboot(self: Pin<&mut RestoreModel>);

        /// Every uQ setting back to its default (a restore point first).
        #[qinvokable]
        fn reset_all(self: Pin<&mut RestoreModel>);

        /// Save the settings to a dialog's `file://` URL.
        #[qinvokable]
        fn save_backup(self: Pin<&mut RestoreModel>, url: &QString);

        /// Bring back the settings in a dialog's `file://` URL (a restore point first).
        #[qinvokable]
        fn restore_backup(self: Pin<&mut RestoreModel>, url: &QString);
    }

    impl cxx_qt::Threading for RestoreModel {}
    impl cxx_qt::Initialize for RestoreModel {}
}

use qobject::RestoreModel;

pub struct RestoreModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<RestoreState>,
}

impl Default for RestoreModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(RestoreState::new),
        }
    }
}

impl PageModel for RestoreModel {
    type Logic = RestoreState;
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

    fn core(&self) -> &ModelCore<RestoreState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<RestoreState> {
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

impl cxx_qt::Initialize for RestoreModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl RestoreModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn probe(self: Pin<&mut Self>) {
        model_core::start(self, Op::Probe);
    }

    fn load_snapshots(self: Pin<&mut Self>) {
        model_core::start(self, Op::LoadSnapshots);
    }

    fn create_snapshot(self: Pin<&mut Self>, description: &QString) {
        let description = description.to_string();
        model_core::start(self, Op::CreateSnapshot { description });
    }

    fn delete_snapshot(self: Pin<&mut Self>, number: i32) {
        let number = number.max(0) as u32;
        model_core::start(self, Op::DeleteSnapshot { number });
    }

    fn set_snapper(self: Pin<&mut Self>, key: &QString, value: &QString) {
        let (key, value) = (key.to_string(), value.to_string());
        model_core::start(self, Op::SetSnapper { key, value });
    }

    fn recover(self: Pin<&mut Self>, kind: &QString, number: i32) {
        let kind = kind.to_string();
        let number = number.max(0) as u32;
        model_core::start(self, Op::Recover { kind, number });
    }

    fn reboot(self: Pin<&mut Self>) {
        model_core::start(self, Op::Reboot);
    }

    fn reset_all(self: Pin<&mut Self>) {
        model_core::start(self, Op::ResetAll);
    }

    fn save_backup(self: Pin<&mut Self>, url: &QString) {
        model_core::start(
            self,
            Op::SaveBackup {
                url: url.to_string(),
            },
        );
    }

    fn restore_backup(self: Pin<&mut Self>, url: &QString) {
        model_core::start(
            self,
            Op::RestoreBackup {
                url: url.to_string(),
            },
        );
    }
}
