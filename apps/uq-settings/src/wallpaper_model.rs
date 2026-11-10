//! The Wallpaper page's QML object: [`WallpaperState`] driven by `crate::model_core`.
//!
//! Like `appearance_model.rs`, this file sits in `src/` because cxx-qt-build supports one
//! directory of bridge files per QML module (QTBUG-93443).

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::wallpaper::{self, Op, WallpaperState};

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
        /// `{outputs: {"<name>"|"*": entry}, problems: {"<name>": "<message>"}}`; `{}` until uqd
        /// has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused change, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        type WallpaperModel = super::WallpaperModelRust;

        /// Read everything from uqd again.
        #[qinvokable]
        fn refresh(self: Pin<&mut WallpaperModel>);

        /// Replace one output's entry (`"*"` = all displays) with `json_entry`.
        #[qinvokable]
        fn set_entry(self: Pin<&mut WallpaperModel>, output: &QString, json_entry: &QString);

        /// Change some fields of one output's entry: `json_patch` is merged over the entry uqd
        /// has (or over the `"*"` entry for a display without its own).
        #[qinvokable]
        fn patch_entry(self: Pin<&mut WallpaperModel>, output: &QString, json_patch: &QString);

        /// Remove one output's entry.
        #[qinvokable]
        fn clear_entry(self: Pin<&mut WallpaperModel>, output: &QString);

        /// The local path of a dialog's `file://` URL, percent-decoded; "" if it isn't local.
        #[qinvokable]
        fn local_path(self: &WallpaperModel, url: &QString) -> QString;
    }

    impl cxx_qt::Threading for WallpaperModel {}
    impl cxx_qt::Initialize for WallpaperModel {}
}

use qobject::WallpaperModel;

pub struct WallpaperModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<WallpaperState>,
}

impl Default for WallpaperModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(WallpaperState::new),
        }
    }
}

impl PageModel for WallpaperModel {
    type Logic = WallpaperState;
    const FOLLOW: Follow = Follow {
        sections: &["wallpaper"],
        theme: false,
        settings: false,
    };

    fn core(&self) -> &ModelCore<WallpaperState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<WallpaperState> {
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

impl cxx_qt::Initialize for WallpaperModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl WallpaperModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_entry(self: Pin<&mut Self>, output: &QString, json_entry: &QString) {
        match serde_json::from_str(&json_entry.to_string()) {
            Ok(entry) => model_core::start(
                self,
                Op::SetEntry {
                    output: output.to_string(),
                    entry,
                },
            ),
            Err(e) => self.set_error(QString::from(&format!("{output}: not valid JSON ({e})"))),
        }
    }

    fn patch_entry(self: Pin<&mut Self>, output: &QString, json_patch: &QString) {
        match serde_json::from_str(&json_patch.to_string()) {
            Ok(patch) => model_core::start(
                self,
                Op::PatchEntry {
                    output: output.to_string(),
                    patch,
                },
            ),
            Err(e) => self.set_error(QString::from(&format!("{output}: not valid JSON ({e})"))),
        }
    }

    fn clear_entry(self: Pin<&mut Self>, output: &QString) {
        model_core::start(
            self,
            Op::ClearEntry {
                output: output.to_string(),
            },
        );
    }

    fn local_path(&self, url: &QString) -> QString {
        QString::from(
            wallpaper::local_path(&url.to_string())
                .unwrap_or_default()
                .as_str(),
        )
    }
}
