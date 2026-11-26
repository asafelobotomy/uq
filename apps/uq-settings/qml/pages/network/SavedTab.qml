// Network's Saved tab: every saved connection, with details and forget.
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

    title: "Saved"
    iconName: "bookmarks"
    available: page.ready

    // The connection in use now: its data use, and the way to its addresses (search for
    // "metered" or "DNS" lands here).
    SettingsCard {
        title: page.current ? String(page.current.name) : "Not connected"
        iconName: page.current ? page.kindIcon(page.current.kind) : "network-disconnect"
        help: "Each saved connection has its own metered and address settings, under Details."
        actions: [
            IconButton {
                icon.name: "configure"
                tip: "Details"
                visible: page.current !== null
                enabled: page.idle
                onClicked: page.details(page.current)
            }
        ]

        SettingRow {
            objectName: "meteredRow"
            settingId: "metered"
            iconName: "network-limited"
            label: "Metered"
            help: "Tells apps to hold back big downloads and updates on this connection. Details holds it for every connection."
            highlight: App.highlight
            enabled: page.idle && page.current !== null
            Layout.fillWidth: true

            UqComboBox {
                id: currentMetered
                readonly property var values: ["auto", "yes", "no"]
                readonly property int shown: page.current ? Math.max(0, values.indexOf(page.current.metered)) : 0
                model: ["Decide automatically", "Yes", "No"]
                currentIndex: currentMetered.shown
                Accessible.name: "Metered connection"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 11
                onActivated: index => {
                    nm.setMetered(String(page.current.uuid), currentMetered.values[index]);
                    currentMetered.currentIndex = Qt.binding(() => currentMetered.shown);
                }
            }
        }
        SettingRow {
            objectName: "ipSettingsRow"
            settingId: "ip-settings"
            iconName: "network-server"
            label: "Addresses and DNS"
            help: "Details holds a connection's addresses, gateway and DNS servers."
            highlight: App.highlight
            Layout.fillWidth: true

            UqLabel {
                visible: page.current !== null
                text: page.current && page.current.ipv4 ? page.methodText(page.current.ipv4.method) : ""
                opacity: 0.8
            }
            QQC2.Button {
                text: "Details…"
                icon.name: "configure"
                enabled: page.idle && page.current !== null
                onClicked: page.details(page.current)
            }
        }
    }

    SettingsCard {
        title: "Saved connections"
        iconName: "bookmarks"
        help: "Passwords you type here go straight to the network service, which keeps them for the connection. Changes may ask for an administrator password."

        SettingRow {
            visible: page.others.length === 0
            settingId: page.others.length === 0 ? "saved-connections" : ""
            highlight: App.highlight
            iconName: "bookmarks"
            label: "None yet."
            opacity: 0.8
            Layout.fillWidth: true
        }
        Repeater {
            model: page.others
            delegate: SettingRow {
                id: savedRow
                required property var modelData
                required property int index
                readonly property var c: modelData
                settingId: index === 0 ? "saved-connections" : ""
                highlight: App.highlight
                iconName: page.kindIcon(savedRow.c.kind)
                label: String(savedRow.c.name)
                Layout.fillWidth: true

                UqLabel {
                    visible: savedRow.c.state !== "off"
                    text: page.stateText(savedRow.c.state)
                    opacity: 0.7
                }
                TipIcon {
                    source: "chronometer"
                    tip: savedRow.c.autoconnect ? "Connects automatically" : "Not automatic"
                    opacity: savedRow.c.autoconnect ? 0.8 : 0.25
                }
                TipIcon {
                    source: savedRow.c.everyone ? "system-users" : "user"
                    tip: savedRow.c.everyone ? "Everyone who uses this computer" : "Only you"
                    opacity: 0.8
                }
                IconButton {
                    icon.name: "configure"
                    tip: "Details"
                    enabled: page.idle
                    onClicked: page.details(savedRow.c)
                }
            }
        }
    }
}
