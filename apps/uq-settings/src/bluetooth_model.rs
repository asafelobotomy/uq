//! The Bluetooth page's QML object: [`BluetoothState`] driven by `crate::model_core`.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::bluetooth::{BluetoothState, Op};

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
        /// `{available, adapter, devices, question, display, agent, notice}` (see
        /// `pages::bluetooth`).
        #[qproperty(QString, state_json)]
        #[qproperty(QString, error)]
        #[qproperty(bool, busy)]
        type BluetoothModel = super::BluetoothModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut BluetoothModel>);
        #[qinvokable]
        fn set_powered(self: Pin<&mut BluetoothModel>, on: bool);
        #[qinvokable]
        fn set_discoverable(self: Pin<&mut BluetoothModel>, on: bool);
        /// Start or stop looking for devices.
        #[qinvokable]
        fn scan(self: Pin<&mut BluetoothModel>, on: bool);
        #[qinvokable]
        fn pair(self: Pin<&mut BluetoothModel>, device: &QString);
        #[qinvokable]
        fn cancel_pairing(self: Pin<&mut BluetoothModel>, device: &QString);
        #[qinvokable]
        fn connect_device(self: Pin<&mut BluetoothModel>, device: &QString);
        #[qinvokable]
        fn disconnect_device(self: Pin<&mut BluetoothModel>, device: &QString);
        #[qinvokable]
        fn set_trusted(self: Pin<&mut BluetoothModel>, device: &QString, on: bool);
        #[qinvokable]
        fn forget(self: Pin<&mut BluetoothModel>, device: &QString);
        /// Answer the pairing question on show; `value` is a typed PIN or passkey, or "".
        #[qinvokable]
        fn answer(self: Pin<&mut BluetoothModel>, id: i64, yes: bool, value: &QString);
    }

    impl cxx_qt::Threading for BluetoothModel {}
    impl cxx_qt::Initialize for BluetoothModel {}
}

use qobject::BluetoothModel;

pub struct BluetoothModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<BluetoothState>,
}

impl Default for BluetoothModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(BluetoothState::new),
        }
    }
}

impl PageModel for BluetoothModel {
    type Logic = BluetoothState;
    const FOLLOW: Follow = Follow {
        sections: &[],
        theme: false,
        settings: false,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
    };

    fn core(&self) -> &ModelCore<BluetoothState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<BluetoothState> {
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

impl cxx_qt::Initialize for BluetoothModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl BluetoothModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_powered(self: Pin<&mut Self>, on: bool) {
        model_core::start(self, Op::SetPowered(on));
    }

    fn set_discoverable(self: Pin<&mut Self>, on: bool) {
        model_core::start(self, Op::SetDiscoverable(on));
    }

    fn scan(self: Pin<&mut Self>, on: bool) {
        model_core::start(self, Op::Scan(on));
    }

    fn pair(self: Pin<&mut Self>, device: &QString) {
        model_core::start(self, Op::Pair(device.to_string()));
    }

    fn cancel_pairing(self: Pin<&mut Self>, device: &QString) {
        model_core::start(self, Op::CancelPairing(device.to_string()));
    }

    fn connect_device(self: Pin<&mut Self>, device: &QString) {
        model_core::start(self, Op::Connect(device.to_string()));
    }

    fn disconnect_device(self: Pin<&mut Self>, device: &QString) {
        model_core::start(self, Op::Disconnect(device.to_string()));
    }

    fn set_trusted(self: Pin<&mut Self>, device: &QString, on: bool) {
        model_core::start(self, Op::SetTrusted(device.to_string(), on));
    }

    fn forget(self: Pin<&mut Self>, device: &QString) {
        model_core::start(self, Op::Forget(device.to_string()));
    }

    fn answer(self: Pin<&mut Self>, id: i64, yes: bool, value: &QString) {
        model_core::start(
            self,
            Op::Answer {
                id: u64::try_from(id).unwrap_or(0),
                yes,
                value: value.to_string(),
            },
        );
    }
}
