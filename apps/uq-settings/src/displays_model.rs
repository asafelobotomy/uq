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
        /// `{live, stored, pending, draft, dirty, view, main, bar, auto_switch, plan,
        /// arrangements, offer, offer_name, arrangement_failed}`; `{}` until uqd has been read.
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

        /// Choose the main display by its connector name; "" chooses none.
        #[qinvokable]
        fn set_main(self: Pin<&mut DisplaysModel>, output: &QString);

        /// Where the bar shows: `show` is `all`, `main` or `chosen`; `outputs_json` is a JSON
        /// list of connector names (used for `chosen`).
        #[qinvokable]
        fn set_bar(self: Pin<&mut DisplaysModel>, show: &QString, outputs_json: &QString);

        /// Switch to a saved arrangement when the screens change.
        #[qinvokable]
        fn set_auto_switch(self: Pin<&mut DisplaysModel>, on: bool);

        /// How the workspaces spread over the screens: `off`, `sequential`, `interleave` or
        /// `manual`.
        #[qinvokable]
        fn set_plan(self: Pin<&mut DisplaysModel>, strategy: &QString, count: i32, move_back: bool);

        /// Put `workspace` on the display `output`; "" takes it back.
        #[qinvokable]
        fn set_manual(self: Pin<&mut DisplaysModel>, workspace: i32, output: &QString);

        /// Save the screens connected now, with their layout, under `name`.
        #[qinvokable]
        fn remember(self: Pin<&mut DisplaysModel>, name: &QString);

        #[qinvokable]
        fn rename_arrangement(self: Pin<&mut DisplaysModel>, id: &QString, name: &QString);

        #[qinvokable]
        fn forget_arrangement(self: Pin<&mut DisplaysModel>, id: &QString);
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
        profiles: false,
        autostart: false,
        themes: false,
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

    fn set_main(self: Pin<&mut Self>, output: &QString) {
        let output = output.to_string();
        model_core::start(
            self,
            Op::SetMain {
                output: (!output.is_empty()).then_some(output),
            },
        );
    }

    fn set_bar(self: Pin<&mut Self>, show: &QString, outputs_json: &QString) {
        match serde_json::from_str::<Vec<String>>(&outputs_json.to_string()) {
            Ok(outputs) => model_core::start(
                self,
                Op::SetBar {
                    show: show.to_string(),
                    outputs,
                },
            ),
            Err(e) => self.set_error(QString::from(&format!(
                "the displays: not valid JSON ({e})"
            ))),
        }
    }

    fn set_auto_switch(self: Pin<&mut Self>, on: bool) {
        model_core::start(self, Op::SetAutoSwitch(on));
    }

    fn set_plan(self: Pin<&mut Self>, strategy: &QString, count: i32, move_back: bool) {
        model_core::start(
            self,
            Op::SetPlan {
                strategy: strategy.to_string(),
                count: count.clamp(0, i32::from(u8::MAX)) as u8,
                move_back,
            },
        );
    }

    fn set_manual(self: Pin<&mut Self>, workspace: i32, output: &QString) {
        let output = output.to_string();
        model_core::start(
            self,
            Op::SetManual {
                workspace: workspace.clamp(0, i32::from(u8::MAX)) as u8,
                output: (!output.is_empty()).then_some(output),
            },
        );
    }

    fn remember(self: Pin<&mut Self>, name: &QString) {
        model_core::start(
            self,
            Op::Remember {
                name: name.to_string(),
            },
        );
    }

    fn rename_arrangement(self: Pin<&mut Self>, id: &QString, name: &QString) {
        model_core::start(
            self,
            Op::Rename {
                id: id.to_string(),
                name: name.to_string(),
            },
        );
    }

    fn forget_arrangement(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::Forget { id: id.to_string() });
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
