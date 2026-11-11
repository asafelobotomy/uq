//! The Date and time page's QML object: [`DateTimeState`] driven by `crate::model_core`.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::datetime::{DateTimeState, Op};

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
        /// `{timedate, clock_24h, notice}` (see `pages::datetime`).
        #[qproperty(QString, state_json)]
        /// The message of the last refused request, "" when there is none.
        #[qproperty(QString, error)]
        /// Whether a request is in flight (a password prompt may be up).
        #[qproperty(bool, busy)]
        type DateTimeModel = super::DateTimeModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut DateTimeModel>);

        #[qinvokable]
        fn set_timezone(self: Pin<&mut DateTimeModel>, zone: &QString);

        /// Set the time from the network, or not.
        #[qinvokable]
        fn set_ntp(self: Pin<&mut DateTimeModel>, on: bool);

        /// Set the clock to `usec` microseconds since 1970 (UTC).
        #[qinvokable]
        fn set_time(self: Pin<&mut DateTimeModel>, usec: i64);

        /// The bar's clock in 24-hour form, or 12-hour.
        #[qinvokable]
        fn set_clock_24h(self: Pin<&mut DateTimeModel>, on: bool);
    }

    impl cxx_qt::Threading for DateTimeModel {}
    impl cxx_qt::Initialize for DateTimeModel {}
}

use qobject::DateTimeModel;

pub struct DateTimeModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<DateTimeState>,
}

impl Default for DateTimeModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(DateTimeState::new),
        }
    }
}

impl PageModel for DateTimeModel {
    type Logic = DateTimeState;
    const FOLLOW: Follow = Follow {
        // The 24-hour clock is uQ's `shell` section.
        sections: &["shell"],
        theme: false,
        settings: false,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
    };

    fn core(&self) -> &ModelCore<DateTimeState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<DateTimeState> {
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

impl cxx_qt::Initialize for DateTimeModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl DateTimeModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_timezone(self: Pin<&mut Self>, zone: &QString) {
        model_core::start(
            self,
            Op::SetTimezone {
                zone: zone.to_string(),
            },
        );
    }

    fn set_ntp(self: Pin<&mut Self>, on: bool) {
        model_core::start(self, Op::SetNtp { on });
    }

    fn set_time(self: Pin<&mut Self>, usec: i64) {
        model_core::start(self, Op::SetTime { usec });
    }

    fn set_clock_24h(self: Pin<&mut Self>, on: bool) {
        model_core::start(self, Op::SetClock24h { on });
    }
}
