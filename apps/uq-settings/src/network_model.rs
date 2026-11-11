//! The Network page's QML object: [`NetworkState`] driven by `crate::model_core`. Passwords
//! from the page go straight into the operation and on to NetworkManager.

use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::model_core::{self, Follow, ModelCore, PageModel};
use crate::pages::network::profile::{Eap, IpInput};
use crate::pages::network::wireguard::PeerEdit;
use crate::pages::network::{NetworkState, Op};
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
        /// `{available, wifi, wired, saved, vpn_on, openvpn, hotspot, secret, agent, notice}`
        /// (see `pages::network`).
        #[qproperty(QString, state_json)]
        #[qproperty(QString, error)]
        #[qproperty(bool, busy)]
        type NetworkModel = super::NetworkModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut NetworkModel>);
        #[qinvokable]
        fn set_wifi(self: Pin<&mut NetworkModel>, on: bool);
        #[qinvokable]
        fn scan(self: Pin<&mut NetworkModel>);
        /// Join a network: `ap` and `device` from the list; `password` may be empty.
        #[qinvokable]
        fn connect_wifi(
            self: Pin<&mut NetworkModel>,
            ap: &QString,
            device: &QString,
            password: &QString,
            all_users: bool,
        );
        /// Join a work or school network. `details` is `{method, identity, anonymous_identity,
        /// password, phase2, ca_cert, domain, client_cert, private_key, key_password}`; the
        /// file fields are dialog URLs or "".
        #[qinvokable]
        fn connect_enterprise(
            self: Pin<&mut NetworkModel>,
            ap: &QString,
            device: &QString,
            details: &QString,
            all_users: bool,
        );
        #[qinvokable]
        fn connect_hidden(
            self: Pin<&mut NetworkModel>,
            ssid: &QString,
            security: &QString,
            password: &QString,
            all_users: bool,
        );
        #[qinvokable]
        fn activate(self: Pin<&mut NetworkModel>, uuid: &QString);
        #[qinvokable]
        fn deactivate(self: Pin<&mut NetworkModel>, uuid: &QString);
        #[qinvokable]
        fn forget(self: Pin<&mut NetworkModel>, uuid: &QString);
        #[qinvokable]
        fn set_autoconnect(self: Pin<&mut NetworkModel>, uuid: &QString, on: bool);
        /// `metered`: `auto`, `yes` or `no`.
        #[qinvokable]
        fn set_metered(self: Pin<&mut NetworkModel>, uuid: &QString, metered: &QString);
        /// Whether everyone on this computer may use the connection.
        #[qinvokable]
        fn set_everyone(self: Pin<&mut NetworkModel>, uuid: &QString, on: bool);
        /// `family` 4 or 6; `method` `auto`, `manual` or `disabled`.
        #[qinvokable]
        fn set_ip(
            self: Pin<&mut NetworkModel>,
            uuid: &QString,
            family: i32,
            method: &QString,
            addresses: &QString,
            gateway: &QString,
            dns: &QString,
        );
        /// Import a WireGuard config file chosen in a dialog.
        #[qinvokable]
        fn import_wireguard(self: Pin<&mut NetworkModel>, url: &QString, all_users: bool);
        #[qinvokable]
        fn edit_wireguard(
            self: Pin<&mut NetworkModel>,
            uuid: &QString,
            endpoint: &QString,
            allowed_ips: &QString,
            keepalive: i32,
        );
        #[qinvokable]
        fn hotspot(self: Pin<&mut NetworkModel>, on: bool, name: &QString, password: &QString);
        /// Answer the password question on show: `values` is `{"<field>": "<text>"}`.
        #[qinvokable]
        fn answer_secret(self: Pin<&mut NetworkModel>, id: i64, values: &QString);
        #[qinvokable]
        fn cancel_secret(self: Pin<&mut NetworkModel>, id: i64);
    }

    impl cxx_qt::Threading for NetworkModel {}
    impl cxx_qt::Initialize for NetworkModel {}
}

use qobject::NetworkModel;

pub struct NetworkModelRust {
    state_json: QString,
    error: QString,
    busy: bool,
    core: ModelCore<NetworkState>,
}

impl Default for NetworkModelRust {
    fn default() -> Self {
        Self {
            state_json: QString::from("{}"),
            error: QString::default(),
            busy: false,
            core: ModelCore::new(NetworkState::new),
        }
    }
}

impl PageModel for NetworkModel {
    type Logic = NetworkState;
    const FOLLOW: Follow = Follow {
        sections: &[],
        theme: false,
        settings: false,
        history: false,
        monitors: false,
        profiles: false,
        autostart: false,
    };

    fn core(&self) -> &ModelCore<NetworkState> {
        &self.rust().core
    }
    fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<NetworkState> {
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

impl cxx_qt::Initialize for NetworkModel {
    fn initialize(self: Pin<&mut Self>) {
        model_core::initialize(self);
    }
}

fn s(q: &QString) -> String {
    q.to_string()
}

impl NetworkModel {
    fn refresh(self: Pin<&mut Self>) {
        model_core::start(self, Op::Refresh);
    }

    fn set_wifi(self: Pin<&mut Self>, on: bool) {
        model_core::start(self, Op::SetWifi(on));
    }

    fn scan(self: Pin<&mut Self>) {
        model_core::start(self, Op::Scan);
    }

    fn connect_wifi(
        self: Pin<&mut Self>,
        ap: &QString,
        device: &QString,
        password: &QString,
        all_users: bool,
    ) {
        model_core::start(
            self,
            Op::ConnectWifi {
                ap: s(ap),
                device: s(device),
                password: s(password),
                all_users,
                eap: None,
            },
        );
    }

    fn connect_enterprise(
        self: Pin<&mut Self>,
        ap: &QString,
        device: &QString,
        details: &QString,
        all_users: bool,
    ) {
        let d: HashMap<String, String> =
            serde_json::from_str(&details.to_string()).unwrap_or_default();
        let field = |k: &str| d.get(k).cloned().unwrap_or_default();
        let path = |k: &str| local_path(&field(k)).unwrap_or_default();
        model_core::start(
            self,
            Op::ConnectWifi {
                ap: s(ap),
                device: s(device),
                password: String::new(),
                all_users,
                eap: Some(Eap {
                    method: field("method"),
                    identity: field("identity"),
                    anonymous_identity: field("anonymous_identity"),
                    password: field("password"),
                    phase2: field("phase2"),
                    ca_cert: path("ca_cert"),
                    domain: field("domain"),
                    client_cert: path("client_cert"),
                    private_key: path("private_key"),
                    key_password: field("key_password"),
                }),
            },
        );
    }

    fn connect_hidden(
        self: Pin<&mut Self>,
        ssid: &QString,
        security: &QString,
        password: &QString,
        all_users: bool,
    ) {
        model_core::start(
            self,
            Op::ConnectHidden {
                ssid: s(ssid),
                security: s(security),
                password: s(password),
                all_users,
            },
        );
    }

    fn activate(self: Pin<&mut Self>, uuid: &QString) {
        model_core::start(self, Op::Activate { uuid: s(uuid) });
    }

    fn deactivate(self: Pin<&mut Self>, uuid: &QString) {
        model_core::start(self, Op::Deactivate { uuid: s(uuid) });
    }

    fn forget(self: Pin<&mut Self>, uuid: &QString) {
        model_core::start(self, Op::Forget { uuid: s(uuid) });
    }

    fn set_autoconnect(self: Pin<&mut Self>, uuid: &QString, on: bool) {
        model_core::start(self, Op::SetAutoconnect { uuid: s(uuid), on });
    }

    fn set_metered(self: Pin<&mut Self>, uuid: &QString, metered: &QString) {
        model_core::start(
            self,
            Op::SetMetered {
                uuid: s(uuid),
                metered: s(metered),
            },
        );
    }

    fn set_everyone(self: Pin<&mut Self>, uuid: &QString, on: bool) {
        model_core::start(self, Op::SetEveryone { uuid: s(uuid), on });
    }

    fn set_ip(
        self: Pin<&mut Self>,
        uuid: &QString,
        family: i32,
        method: &QString,
        addresses: &QString,
        gateway: &QString,
        dns: &QString,
    ) {
        model_core::start(
            self,
            Op::SetIp {
                uuid: s(uuid),
                family: u8::try_from(family).unwrap_or(0),
                ip: IpInput {
                    method: s(method),
                    addresses: s(addresses),
                    gateway: s(gateway),
                    dns: s(dns),
                },
            },
        );
    }

    fn import_wireguard(self: Pin<&mut Self>, url: &QString, all_users: bool) {
        match local_path(&url.to_string()) {
            Some(path) => model_core::start(
                self,
                Op::ImportWireGuard {
                    path: PathBuf::from(path),
                    all_users,
                },
            ),
            None => self.set_error(QString::from("Choose a file on this computer.")),
        }
    }

    fn edit_wireguard(
        self: Pin<&mut Self>,
        uuid: &QString,
        endpoint: &QString,
        allowed_ips: &QString,
        keepalive: i32,
    ) {
        model_core::start(
            self,
            Op::EditWireGuard {
                uuid: s(uuid),
                peer: PeerEdit {
                    endpoint: s(endpoint),
                    allowed_ips: s(allowed_ips),
                    keepalive: u32::try_from(keepalive).unwrap_or(0),
                },
            },
        );
    }

    fn hotspot(self: Pin<&mut Self>, on: bool, name: &QString, password: &QString) {
        model_core::start(
            self,
            Op::Hotspot {
                on,
                name: s(name),
                password: s(password),
            },
        );
    }

    fn answer_secret(self: Pin<&mut Self>, id: i64, values: &QString) {
        let values: HashMap<String, String> =
            serde_json::from_str(&values.to_string()).unwrap_or_default();
        model_core::start(
            self,
            Op::AnswerSecret {
                id: u64::try_from(id).unwrap_or(0),
                values,
            },
        );
    }

    fn cancel_secret(self: Pin<&mut Self>, id: i64) {
        model_core::start(
            self,
            Op::CancelSecret {
                id: u64::try_from(id).unwrap_or(0),
            },
        );
    }
}
