//! The theme row's QML object: [`ThemesState`] driven by `crate::model_core`. The Appearance page
//! holds one beside its `AppearanceModel`.
//!
//! Like `history_model.rs`, this file sits in `src/` because cxx-qt-build supports one directory
//! of bridge files per QML module (QTBUG-93443).

use std::path::PathBuf;
use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde_json::{Value, json};

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::themes::{Op, ThemesState};
use crate::pages::wallpaper::local_path;

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
        /// `{themes, skipped, rootIsLink, theme, variant, editor, notice, conflict, deleted,
        /// imported}` (see `pages::themes`); `{}` until uqd has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        type ThemesModel = super::ThemesModelRust;

        /// Read the themes again.
        #[qinvokable]
        fn refresh(self: Pin<&mut ThemesModel>);

        /// Add the theme in a dialog's `file://` URL; `replace` answers a conflict, and
        /// `with_look` also uses the file's window gaps, corners and effects.
        #[qinvokable]
        fn import_from(self: Pin<&mut ThemesModel>, url: &QString, replace: bool, with_look: bool);

        /// Save theme `id` to a dialog's `file://` URL.
        #[qinvokable]
        fn export_to(self: Pin<&mut ThemesModel>, id: &QString, url: &QString, with_look: bool);

        /// Remove one of the user's themes. Not `delete`: that is a C++ keyword.
        #[qinvokable]
        fn remove(self: Pin<&mut ThemesModel>, id: &QString);

        /// Open the editor on a copy of theme `from`, or (`edit`) on the user's theme itself.
        #[qinvokable]
        fn open_editor(self: Pin<&mut ThemesModel>, from: &QString, edit: bool);

        #[qinvokable]
        fn close_editor(self: Pin<&mut ThemesModel>);

        /// Save the editor's draft (`{name, author, dark: {…}, light: {…}}` as JSON).
        #[qinvokable]
        fn save(self: Pin<&mut ThemesModel>, draft_json: &QString);

        /// Switch to theme `id`, keeping light or dark.
        #[qinvokable]
        fn use_theme(self: Pin<&mut ThemesModel>, id: &QString);

        /// `{dark: {tokens, problems}, light: {…}, ok, nameProblem, authorProblem}` for a draft.
        /// Synchronous, no request to uqd, never busy, never sets `error`.
        #[qinvokable]
        fn preview(self: &ThemesModel, draft_json: &QString) -> QString;
    }

    impl cxx_qt::Threading for ThemesModel {}
    impl cxx_qt::Initialize for ThemesModel {}
}

use qobject::ThemesModel;

pub struct ThemesModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<ThemesState>,
}

impl Default for ThemesModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(ThemesState::new),
        }
    }
}

impl PageModel for ThemesModel {
    type Logic = ThemesState;
    // The active theme is part of the state; `ThemesChanged` covers the user's themes.
    const FOLLOW: Follow = Follow {
        sections: &[],
        theme: true,
        settings: false,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
        themes: true,
    };

    fn core(&self) -> &ModelCore<ThemesState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<ThemesState> {
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

impl cxx_qt::Initialize for ThemesModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

/// A dialog's URL as a local path, or the message to show when it isn't on this computer.
fn dialog_path(url: &QString) -> Result<PathBuf, QString> {
    local_path(&url.to_string())
        .map(PathBuf::from)
        .ok_or_else(|| QString::from("Choose a file on this computer."))
}

/// The error for a request from the page whose JSON argument doesn't parse (a programming error).
fn unreadable(e: serde_json::Error) -> QString {
    QString::from(&format!("That request couldn't be read ({e})."))
}

impl ThemesModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn import_from(self: Pin<&mut Self>, url: &QString, replace: bool, with_look: bool) {
        match dialog_path(url) {
            Ok(path) => model_core::start(
                self,
                Op::Import {
                    path,
                    replace,
                    with_look,
                },
            ),
            Err(e) => self.set_error(e),
        }
    }

    fn export_to(self: Pin<&mut Self>, id: &QString, url: &QString, with_look: bool) {
        match dialog_path(url) {
            Ok(path) => model_core::start(
                self,
                Op::Export {
                    id: id.to_string(),
                    path,
                    with_look,
                },
            ),
            Err(e) => self.set_error(e),
        }
    }

    fn remove(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::Delete { id: id.to_string() });
    }

    fn open_editor(self: Pin<&mut Self>, from: &QString, edit: bool) {
        model_core::start(
            self,
            Op::OpenEditor {
                from: from.to_string(),
                edit,
            },
        );
    }

    fn close_editor(self: Pin<&mut Self>) {
        model_core::start(self, Op::CloseEditor);
    }

    fn save(self: Pin<&mut Self>, draft_json: &QString) {
        match serde_json::from_str(&draft_json.to_string()) {
            Ok(draft) => model_core::start(self, Op::Save { draft }),
            Err(e) => self.set_error(unreadable(e)),
        }
    }

    fn use_theme(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::Use { id: id.to_string() });
    }

    fn preview(&self, draft_json: &QString) -> QString {
        let answer = match serde_json::from_str::<Value>(&draft_json.to_string()) {
            Ok(draft) => match self.core().logic() {
                Some(logic) => logic.preview(&draft),
                None => unavailable("uQ isn't running"),
            },
            Err(_) => unavailable("this theme can't be read"),
        };
        QString::from(&answer.to_string())
    }
}

/// A preview that blocks saving, for a draft that can't be read or no uqd.
fn unavailable(reason: &str) -> Value {
    json!({
        "dark": {"tokens": {}, "problems": [reason]},
        "light": {"tokens": {}, "problems": []},
        "ok": false,
        "nameProblem": "",
        "authorProblem": "",
    })
}
