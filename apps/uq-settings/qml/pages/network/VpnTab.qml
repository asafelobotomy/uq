// Network's VPN tab: VPN and WireGuard connections, and importing one.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    // Passed in from NetworkPage.qml.
    property var nm
    property var page
    property var wireguardFile

    title: "VPN"
    iconName: "network-vpn"
    available: page.ready

    SettingsCard {
        title: "VPN"
        iconName: "network-vpn"
        help: page.snap.openvpn === false ? "For OpenVPN connections, install the networkmanager-openvpn package." : ""
        wide: true
        actions: [
            QQC2.Button {
                objectName: "importWireguard"
                text: "Add a WireGuard file…"
                icon.name: "document-import"
                enabled: page.idle
                onClicked: wireguardFile.open()
            }
        ]

        SettingRow {
            visible: page.vpns.length === 0
            settingId: page.vpns.length === 0 ? "vpn" : ""
            iconName: "network-vpn"
            label: "No VPNs yet."
            help: "Your VPN provider gives you a WireGuard file to add here."
            highlight: App.highlight
            opacity: 0.8
            Layout.fillWidth: true
        }
        Repeater {
            model: page.vpns
            delegate: SwitchRow {
                id: vpnRow
                required property var modelData
                required property int index
                settingId: index === 0 ? "vpn" : ""
                highlight: App.highlight
                iconName: "network-vpn"
                label: String(modelData.name) + (modelData.state === "starting" ? " · Connecting…" : "")
                value: modelData.state === "on" || modelData.state === "starting"
                enabled: page.idle
                onChosen: on => on ? nm.activate(String(vpnRow.modelData.uuid)) : nm.deactivate(String(vpnRow.modelData.uuid))
                Layout.fillWidth: true

                IconButton {
                    icon.name: "configure"
                    tip: "Details"
                    enabled: page.idle
                    onClicked: page.details(vpnRow.modelData)
                }
            }
        }
    }
}
