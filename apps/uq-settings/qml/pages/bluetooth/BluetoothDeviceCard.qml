// One device: its picture and name, battery, connect or pair, and (once paired) whether it
// may connect on its own and Forget.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsCard {
    id: card
    // Passed in from BluetoothPage.qml.
    property var bm
    property var forgetDialog
    property var page

    required property var modelData
    required property int index
    readonly property var d: modelData

    title: String(card.d.name)
    iconName: String(card.d.icon || "") === "" || card.d.icon === "bluetooth" ? "network-bluetooth" : String(card.d.icon)
    border.width: card.d.connected === true ? 2 : 1
    border.color: card.d.connected === true ? Kirigami.Theme.highlightColor : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.08)
    Accessible.description: String(card.d.kind)

    actions: [
        Kirigami.Icon {
            visible: page.hasBattery(card.d)
            source: page.hasBattery(card.d) ? page.batteryIcon(card.d.battery) : ""
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: Kirigami.Units.iconSizes.small
            Accessible.ignored: true
        },
        UqLabel {
            visible: page.hasBattery(card.d)
            text: page.hasBattery(card.d) ? card.d.battery + "%" : ""
            Accessible.name: "Battery " + text
        },
        QQC2.ToolButton {
            icon.name: "edit-delete"
            visible: card.d.paired === true
            enabled: page.idle
            display: QQC2.AbstractButton.IconOnly
            Accessible.name: "Forget " + String(card.d.name)
            QQC2.ToolTip.text: "Forget"
            QQC2.ToolTip.visible: hovered
            onClicked: {
                forgetDialog.path = String(card.d.path);
                forgetDialog.name = String(card.d.name);
                forgetDialog.open();
            }
        }
    ]

    SettingRow {
        settingId: card.d.paired && card.index === 0 ? "bluetooth-devices" : ""
        highlight: App.highlight
        iconName: card.d.connected === true ? "network-connect" : (card.d.working ? "chronometer" : "network-disconnect")
        label: page.status(card.d) !== "" ? page.status(card.d) : (card.d.paired ? "Not connected" : String(card.d.kind))
        Layout.fillWidth: true

        QQC2.BusyIndicator {
            visible: !!card.d.working
            running: visible
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: Kirigami.Units.iconSizes.small
        }
        QQC2.Button {
            text: "Pair"
            icon.name: "network-connect"
            visible: !card.d.paired && !card.d.working
            enabled: page.idle && page.powered
            onClicked: bm.pair(String(card.d.path))
        }
        QQC2.Button {
            text: "Stop"
            icon.name: "process-stop"
            visible: card.d.working === "pairing"
            onClicked: bm.cancelPairing(String(card.d.path))
        }
        QQC2.Button {
            text: "Connect"
            icon.name: "network-connect"
            visible: card.d.paired && !card.d.connected && !card.d.working
            enabled: page.idle && page.powered
            onClicked: bm.connectDevice(String(card.d.path))
        }
        QQC2.Button {
            text: "Disconnect"
            icon.name: "network-disconnect"
            visible: card.d.paired && card.d.connected === true
            enabled: page.idle
            onClicked: bm.disconnectDevice(String(card.d.path))
        }
    }
    SwitchRow {
        visible: card.d.paired === true
        iconName: "security-high"
        label: "Connects without asking"
        help: "It can connect to this computer on its own, without a question each time."
        value: card.d.trusted === true
        enabled: page.idle
        Layout.fillWidth: true
        onChosen: on => bm.setTrusted(String(card.d.path), on)
    }
}
