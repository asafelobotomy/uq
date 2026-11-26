// Network's dialog for joining a work (enterprise, 802.1X) network.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Kirigami.Dialog {
    id: enterpriseDialog
    // Passed in from NetworkPage.qml.
    property var certDialog
    property var nm
    property var page

    property var network: ({})
    readonly property var methods: ["peap", "ttls", "tls"]
    readonly property var phase2: ["mschapv2", "pap", "gtc", "mschap", "chap"]
    readonly property string method: methods[eapMethod.currentIndex] || "peap"
    property string caCert: ""
    property string clientCert: ""
    property string privateKey: ""
    property string picking: ""
    title: "Join " + String(enterpriseDialog.network.ssid || "")
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 28
    onOpened: {
        eapIdentity.clear();
        eapAnonymous.clear();
        eapDomain.clear();
        enterpriseDialog.caCert = "";
        enterpriseDialog.clientCert = "";
        enterpriseDialog.privateKey = "";
        eapEveryone.checked = false;
        eapIdentity.forceActiveFocus();
    }
    onClosed: {
        eapPassword.wipe();
        eapKeyPassword.wipe();
    }
    customFooterActions: [
        Kirigami.Action {
            text: "Connect"
            icon.name: "network-connect"
            enabled: page.idle && eapIdentity.text.trim() !== "" && (enterpriseDialog.method === "tls" ? enterpriseDialog.clientCert !== "" && enterpriseDialog.privateKey !== "" : eapPassword.text !== "")
            onTriggered: {
                nm.connectEnterprise(String(enterpriseDialog.network.ap), String(enterpriseDialog.network.device), JSON.stringify({
                    method: enterpriseDialog.method,
                    identity: eapIdentity.text,
                    anonymous_identity: eapAnonymous.text,
                    password: eapPassword.text,
                    phase2: enterpriseDialog.phase2[eapPhase2.currentIndex] || "mschapv2",
                    ca_cert: enterpriseDialog.caCert,
                    domain: eapDomain.text,
                    client_cert: enterpriseDialog.clientCert,
                    private_key: enterpriseDialog.privateKey,
                    key_password: eapKeyPassword.text
                }), eapEveryone.checked);
                enterpriseDialog.close();
            }
        }
    ]
    GridLayout {
        columns: 2
        UqLabel {
            text: "Sign-in method"
        }
        UqComboBox {
            id: eapMethod
            model: ["Password, protected (PEAP)", "Password, tunnelled (TTLS)", "Certificate (TLS)"]
            Accessible.name: "Sign-in method"
            Layout.fillWidth: true
        }
        UqLabel {
            text: "User name"
        }
        QQC2.TextField {
            id: eapIdentity
            Accessible.name: "User name"
            Layout.fillWidth: true
        }
        UqLabel {
            text: "Outer name (optional)"
        }
        QQC2.TextField {
            id: eapAnonymous
            Accessible.name: "Outer name, sent before the protected sign-in"
            Layout.fillWidth: true
        }
        UqLabel {
            visible: enterpriseDialog.method !== "tls"
            text: "Password"
        }
        SecretField {
            id: eapPassword
            visible: enterpriseDialog.method !== "tls"
            Accessible.name: "Password"
            Layout.fillWidth: true
        }
        UqLabel {
            visible: enterpriseDialog.method !== "tls"
            text: "Inner method"
        }
        UqComboBox {
            id: eapPhase2
            visible: enterpriseDialog.method !== "tls"
            model: ["MSCHAPv2", "PAP", "GTC", "MSCHAP", "CHAP"]
            Accessible.name: "Inner sign-in method"
            Layout.fillWidth: true
        }
        UqLabel {
            text: "Network's certificate"
        }
        RowLayout {
            UqLabel {
                text: enterpriseDialog.caCert === "" ? "The system's certificates" : decodeURIComponent(enterpriseDialog.caCert.replace(/^.*\//, ""))
                elide: Text.ElideMiddle
                Layout.fillWidth: true
            }
            QQC2.Button {
                text: "Choose…"
                onClicked: {
                    enterpriseDialog.picking = "ca";
                    certDialog.open();
                }
            }
        }
        UqLabel {
            text: "Server name ends with"
        }
        QQC2.TextField {
            id: eapDomain
            placeholderText: "e.g. radius.example.org"
            Accessible.name: "Only trust a server whose name ends with this"
            Layout.fillWidth: true
        }
        UqLabel {
            visible: enterpriseDialog.method === "tls"
            text: "Your certificate"
        }
        RowLayout {
            visible: enterpriseDialog.method === "tls"
            UqLabel {
                text: enterpriseDialog.clientCert === "" ? "None chosen" : decodeURIComponent(enterpriseDialog.clientCert.replace(/^.*\//, ""))
                elide: Text.ElideMiddle
                Layout.fillWidth: true
            }
            QQC2.Button {
                text: "Choose…"
                onClicked: {
                    enterpriseDialog.picking = "client";
                    certDialog.open();
                }
            }
        }
        UqLabel {
            visible: enterpriseDialog.method === "tls"
            text: "Your private key"
        }
        RowLayout {
            visible: enterpriseDialog.method === "tls"
            UqLabel {
                text: enterpriseDialog.privateKey === "" ? "None chosen" : decodeURIComponent(enterpriseDialog.privateKey.replace(/^.*\//, ""))
                elide: Text.ElideMiddle
                Layout.fillWidth: true
            }
            QQC2.Button {
                text: "Choose…"
                onClicked: {
                    enterpriseDialog.picking = "key";
                    certDialog.open();
                }
            }
        }
        UqLabel {
            visible: enterpriseDialog.method === "tls"
            text: "Key password"
        }
        SecretField {
            id: eapKeyPassword
            visible: enterpriseDialog.method === "tls"
            Accessible.name: "Private key password"
            Layout.fillWidth: true
        }
        Item {
            implicitWidth: 1
        }
        QQC2.CheckBox {
            id: eapEveryone
            text: "Everyone who uses this computer can join it too"
        }
    }
}
