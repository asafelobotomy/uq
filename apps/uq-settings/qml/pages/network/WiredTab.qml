// Network's Wired tab: each cable connection.
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

    title: "Wired"
    iconName: "network-wired"
    available: page.ready && page.wired.length > 0

    Repeater {
        model: page.ready ? page.wired : []
        delegate: SettingsCard {
            id: wiredCard
            required property var modelData
            required property int index
            title: String(modelData.name)
            iconName: "network-wired"
            border.width: modelData.connected ? 2 : 1
            border.color: modelData.connected ? Kirigami.Theme.highlightColor : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.08)

            actions: [
                IconButton {
                    icon.name: "configure"
                    tip: "Details"
                    visible: !!wiredCard.modelData.uuid
                    enabled: page.idle
                    onClicked: page.details({
                        uuid: wiredCard.modelData.uuid
                    })
                }
            ]

            SettingRow {
                settingId: wiredCard.index === 0 ? "wired" : ""
                highlight: App.highlight
                iconName: wiredCard.modelData.connected ? "network-connect" : "network-disconnect"
                label: String(wiredCard.modelData.state)
                Layout.fillWidth: true

                TipIcon {
                    visible: String(wiredCard.modelData.speed) !== ""
                    source: "speedometer"
                    tip: "Speed"
                }
                UqLabel {
                    visible: String(wiredCard.modelData.speed) !== ""
                    text: String(wiredCard.modelData.speed)
                }
                QQC2.Button {
                    text: "Disconnect"
                    icon.name: "network-disconnect"
                    visible: !!wiredCard.modelData.uuid && wiredCard.modelData.connected
                    enabled: page.idle
                    onClicked: nm.deactivate(String(wiredCard.modelData.uuid))
                }
            }
        }
    }
}
