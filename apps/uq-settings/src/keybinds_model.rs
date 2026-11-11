//! The Keyboard shortcuts page's QML object: [`KeybindsState`] driven by `crate::model_core`.
//!
//! Like `rules_model.rs`, this file sits in `src/` because cxx-qt-build supports one directory of
//! bridge files per QML module (QTBUG-93443).

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde_json::{Value, json};

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::keybinds::{self, KeybindsState, Op};

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
        /// `{groups, custom, user_lua, section, capture, catalogue, apps, limits, notice}` (see
        /// `pages::keybinds`); `{}` until uqd has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        type KeybindsModel = super::KeybindsModelRust;

        /// Read the shortcuts again.
        #[qinvokable]
        fn refresh(self: Pin<&mut KeybindsModel>);

        /// New keys for the shortcut `id` (one of uQ's, or one of your own).
        #[qinvokable]
        fn set_combo(self: Pin<&mut KeybindsModel>, id: &QString, combo: &QString);

        /// Turn the shortcut `id` on or off.
        #[qinvokable]
        fn set_enabled(self: Pin<&mut KeybindsModel>, id: &QString, on: bool);

        /// Put one of uQ's shortcuts back to how it came.
        #[qinvokable]
        fn reset(self: Pin<&mut KeybindsModel>, id: &QString);

        /// Put all of uQ's shortcuts back; your own stay.
        #[qinvokable]
        fn reset_all(self: Pin<&mut KeybindsModel>);

        /// Add a shortcut of your own (no `id` in `bind_json`) or replace one (with its `id`).
        #[qinvokable]
        fn save_custom(self: Pin<&mut KeybindsModel>, bind_json: &QString);

        /// Delete one of your own shortcuts.
        #[qinvokable]
        fn delete_custom(self: Pin<&mut KeybindsModel>, id: &QString);

        /// Whether a draft shortcut can be saved, and who already uses its keys:
        /// `{valid, error, conflicts, user_lua, combo_text}`. Synchronous, no request to uqd,
        /// never busy, never sets `error`.
        #[qinvokable]
        fn check(self: &KeybindsModel, draft_json: &QString) -> QString;

        /// The shortcut a key press makes (`key` and `modifiers` as Qt reports them), as
        /// `SUPER + SHIFT + Q`; "" for a modifier on its own or a key uQ has no name for.
        #[qinvokable]
        fn combo_from_key(self: &KeybindsModel, key: i32, modifiers: i32) -> QString;

        /// Pause every shortcut while the next combo is pressed (at most 15 s).
        #[qinvokable]
        fn pause_capture(self: Pin<&mut KeybindsModel>);

        /// Put the shortcuts back on after a capture; harmless when none is in force.
        #[qinvokable]
        fn resume_capture(self: Pin<&mut KeybindsModel>);
    }

    impl cxx_qt::Threading for KeybindsModel {}
    impl cxx_qt::Initialize for KeybindsModel {}
}

use qobject::KeybindsModel;

pub struct KeybindsModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<KeybindsState>,
}

impl Default for KeybindsModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(KeybindsState::new),
        }
    }
}

impl PageModel for KeybindsModel {
    type Logic = KeybindsState;
    // History covers undo and restore; profiles show "From the X profile" under a switch.
    // Coming back to the window re-reads through the page's `refresh()`.
    const FOLLOW: Follow = Follow {
        sections: &["keybinds"],
        theme: false,
        settings: false,
        history: true,
        monitors: false,
        profiles: true,
        autostart: false,
        themes: false,
    };

    fn core(&self) -> &ModelCore<KeybindsState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<KeybindsState> {
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

impl cxx_qt::Initialize for KeybindsModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

/// The error for a request from the page whose JSON argument doesn't parse (a programming error).
fn unreadable(e: serde_json::Error) -> QString {
    QString::from(&format!("That request couldn't be read ({e})."))
}

impl KeybindsModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_combo(self: Pin<&mut Self>, id: &QString, combo: &QString) {
        model_core::start(
            self,
            Op::SetCombo {
                id: id.to_string(),
                combo: combo.to_string(),
            },
        );
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

    fn reset(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::Reset { id: id.to_string() });
    }

    fn reset_all(self: Pin<&mut Self>) {
        model_core::start(self, Op::ResetAll);
    }

    fn save_custom(self: Pin<&mut Self>, bind_json: &QString) {
        match serde_json::from_str(&bind_json.to_string()) {
            Ok(bind) => model_core::start(self, Op::SaveCustom { bind }),
            Err(e) => self.set_error(unreadable(e)),
        }
    }

    fn delete_custom(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::DeleteCustom { id: id.to_string() });
    }

    fn check(&self, draft_json: &QString) -> QString {
        let answer = match serde_json::from_str::<Value>(&draft_json.to_string()) {
            Ok(draft) => match self.core().logic() {
                Some(logic) => logic.check(&draft),
                None => json!({"valid": false, "error": "uQ isn't running", "conflicts": [],
                    "user_lua": [], "combo_text": ""}),
            },
            Err(_) => json!({"valid": false, "error": "this shortcut can't be read",
                "conflicts": [], "user_lua": [], "combo_text": ""}),
        };
        QString::from(&answer.to_string())
    }

    fn combo_from_key(&self, key: i32, modifiers: i32) -> QString {
        let text = keybinds::combo_from_qt(key, u32::from_ne_bytes(modifiers.to_ne_bytes()));
        QString::from(&text.unwrap_or_default())
    }

    fn pause_capture(self: Pin<&mut Self>) {
        model_core::start(self, Op::PauseCapture);
    }

    fn resume_capture(self: Pin<&mut Self>) {
        model_core::start(self, Op::ResumeCapture);
    }
}
