//! The Login screen page's QML object: [`LoginScreenState`] driven by `crate::model_core`.

use std::path::PathBuf;
use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::login_screen::{LoginScreenState, Op};
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
        /// `{installed, helper, helper_note, greeter, variants, mine, wallpaper, monitors,
        /// auto_login, notice}` (see `pages::login_screen`); `{}` until read.
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request is in flight (a password prompt may be up).
        #[qproperty(bool, busy)]
        type LoginScreenModel = super::LoginScreenModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut LoginScreenModel>);

        /// Save the look; `theme` "" copies your own theme and variant now.
        #[qinvokable]
        fn set_appearance(
            self: Pin<&mut LoginScreenModel>,
            theme: &QString,
            variant: &QString,
            hide_users: bool,
            reduce_motion: bool,
        );

        /// Copy your desktop's picture to the login screen.
        #[qinvokable]
        fn wallpaper_mine(self: Pin<&mut LoginScreenModel>);

        /// Copy the picture at a dialog's `file://` URL to the login screen.
        #[qinvokable]
        fn wallpaper_file(self: Pin<&mut LoginScreenModel>, url: &QString);

        /// Go back to the theme's background colour.
        #[qinvokable]
        fn clear_wallpaper(self: Pin<&mut LoginScreenModel>);

        /// Copy your display layout to the login screen.
        #[qinvokable]
        fn use_my_displays(self: Pin<&mut LoginScreenModel>);

        /// Log you in automatically when the computer starts, or stop doing so.
        #[qinvokable]
        fn set_auto_login(self: Pin<&mut LoginScreenModel>, on: bool);
    }

    impl cxx_qt::Threading for LoginScreenModel {}
    impl cxx_qt::Initialize for LoginScreenModel {}
}

use qobject::LoginScreenModel;

pub struct LoginScreenModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<LoginScreenState>,
}

impl Default for LoginScreenModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(LoginScreenState::new),
        }
    }
}

impl PageModel for LoginScreenModel {
    type Logic = LoginScreenState;
    // "Same as mine" reads your theme and wallpaper, so their changes refresh the page.
    const FOLLOW: Follow = Follow {
        sections: &["wallpaper", "monitors"],
        theme: true,
        settings: false,
        history: false,
        monitors: true,
        profiles: false,
        autostart: false,
    };

    fn core(&self) -> &ModelCore<LoginScreenState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<LoginScreenState> {
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

impl cxx_qt::Initialize for LoginScreenModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl LoginScreenModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_appearance(
        self: Pin<&mut Self>,
        theme: &QString,
        variant: &QString,
        hide_users: bool,
        reduce_motion: bool,
    ) {
        model_core::start(
            self,
            Op::SetAppearance {
                theme: theme.to_string(),
                variant: variant.to_string(),
                hide_users,
                reduce_motion,
            },
        );
    }

    fn wallpaper_mine(self: Pin<&mut Self>) {
        model_core::start(self, Op::WallpaperMine);
    }

    fn wallpaper_file(mut self: Pin<&mut Self>, url: &QString) {
        match local_path(&url.to_string()) {
            Some(path) => model_core::start(
                self,
                Op::WallpaperFile {
                    path: PathBuf::from(path),
                },
            ),
            None => self
                .as_mut()
                .set_error(QString::from("Pick a picture on this computer.")),
        }
    }

    fn clear_wallpaper(self: Pin<&mut Self>) {
        model_core::start(self, Op::ClearWallpaper);
    }

    fn use_my_displays(self: Pin<&mut Self>) {
        model_core::start(self, Op::UseMyDisplays);
    }

    fn set_auto_login(self: Pin<&mut Self>, on: bool) {
        model_core::start(self, Op::SetAutoLogin { on });
    }
}
