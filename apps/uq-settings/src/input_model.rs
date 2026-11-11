//! The Input page's QML object: [`InputState`] driven by `crate::model_core`.
//!
//! This file sits in `src/` because cxx-qt-build supports one directory of bridge files per QML
//! module (QTBUG-93443), and `app.rs` and `window.rs` are there.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde_json::Value;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::input::{InputState, Op};

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
        /// `{input, devices, xkb}`; `{}` until uqd has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused change, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        type InputModel = super::InputModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut InputModel>);

        /// One keyboard field (`layouts` takes the whole list).
        #[qinvokable]
        fn set_keyboard(self: Pin<&mut InputModel>, field: &QString, json_value: &QString);

        #[qinvokable]
        fn set_mouse(self: Pin<&mut InputModel>, field: &QString, json_value: &QString);

        #[qinvokable]
        fn set_touchpad(self: Pin<&mut InputModel>, field: &QString, json_value: &QString);

        /// One field of a device's own setting; `null` removes it.
        #[qinvokable]
        fn set_device(
            self: Pin<&mut InputModel>,
            name: &QString,
            field: &QString,
            json_value: &QString,
        );

        #[qinvokable]
        fn remove_device(self: Pin<&mut InputModel>, name: &QString);
    }

    impl cxx_qt::Threading for InputModel {}
    impl cxx_qt::Initialize for InputModel {}
}

use qobject::InputModel;

pub struct InputModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<InputState>,
}

impl Default for InputModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(InputState::new),
        }
    }
}

impl PageModel for InputModel {
    type Logic = InputState;
    // `uqctl mouse` writes `[mouse]` through SetSetting, which announces SettingsChanged.
    const FOLLOW: Follow = Follow {
        sections: &["input"],
        theme: false,
        settings: true,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
        themes: false,
    };

    fn core(&self) -> &ModelCore<InputState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<InputState> {
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

impl cxx_qt::Initialize for InputModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl InputModel {
    /// Parse `json_value`, then start the op `make` builds from it.
    fn with_value(
        self: Pin<&mut Self>,
        field: &QString,
        json_value: &QString,
        make: impl FnOnce(Value) -> Op,
    ) {
        match serde_json::from_str(&json_value.to_string()) {
            Ok(value) => model_core::start(self, make(value)),
            Err(e) => self.set_error(QString::from(&format!("{field}: not valid JSON ({e})"))),
        }
    }

    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_keyboard(self: Pin<&mut Self>, field: &QString, json_value: &QString) {
        let f = field.to_string();
        self.with_value(field, json_value, |value| Op::SetKeyboard {
            field: f,
            value,
        });
    }

    fn set_mouse(self: Pin<&mut Self>, field: &QString, json_value: &QString) {
        let f = field.to_string();
        self.with_value(field, json_value, |value| Op::SetMouse { field: f, value });
    }

    fn set_touchpad(self: Pin<&mut Self>, field: &QString, json_value: &QString) {
        let f = field.to_string();
        self.with_value(field, json_value, |value| Op::SetTouchpad {
            field: f,
            value,
        });
    }

    fn set_device(self: Pin<&mut Self>, name: &QString, field: &QString, json_value: &QString) {
        let (n, f) = (name.to_string(), field.to_string());
        self.with_value(field, json_value, |value| Op::SetDevice {
            name: n,
            field: f,
            value,
        });
    }

    fn remove_device(self: Pin<&mut Self>, name: &QString) {
        model_core::start(
            self,
            Op::RemoveDevice {
                name: name.to_string(),
            },
        );
    }
}
