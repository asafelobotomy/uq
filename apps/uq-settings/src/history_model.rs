//! The settings history's QML object: [`HistoryState`] driven by `crate::model_core`. A singleton,
//! because the window's undo and redo, its "Changed this session" sheet and the History page share
//! one undo stack (it lives in [`HistoryState`], so its rules are tested without Qt).
//!
//! This file sits in `src/` because cxx-qt-build supports one directory of bridge files per QML
//! module (QTBUG-93443), and `app.rs` and `window.rs` are there.

use std::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::history::{HistoryState, Op};

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
        #[qml_singleton]
        /// `{entries, points, session_start, canUndo, canRedo, notice, conflict}` (see
        /// `pages::history`); `{}` until uqd has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        type HistoryModel = super::HistoryModelRust;

        /// Read the journal and the restore points again.
        #[qinvokable]
        fn refresh(self: Pin<&mut HistoryModel>);

        /// Undo this app's latest change of this session (Ctrl+Z).
        #[qinvokable]
        fn undo_last(self: Pin<&mut HistoryModel>);

        /// Undo the latest undo (Ctrl+Shift+Z).
        #[qinvokable]
        fn redo_last(self: Pin<&mut HistoryModel>);

        /// Undo one history entry; `force` undoes it even though its domain changed again since.
        #[qinvokable]
        fn undo(self: Pin<&mut HistoryModel>, id: &QString, force: bool);

        /// Save the current settings as a restore point called `name`.
        #[qinvokable]
        fn save_restore_point(self: Pin<&mut HistoryModel>, name: &QString);

        /// Delete a restore point the user saved.
        #[qinvokable]
        fn delete_restore_point(self: Pin<&mut HistoryModel>, id: &QString);

        /// Bring back every setting of a restore point.
        #[qinvokable]
        fn restore(self: Pin<&mut HistoryModel>, id: &QString);
    }

    impl cxx_qt::Threading for HistoryModel {}
    impl cxx_qt::Initialize for HistoryModel {}
}

use qobject::HistoryModel;

pub struct HistoryModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<HistoryState>,
}

impl Default for HistoryModelRust {
    fn default() -> Self {
        // The singleton is made when the window first uses it, as the app starts.
        let session_start = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(|conn| HistoryState::new(conn, session_start)),
        }
    }
}

impl PageModel for HistoryModel {
    type Logic = HistoryState;
    const FOLLOW: Follow = Follow {
        sections: &[],
        theme: false,
        settings: false,
        history: true,
        monitors: false,
        profiles: true,
        autostart: false,
        themes: false,
    };

    fn core(&self) -> &ModelCore<HistoryState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<HistoryState> {
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

impl cxx_qt::Initialize for HistoryModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl HistoryModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn undo_last(self: Pin<&mut Self>) {
        model_core::start(self, Op::UndoLast);
    }

    fn redo_last(self: Pin<&mut Self>) {
        model_core::start(self, Op::RedoLast);
    }

    fn undo(self: Pin<&mut Self>, id: &QString, force: bool) {
        model_core::start(
            self,
            Op::Undo {
                id: id.to_string(),
                force,
            },
        );
    }

    fn save_restore_point(self: Pin<&mut Self>, name: &QString) {
        model_core::start(
            self,
            Op::SaveRestorePoint {
                name: name.to_string(),
            },
        );
    }

    fn delete_restore_point(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::DeleteRestorePoint { id: id.to_string() });
    }

    fn restore(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::Restore { id: id.to_string() });
    }
}
