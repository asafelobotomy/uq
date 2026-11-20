//! The Printers page's QML object: [`PrintersState`] driven by `crate::model_core`.

use std::collections::BTreeMap;
use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::printers::{Op, PrintersState};

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
        /// `{available, printers, default, found, looking, options, drivers, notice, classic}`
        /// (see `pages::printers`).
        #[qproperty(QString, state_json)]
        #[qproperty(QString, error)]
        #[qproperty(bool, busy)]
        type PrintersModel = super::PrintersModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut PrintersModel>);
        /// Look for printers on the network and the USB ports.
        #[qinvokable]
        fn look(self: Pin<&mut PrintersModel>);
        #[qinvokable]
        fn find_drivers(
            self: Pin<&mut PrintersModel>,
            uri: &QString,
            device_id: &QString,
            make_model: &QString,
        );
        #[qinvokable]
        fn add(
            self: Pin<&mut PrintersModel>,
            uri: &QString,
            name: &QString,
            location: &QString,
            driver: &QString,
        );
        #[qinvokable]
        fn remove(self: Pin<&mut PrintersModel>, printer: &QString);
        #[qinvokable]
        fn set_default(self: Pin<&mut PrintersModel>, printer: &QString);
        #[qinvokable]
        fn pause(self: Pin<&mut PrintersModel>, printer: &QString);
        #[qinvokable]
        fn resume(self: Pin<&mut PrintersModel>, printer: &QString);
        #[qinvokable]
        fn test_page(self: Pin<&mut PrintersModel>, printer: &QString);
        #[qinvokable]
        fn cancel_job(self: Pin<&mut PrintersModel>, id: i32);
        #[qinvokable]
        fn cancel_all(self: Pin<&mut PrintersModel>, printer: &QString);
        #[qinvokable]
        fn load_options(self: Pin<&mut PrintersModel>, printer: &QString);
        /// `options_json` is `{"<option>": "<choice>"}` of the options that changed.
        #[qinvokable]
        fn save(
            self: Pin<&mut PrintersModel>,
            printer: &QString,
            name: &QString,
            location: &QString,
            options_json: &QString,
        );
        #[qinvokable]
        fn turn_on(self: Pin<&mut PrintersModel>);
    }

    impl cxx_qt::Threading for PrintersModel {}
    impl cxx_qt::Initialize for PrintersModel {}
}

use qobject::PrintersModel;

pub struct PrintersModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<PrintersState>,
}

impl Default for PrintersModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(PrintersState::new),
        }
    }
}

impl PageModel for PrintersModel {
    type Logic = PrintersState;
    const FOLLOW: Follow = Follow {
        sections: &[],
        theme: false,
        settings: false,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
        themes: false,
    };

    fn core(&self) -> &ModelCore<PrintersState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<PrintersState> {
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

impl cxx_qt::Initialize for PrintersModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

impl PrintersModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn look(self: Pin<&mut Self>) {
        model_core::start(self, Op::Look);
    }

    fn find_drivers(
        self: Pin<&mut Self>,
        uri: &QString,
        device_id: &QString,
        make_model: &QString,
    ) {
        model_core::start(
            self,
            Op::Drivers {
                uri: uri.to_string(),
                device_id: device_id.to_string(),
                make_model: make_model.to_string(),
            },
        );
    }

    fn add(
        self: Pin<&mut Self>,
        uri: &QString,
        name: &QString,
        location: &QString,
        driver: &QString,
    ) {
        model_core::start(
            self,
            Op::Add {
                uri: uri.to_string(),
                name: name.to_string(),
                location: location.to_string(),
                driver: driver.to_string(),
            },
        );
    }

    fn remove(self: Pin<&mut Self>, printer: &QString) {
        model_core::start(self, Op::Remove(printer.to_string()));
    }

    fn set_default(self: Pin<&mut Self>, printer: &QString) {
        model_core::start(self, Op::SetDefault(printer.to_string()));
    }

    fn pause(self: Pin<&mut Self>, printer: &QString) {
        model_core::start(self, Op::Pause(printer.to_string()));
    }

    fn resume(self: Pin<&mut Self>, printer: &QString) {
        model_core::start(self, Op::Resume(printer.to_string()));
    }

    fn test_page(self: Pin<&mut Self>, printer: &QString) {
        model_core::start(self, Op::TestPage(printer.to_string()));
    }

    fn cancel_job(self: Pin<&mut Self>, id: i32) {
        model_core::start(self, Op::CancelJob(id));
    }

    fn cancel_all(self: Pin<&mut Self>, printer: &QString) {
        model_core::start(self, Op::CancelAll(printer.to_string()));
    }

    fn load_options(self: Pin<&mut Self>, printer: &QString) {
        model_core::start(self, Op::LoadOptions(printer.to_string()));
    }

    fn save(
        self: Pin<&mut Self>,
        printer: &QString,
        name: &QString,
        location: &QString,
        options_json: &QString,
    ) {
        // Anything but a flat object of strings changes no option.
        let options: BTreeMap<String, String> =
            serde_json::from_str(&options_json.to_string()).unwrap_or_default();
        model_core::start(
            self,
            Op::Save {
                printer: printer.to_string(),
                name: name.to_string(),
                location: location.to_string(),
                options,
            },
        );
    }

    fn turn_on(self: Pin<&mut Self>) {
        model_core::start(self, Op::TurnOn);
    }
}
