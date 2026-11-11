//! The Sound page's QML object: [`SoundState`] driven by `crate::model_core`.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::sound::{Op, SoundState};

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
        /// `{available, outputs, inputs, playback, recording, cards, test, notice}` (see
        /// `pages::sound`).
        #[qproperty(QString, state_json)]
        #[qproperty(QString, error)]
        #[qproperty(bool, busy)]
        type SoundModel = super::SoundModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut SoundModel>);
        /// Make output or input `id` the default.
        #[qinvokable]
        fn set_default(self: Pin<&mut SoundModel>, id: i32);
        /// `percent` 0 to 100.
        #[qinvokable]
        fn set_volume(self: Pin<&mut SoundModel>, id: i32, percent: i32);
        #[qinvokable]
        fn set_mute(self: Pin<&mut SoundModel>, id: i32, on: bool);
        #[qinvokable]
        fn set_profile(self: Pin<&mut SoundModel>, card: i32, index: i32);
        /// Play app stream `id` through the device with `serial`; 0 follows the default again.
        #[qinvokable]
        fn route(self: Pin<&mut SoundModel>, id: i32, serial: f64);
        /// Play the test sound on output `id`.
        #[qinvokable]
        fn test_sound(self: Pin<&mut SoundModel>, id: i32);
    }

    impl cxx_qt::Threading for SoundModel {}
    impl cxx_qt::Initialize for SoundModel {}
}

use qobject::SoundModel;

pub struct SoundModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<SoundState>,
}

impl Default for SoundModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(SoundState::new),
        }
    }
}

impl PageModel for SoundModel {
    type Logic = SoundState;
    const FOLLOW: Follow = Follow {
        sections: &[],
        theme: false,
        settings: false,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
    };

    fn core(&self) -> &ModelCore<SoundState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<SoundState> {
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

impl cxx_qt::Initialize for SoundModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

fn id(v: i32) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

impl SoundModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_default(self: Pin<&mut Self>, node: i32) {
        model_core::start(self, Op::SetDefault(id(node)));
    }

    fn set_volume(self: Pin<&mut Self>, node: i32, percent: i32) {
        model_core::start(
            self,
            Op::SetVolume(id(node), u32::try_from(percent.clamp(0, 100)).unwrap_or(0)),
        );
    }

    fn set_mute(self: Pin<&mut Self>, node: i32, on: bool) {
        model_core::start(self, Op::SetMute(id(node), on));
    }

    fn set_profile(self: Pin<&mut Self>, card: i32, index: i32) {
        model_core::start(self, Op::SetProfile(id(card), id(index)));
    }

    fn route(self: Pin<&mut Self>, stream: i32, serial: f64) {
        // Serials pass through QML as numbers; they stay far below 2^53.
        let serial = if serial.is_finite() && serial >= 0.0 {
            serial as u64
        } else {
            0
        };
        model_core::start(self, Op::Route(id(stream), serial));
    }

    fn test_sound(self: Pin<&mut Self>, node: i32) {
        model_core::start(self, Op::Test(id(node)));
    }
}
