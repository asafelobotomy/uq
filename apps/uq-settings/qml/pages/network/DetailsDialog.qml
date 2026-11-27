// Network's connection details dialog: addresses, DNS, metered, auto-connect and who may use it.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Kirigami.Dialog {
    id: detailsDialog
    // Passed in from NetworkPage.qml.
    property var forgetDialog
    property var nm
    property var page

    property string uuid: ""
    readonly property var c: page.saved.find(x => x.uuid === detailsDialog.uuid) || null
    readonly property var methods: ["auto", "manual", "disabled"]
    readonly property var metered: ["auto", "yes", "no"]
    title: detailsDialog.c ? String(detailsDialog.c.name) : ""
    standardButtons: Kirigami.Dialog.Close
    preferredWidth: Kirigami.Units.gridUnit * 30
    onOpened: {
        if (!detailsDialog.c)
            return;
        ipv4Method.currentIndex = Math.max(0, detailsDialog.methods.indexOf(detailsDialog.c.ipv4.method));
        ipv4Addresses.text = String(detailsDialog.c.ipv4.addresses);
        ipv4Gateway.text = String(detailsDialog.c.ipv4.gateway);
        ipv4Dns.text = String(detailsDialog.c.ipv4.dns);
        ipv6Method.currentIndex = Math.max(0, detailsDialog.methods.indexOf(detailsDialog.c.ipv6.method));
        ipv6Addresses.text = String(detailsDialog.c.ipv6.addresses);
        ipv6Gateway.text = String(detailsDialog.c.ipv6.gateway);
        ipv6Dns.text = String(detailsDialog.c.ipv6.dns);
        if (detailsDialog.c.wireguard) {
            wgEndpoint.text = String(detailsDialog.c.wireguard.endpoint);
            wgAllowed.text = String(detailsDialog.c.wireguard.allowed_ips);
            wgKeepalive.value = detailsDialog.c.wireguard.keepalive;
        }
    }
    customFooterActions: [
        Kirigami.Action {
            text: "Forget…"
            icon.name: "edit-delete"
            enabled: page.idle && detailsDialog.c !== null
            onTriggered: {
                forgetDialog.uuid = detailsDialog.uuid;
                forgetDialog.name = detailsDialog.c ? String(detailsDialog.c.name) : "";
                detailsDialog.close();
                forgetDialog.open();
            }
        }
    ]
    ColumnLayout {
        visible: detailsDialog.c !== null
        spacing: Kirigami.Units.smallSpacing

        UqLabel {
            text: detailsDialog.c ? page.kindText(detailsDialog.c.kind) + (detailsDialog.c.state !== "off" ? " · " + page.stateText(detailsDialog.c.state) : "") : ""
            opacity: 0.7
        }
        QQC2.Switch {
            id: autoSwitch
            text: "Connect automatically"
            checked: detailsDialog.c ? !!detailsDialog.c.autoconnect : false
            enabled: page.idle
            onToggled: {
                nm.setAutoconnect(detailsDialog.uuid, autoSwitch.checked);
                autoSwitch.checked = Qt.binding(() => detailsDialog.c ? !!detailsDialog.c.autoconnect : false);
            }
        }
        QQC2.Switch {
            id: everyoneSwitch
            text: "Everyone who uses this computer can use it"
            checked: detailsDialog.c ? !!detailsDialog.c.everyone : false
            enabled: page.idle
            onToggled: {
                nm.setEveryone(detailsDialog.uuid, everyoneSwitch.checked);
                everyoneSwitch.checked = Qt.binding(() => detailsDialog.c ? !!detailsDialog.c.everyone : false);
            }
        }
        RowLayout {
            UqLabel {
                text: "Metered (limit data use)"
            }
            HelpTip {
                tip: "A metered connection tells apps to hold back big downloads and updates."
            }
            Item {
                Layout.fillWidth: true
            }
            UqComboBox {
                id: meteredCombo
                model: ["Decide automatically", "Yes", "No"]
                currentIndex: detailsDialog.c ? Math.max(0, detailsDialog.metered.indexOf(detailsDialog.c.metered)) : 0
                enabled: page.idle
                Accessible.name: "Metered connection"
                onActivated: index => {
                    nm.setMetered(detailsDialog.uuid, detailsDialog.metered[index]);
                    meteredCombo.currentIndex = Qt.binding(() => detailsDialog.c ? Math.max(0, detailsDialog.metered.indexOf(detailsDialog.c.metered)) : 0);
                }
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }
        RowLayout {
            UqLabel {
                text: "IPv4 addresses and DNS"
                font.bold: true
            }
            HelpTip {
                tip: "DNS servers you list here are used instead of the network's own."
            }
        }
        GridLayout {
            columns: 2
            UqLabel {
                text: "Addresses"
            }
            UqComboBox {
                id: ipv4Method
                model: ["Automatic", "Manual", "Off"]
                Accessible.name: "IPv4 addresses"
                Layout.fillWidth: true
            }
            UqLabel {
                visible: ipv4Method.currentIndex === 1
                text: "Address/size"
            }
            QQC2.TextField {
                id: ipv4Addresses
                visible: ipv4Method.currentIndex === 1
                placeholderText: "192.168.1.20/24"
                Accessible.name: "IPv4 addresses, comma-separated"
                Layout.fillWidth: true
            }
            UqLabel {
                visible: ipv4Method.currentIndex === 1
                text: "Gateway"
            }
            QQC2.TextField {
                id: ipv4Gateway
                visible: ipv4Method.currentIndex === 1
                Accessible.name: "IPv4 gateway"
                Layout.fillWidth: true
            }
            UqLabel {
                text: "DNS servers"
            }
            QQC2.TextField {
                id: ipv4Dns
                placeholderText: "Automatic"
                Accessible.name: "IPv4 DNS servers, comma-separated"
                Layout.fillWidth: true
            }
        }
        QQC2.Button {
            text: "Save IPv4 settings"
            icon.name: "document-save"
            enabled: page.idle
            onClicked: nm.setIp(detailsDialog.uuid, 4, detailsDialog.methods[ipv4Method.currentIndex], ipv4Addresses.text, ipv4Gateway.text, ipv4Dns.text)
        }
        UqLabel {
            text: "IPv6 addresses and DNS"
            font.bold: true
        }
        GridLayout {
            columns: 2
            UqLabel {
                text: "Addresses"
            }
            UqComboBox {
                id: ipv6Method
                model: ["Automatic", "Manual", "Off"]
                Accessible.name: "IPv6 addresses"
                Layout.fillWidth: true
            }
            UqLabel {
                visible: ipv6Method.currentIndex === 1
                text: "Address/size"
            }
            QQC2.TextField {
                id: ipv6Addresses
                visible: ipv6Method.currentIndex === 1
                placeholderText: "fd00::20/64"
                Accessible.name: "IPv6 addresses, comma-separated"
                Layout.fillWidth: true
            }
            UqLabel {
                visible: ipv6Method.currentIndex === 1
                text: "Gateway"
            }
            QQC2.TextField {
                id: ipv6Gateway
                visible: ipv6Method.currentIndex === 1
                Accessible.name: "IPv6 gateway"
                Layout.fillWidth: true
            }
            UqLabel {
                text: "DNS servers"
            }
            QQC2.TextField {
                id: ipv6Dns
                placeholderText: "Automatic"
                Accessible.name: "IPv6 DNS servers, comma-separated"
                Layout.fillWidth: true
            }
        }
        QQC2.Button {
            text: "Save IPv6 settings"
            icon.name: "document-save"
            enabled: page.idle
            onClicked: nm.setIp(detailsDialog.uuid, 6, detailsDialog.methods[ipv6Method.currentIndex], ipv6Addresses.text, ipv6Gateway.text, ipv6Dns.text)
        }

        ColumnLayout {
            visible: detailsDialog.c !== null && detailsDialog.c.kind === "wireguard"
            Layout.fillWidth: true
            Kirigami.Separator {
                Layout.fillWidth: true
            }
            UqLabel {
                text: "VPN server"
                font.bold: true
            }
            GridLayout {
                columns: 2
                UqLabel {
                    text: "Server address"
                }
                QQC2.TextField {
                    id: wgEndpoint
                    placeholderText: "vpn.example.com:51820"
                    Accessible.name: "VPN server address and port"
                    Layout.fillWidth: true
                }
                UqLabel {
                    text: "Send through the VPN"
                }
                QQC2.TextField {
                    id: wgAllowed
                    placeholderText: "0.0.0.0/0, ::/0"
                    Accessible.name: "Address ranges sent through the VPN, comma-separated"
                    Layout.fillWidth: true
                }
                UqLabel {
                    text: "Keep alive every (seconds, 0 for off)"
                }
                QQC2.SpinBox {
                    id: wgKeepalive
                    from: 0
                    to: 65535
                    Accessible.name: "Keep-alive interval in seconds"
                }
            }
            QQC2.Button {
                text: "Save VPN server"
                icon.name: "document-save"
                enabled: page.idle
                onClicked: nm.editWireguard(detailsDialog.uuid, wgEndpoint.text, wgAllowed.text, wgKeepalive.value)
            }
        }
    }
}
