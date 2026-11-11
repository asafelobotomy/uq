//! The Profiles page's QML object: [`ProfilesState`] driven by `crate::model_core`.
//!
//! Like `history_model.rs`, this file sits in `src/` because cxx-qt-build supports one directory
//! of bridge files per QML module (QTBUG-93443).

use std::path::PathBuf;
use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::profiles::{Op, ProfilesState};
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
        /// `{profiles, active, available, pages, icons, notice, report, conflict}` (see
        /// `pages::profiles`); `{}` until uqd has been read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request to uqd is in flight.
        #[qproperty(bool, busy)]
        type ProfilesModel = super::ProfilesModelRust;

        /// Read the profiles and the current settings again.
        #[qinvokable]
        fn refresh(self: Pin<&mut ProfilesModel>);

        /// Turn profile `id` on (it replaces the one that's on, if any).
        #[qinvokable]
        fn apply(self: Pin<&mut ProfilesModel>, id: &QString);

        /// "Back to normal": turn the active profile off.
        #[qinvokable]
        fn off(self: Pin<&mut ProfilesModel>);

        /// A new profile with the current values of these pages' settings; `pages_json` is a
        /// JSON array of page ids.
        #[qinvokable]
        fn create_from_pages(
            self: Pin<&mut ProfilesModel>,
            name: &QString,
            icon: &QString,
            pages_json: &QString,
        );

        /// A new profile with the current values of the settings changed since `since` (Unix
        /// seconds; a JS number, so a double).
        #[qinvokable]
        fn create_from_history(
            self: Pin<&mut ProfilesModel>,
            name: &QString,
            icon: &QString,
            since: f64,
        );

        /// Add keys to profile `id` with their current values; `keys_json` is
        /// `[[domain, [path…]], …]`.
        #[qinvokable]
        fn add_keys(self: Pin<&mut ProfilesModel>, id: &QString, keys_json: &QString);

        /// Remove one key from profile `id`; `path_json` is its path as a JSON array.
        #[qinvokable]
        fn remove_key(
            self: Pin<&mut ProfilesModel>,
            id: &QString,
            domain: &QString,
            path_json: &QString,
        );

        /// Give profile `id` a new name and icon ("" for none).
        #[qinvokable]
        fn rename(self: Pin<&mut ProfilesModel>, id: &QString, name: &QString, icon: &QString);

        /// Delete profile `id`. Not `delete`: that is a C++ keyword.
        #[qinvokable]
        fn delete_profile(self: Pin<&mut ProfilesModel>, id: &QString);

        /// Save profile `id` to a dialog's `file://` URL.
        #[qinvokable]
        fn export_to(self: Pin<&mut ProfilesModel>, id: &QString, url: &QString);

        /// Import the profile file at a dialog's `file://` URL; `replace` answers a conflict.
        #[qinvokable]
        fn import_from(self: Pin<&mut ProfilesModel>, url: &QString, replace: bool);
    }

    impl cxx_qt::Threading for ProfilesModel {}
    impl cxx_qt::Initialize for ProfilesModel {}
}

use qobject::ProfilesModel;

pub struct ProfilesModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<ProfilesState>,
}

impl Default for ProfilesModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(ProfilesState::new),
        }
    }
}

impl PageModel for ProfilesModel {
    type Logic = ProfilesState;
    // `available` holds the current values of these domains; `ProfileChanged` covers the list
    // and which profile is on.
    const FOLLOW: Follow = Follow {
        sections: &[
            "look",
            "layouts",
            "appearance",
            "shell",
            "wallpaper",
            "rules",
            "env",
            "compat",
            "autostart",
        ],
        theme: true,
        settings: false,
        history: true,
        monitors: false,
        profiles: true,
        autostart: false,
        themes: false,
    };

    fn core(&self) -> &ModelCore<ProfilesState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<ProfilesState> {
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

impl cxx_qt::Initialize for ProfilesModel {
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

impl ProfilesModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn apply(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::Apply { id: id.to_string() });
    }

    fn off(self: Pin<&mut Self>) {
        model_core::start(self, Op::Off);
    }

    fn create_from_pages(
        self: Pin<&mut Self>,
        name: &QString,
        icon: &QString,
        pages_json: &QString,
    ) {
        match serde_json::from_str(&pages_json.to_string()) {
            Ok(pages) => model_core::start(
                self,
                Op::CreateFromPages {
                    name: name.to_string(),
                    icon: icon.to_string(),
                    pages,
                },
            ),
            Err(e) => self.set_error(unreadable(e)),
        }
    }

    fn create_from_history(self: Pin<&mut Self>, name: &QString, icon: &QString, since: f64) {
        model_core::start(
            self,
            Op::CreateFromHistory {
                name: name.to_string(),
                icon: icon.to_string(),
                since: since as i64,
            },
        );
    }

    fn add_keys(self: Pin<&mut Self>, id: &QString, keys_json: &QString) {
        match serde_json::from_str(&keys_json.to_string()) {
            Ok(keys) => model_core::start(
                self,
                Op::AddKeys {
                    id: id.to_string(),
                    keys,
                },
            ),
            Err(e) => self.set_error(unreadable(e)),
        }
    }

    fn remove_key(self: Pin<&mut Self>, id: &QString, domain: &QString, path_json: &QString) {
        match serde_json::from_str(&path_json.to_string()) {
            Ok(path) => model_core::start(
                self,
                Op::RemoveKey {
                    id: id.to_string(),
                    domain: domain.to_string(),
                    path,
                },
            ),
            Err(e) => self.set_error(unreadable(e)),
        }
    }

    fn rename(self: Pin<&mut Self>, id: &QString, name: &QString, icon: &QString) {
        model_core::start(
            self,
            Op::Rename {
                id: id.to_string(),
                name: name.to_string(),
                icon: icon.to_string(),
            },
        );
    }

    fn delete_profile(self: Pin<&mut Self>, id: &QString) {
        model_core::start(self, Op::Delete { id: id.to_string() });
    }

    fn export_to(self: Pin<&mut Self>, id: &QString, url: &QString) {
        match dialog_path(url) {
            Ok(path) => model_core::start(
                self,
                Op::Export {
                    id: id.to_string(),
                    path,
                },
            ),
            Err(e) => self.set_error(e),
        }
    }

    fn import_from(self: Pin<&mut Self>, url: &QString, replace: bool) {
        match dialog_path(url) {
            Ok(path) => model_core::start(self, Op::Import { path, replace }),
            Err(e) => self.set_error(e),
        }
    }
}
