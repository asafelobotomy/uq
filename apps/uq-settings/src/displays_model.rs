//! The Displays page's QML object: [`DisplaysState`] driven by `crate::model_core`.
//!
//! This file sits in `src/` because cxx-qt-build supports one directory of bridge files per QML
//! module (QTBUG-93443), and `app.rs` and `window.rs` are there.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::displays::{DisplaysState, Op};

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
        /// `{live, stored, pending, draft, dirty, view}`; `{}` until uqd has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused change, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        /// Seconds until uqd reverts the change being tried; -1 when nothing is being tried.
        #[qproperty(i32, pending_seconds)]
        type DisplaysModel = super::DisplaysModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut DisplaysModel>);

        /// Change one field of `output` in the draft (`json_value` `null` = the default).
        #[qinvokable]
        fn edit(
            self: Pin<&mut DisplaysModel>,
            output: &QString,
            field: &QString,
            json_value: &QString,
        );

        /// Drop `output` at `(x, y)` on the arrangement, in logical pixels.
        #[qinvokable]
        fn move_output(self: Pin<&mut DisplaysModel>, output: &QString, x: i32, y: i32);

        #[qinvokable]
        fn discard(self: Pin<&mut DisplaysModel>);

        /// Try the draft; it is put back unless kept.
        #[qinvokable]
        fn apply(self: Pin<&mut DisplaysModel>);

        #[qinvokable]
        fn keep(self: Pin<&mut DisplaysModel>);

        #[qinvokable]
        fn revert(self: Pin<&mut DisplaysModel>);
    }

    impl cxx_qt::Threading for DisplaysModel {}
    impl cxx_qt::Initialize for DisplaysModel {}
}

use qobject::DisplaysModel;

pub struct DisplaysModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    pending_seconds: i32,
    core: ModelCore<DisplaysState>,
}

impl Default for DisplaysModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            pending_seconds: -1,
            core: ModelCore::new(DisplaysState::new),
        }
    }
}

/// `pending.seconds_left` of a snapshot, or -1.
fn pending_seconds(json: &str) -> i32 {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v["pending"]["seconds_left"].as_i64())
        .map_or(-1, |s| s.clamp(0, 600) as i32)
}

impl PageModel for DisplaysModel {
    type Logic = DisplaysState;
    const FOLLOW: Follow = Follow {
        sections: &["monitors"],
        theme: false,
        settings: false,
        history: false,
        monitors: true,
    };

    fn core(&self) -> &ModelCore<DisplaysState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<DisplaysState> {
        &mut self.rust_mut().get_mut().core
    }
    fn show_busy(self: Pin<&mut Self>, busy: bool) {
        self.set_busy(busy);
    }
    fn show_error(self: Pin<&mut Self>, e: QString) {
        self.set_error(e);
    }
    fn show_state(mut self: Pin<&mut Self>, json: QString) {
        let seconds = pending_seconds(&json.to_string());
        self.as_mut().set_pending_seconds(seconds);
        self.set_state_json(json);
    }
}

impl cxx_qt::Initialize for DisplaysModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl DisplaysModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn edit(self: Pin<&mut Self>, output: &QString, field: &QString, json_value: &QString) {
        match serde_json::from_str(&json_value.to_string()) {
            Ok(value) => model_core::start(
                self,
                Op::Edit {
                    output: output.to_string(),
                    field: field.to_string(),
                    value,
                },
            ),
            Err(e) => self.set_error(QString::from(&format!("{field}: not valid JSON ({e})"))),
        }
    }

    fn move_output(self: Pin<&mut Self>, output: &QString, x: i32, y: i32) {
        model_core::start(
            self,
            Op::Move {
                output: output.to_string(),
                x,
                y,
            },
        );
    }

    fn discard(self: Pin<&mut Self>) {
        model_core::start(self, Op::Discard);
    }

    fn apply(self: Pin<&mut Self>) {
        model_core::start(self, Op::Apply);
    }

    fn keep(self: Pin<&mut Self>) {
        model_core::start(self, Op::Keep);
    }

    fn revert(self: Pin<&mut Self>) {
        model_core::start(self, Op::Revert);
    }
}

#[cfg(test)]
mod tests {
    use super::pending_seconds;

    #[test]
    fn pending_seconds_from_the_snapshot() {
        assert_eq!(
            pending_seconds(r#"{"pending":{"token":"t","seconds_left":12}}"#),
            12
        );
        assert_eq!(pending_seconds(r#"{"pending":null}"#), -1);
        assert_eq!(pending_seconds("{}"), -1);
        assert_eq!(pending_seconds("not json"), -1);
    }
}
