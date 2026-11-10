//! The Shell page's QML object: [`ShellState`] driven by `crate::model_core`.
//!
//! This file sits in `src/` because cxx-qt-build supports one directory of bridge files per QML
//! module (QTBUG-93443), and `app.rs` and `window.rs` are there.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::shell::{Op, ShellState};

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
        /// `{settings, shell, providers, apps, terminals}`; `{}` until uqd has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused change, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        type ShellModel = super::ShellModelRust;

        /// Read everything from uqd again.
        #[qinvokable]
        fn refresh(self: Pin<&mut ShellModel>);

        /// Set one Phase 2 setting (`pinned`, `idle.*`, `notif.*`, `privacy.*`).
        #[qinvokable]
        fn set_setting(self: Pin<&mut ShellModel>, key: &QString, json_value: &QString);

        /// Set one field of the `shell` section; `islands.<id>` sets one island.
        #[qinvokable]
        fn set_shell(self: Pin<&mut ShellModel>, field: &QString, json_value: &QString);

        /// Forget which apps were launched how often.
        #[qinvokable]
        fn clear_launch_counts(self: Pin<&mut ShellModel>);

        /// Unpin `id`.
        #[qinvokable]
        fn pin_remove(self: Pin<&mut ShellModel>, id: &QString);

        /// Move pin `id` by `to - from` places from where it is now.
        #[qinvokable]
        fn pin_move(self: Pin<&mut ShellModel>, id: &QString, from: i32, to: i32);

        /// Move provider `id` by `to - from` places from where it is now.
        #[qinvokable]
        fn provider_move(self: Pin<&mut ShellModel>, id: &QString, from: i32, to: i32);

        /// Turn provider `id` on or off.
        #[qinvokable]
        fn provider_toggle(self: Pin<&mut ShellModel>, id: &QString, on: bool);
    }

    impl cxx_qt::Threading for ShellModel {}
    impl cxx_qt::Initialize for ShellModel {}
}

use qobject::ShellModel;

pub struct ShellModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<ShellState>,
}

impl Default for ShellModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(ShellState::new),
        }
    }
}

impl PageModel for ShellModel {
    type Logic = ShellState;
    const FOLLOW: Follow = Follow {
        sections: &["shell"],
        theme: false,
        settings: true,
        history: false,
    };

    fn core(&self) -> &ModelCore<ShellState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<ShellState> {
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

impl cxx_qt::Initialize for ShellModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl ShellModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_setting(self: Pin<&mut Self>, key: &QString, json_value: &QString) {
        match serde_json::from_str(&json_value.to_string()) {
            Ok(value) => model_core::start(
                self,
                Op::SetSetting {
                    key: key.to_string(),
                    value,
                },
            ),
            Err(e) => self.set_error(QString::from(&format!("{key}: not valid JSON ({e})"))),
        }
    }

    fn set_shell(self: Pin<&mut Self>, field: &QString, json_value: &QString) {
        match serde_json::from_str(&json_value.to_string()) {
            Ok(value) => model_core::start(
                self,
                Op::SetShell {
                    field: field.to_string(),
                    value,
                },
            ),
            Err(e) => self.set_error(QString::from(&format!("{field}: not valid JSON ({e})"))),
        }
    }

    fn clear_launch_counts(self: Pin<&mut Self>) {
        model_core::start(self, Op::ClearLaunchCounts);
    }

    fn pin_remove(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::PinRemove { id: id.to_string() });
    }

    fn pin_move(self: Pin<&mut Self>, id: &QString, from: i32, to: i32) {
        let (id, from, to) = (id.to_string(), index(from), index(to));
        model_core::start(self, Op::PinMove { id, from, to });
    }

    fn provider_move(self: Pin<&mut Self>, id: &QString, from: i32, to: i32) {
        let (id, from, to) = (id.to_string(), index(from), index(to));
        model_core::start(self, Op::ProviderMove { id, from, to });
    }

    fn provider_toggle(self: Pin<&mut Self>, id: &QString, on: bool) {
        let id = id.to_string();
        model_core::start(self, Op::ProviderToggle { id, on });
    }
}

/// A QML row index; a negative one is 0.
fn index(i: i32) -> usize {
    usize::try_from(i).unwrap_or(0)
}
