//! The Users page's QML object: [`UsersState`] driven by `crate::model_core`. Password strings
//! from the page go straight into the operation, which hashes and wipes them.

use std::path::PathBuf;
use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::users::{Op, UsersState};
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
        /// `{service, users, me, auto_login, limits, notice}` (see `pages::users`).
        #[qproperty(QString, state_json)]
        #[qproperty(QString, error)]
        #[qproperty(bool, busy)]
        type UsersModel = super::UsersModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut UsersModel>);

        #[qinvokable]
        fn set_real_name(self: Pin<&mut UsersModel>, uid: i64, name: &QString);

        /// Make the account an administrator or a standard user.
        #[qinvokable]
        fn set_admin(self: Pin<&mut UsersModel>, uid: i64, admin: bool);

        #[qinvokable]
        fn set_password(
            self: Pin<&mut UsersModel>,
            uid: i64,
            password: &QString,
            confirm: &QString,
        );

        /// Use the picture at a dialog's `file://` URL as the account's picture.
        #[qinvokable]
        fn set_avatar(self: Pin<&mut UsersModel>, uid: i64, url: &QString);

        #[qinvokable]
        fn add_user(
            self: Pin<&mut UsersModel>,
            name: &QString,
            real_name: &QString,
            admin: bool,
            password: &QString,
            confirm: &QString,
        );

        /// Remove the account; `typed` must be its user name.
        #[qinvokable]
        fn remove_user(self: Pin<&mut UsersModel>, uid: i64, typed: &QString, remove_files: bool);
    }

    impl cxx_qt::Threading for UsersModel {}
    impl cxx_qt::Initialize for UsersModel {}
}

use qobject::UsersModel;

pub struct UsersModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<UsersState>,
}

impl Default for UsersModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(UsersState::new),
        }
    }
}

impl PageModel for UsersModel {
    type Logic = UsersState;
    // Accounts live outside uqd; the page re-reads when it comes back to the front.
    const FOLLOW: Follow = Follow {
        sections: &[],
        theme: false,
        settings: false,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
    };

    fn core(&self) -> &ModelCore<UsersState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<UsersState> {
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

impl cxx_qt::Initialize for UsersModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

fn uid(v: i64) -> u64 {
    u64::try_from(v).unwrap_or(u64::MAX)
}

impl UsersModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_real_name(self: Pin<&mut Self>, id: i64, name: &QString) {
        model_core::start(
            self,
            Op::SetRealName {
                uid: uid(id),
                name: name.to_string(),
            },
        );
    }

    fn set_admin(self: Pin<&mut Self>, id: i64, admin: bool) {
        model_core::start(
            self,
            Op::SetAdmin {
                uid: uid(id),
                admin,
            },
        );
    }

    fn set_password(self: Pin<&mut Self>, id: i64, password: &QString, confirm: &QString) {
        model_core::start(
            self,
            Op::SetPassword {
                uid: uid(id),
                password: password.to_string(),
                confirm: confirm.to_string(),
            },
        );
    }

    fn set_avatar(mut self: Pin<&mut Self>, id: i64, url: &QString) {
        match local_path(&url.to_string()) {
            Some(path) => model_core::start(
                self,
                Op::SetAvatar {
                    uid: uid(id),
                    path: PathBuf::from(path),
                },
            ),
            None => self
                .as_mut()
                .set_error(QString::from("Pick a picture on this computer.")),
        }
    }

    fn add_user(
        self: Pin<&mut Self>,
        name: &QString,
        real_name: &QString,
        admin: bool,
        password: &QString,
        confirm: &QString,
    ) {
        model_core::start(
            self,
            Op::AddUser {
                name: name.to_string(),
                real_name: real_name.to_string(),
                admin,
                password: password.to_string(),
                confirm: confirm.to_string(),
            },
        );
    }

    fn remove_user(self: Pin<&mut Self>, id: i64, typed: &QString, remove_files: bool) {
        model_core::start(
            self,
            Op::RemoveUser {
                uid: uid(id),
                typed: typed.to_string(),
                remove_files,
            },
        );
    }
}
