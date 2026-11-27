// Network's Wi-Fi tab: networks in range, the one in use, and joining a hidden one.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    id: wifiTab
    // Passed in from NetworkPage.qml.
    property var hiddenDialog
    property var nm
    property var page

    title: "Wi-Fi"
    iconName: "network-wireless"
    available: page.hasWifi

    SettingsCard {
        title: "Networks"
        iconName: "network-wireless"
        wide: true

        SettingRow {
            visible: page.wifi.enabled !== true || page.networks.length === 0
            iconName: page.wifi.enabled === true ? "network-wireless-acquiring" : "network-wireless-disconnected"
            label: page.wifi.enabled !== true ? "Wi-Fi is off." : (page.wifi.scanned ? "No networks found." : "Looking for networks…")
            opacity: 0.8
            Layout.fillWidth: true

            QQC2.Button {
                text: "Turn on"
                icon.name: "network-wireless"
                visible: page.wifi.enabled !== true
                enabled: page.idle && page.wifi.hardware !== false
                onClicked: nm.setWifi(true)
            }
        }
        GridLayout {
            columns: page.width >= Kirigami.Units.gridUnit * 70 ? 2 : 1
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.smallSpacing
            uniformCellWidths: true
            visible: page.wifi.enabled === true
            Layout.fillWidth: true

            Repeater {
                model: page.wifi.enabled === true ? page.networks : []
                delegate: NetworkTile {
                    page: wifiTab.page
                    nm: wifiTab.nm
                }
            }
        }
        Kirigami.Separator {
            visible: page.wifi.enabled === true
            Layout.fillWidth: true
        }
        SettingRow {
            settingId: "hidden-network"
            iconName: "view-hidden"
            label: "Hidden network"
            help: "A network that doesn't show its name. You need its exact name to join."
            highlight: App.highlight
            visible: page.wifi.enabled === true
            Layout.fillWidth: true

            QQC2.Button {
                text: "Join…"
                icon.name: "list-add"
                enabled: page.idle
                Accessible.name: "Join a hidden network"
                onClicked: hiddenDialog.open()
            }
        }
    }
}
