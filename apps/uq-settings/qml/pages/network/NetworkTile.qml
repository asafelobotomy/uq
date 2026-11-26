// One Wi-Fi network: signal, lock, name, band, and what can be done with it.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Rectangle {
    id: tile
    // The page and its NetworkModel (NetworkPage.qml's `page` and `nm`).
    property var page
    property var nm
    required property var modelData
    readonly property var n: modelData
    readonly property bool on: tile.n.state === "on"

    implicitHeight: tileRow.implicitHeight + Kirigami.Units.smallSpacing * 2
    implicitWidth: tileRow.implicitWidth + Kirigami.Units.largeSpacing * 2
    Layout.fillWidth: true
    radius: Kirigami.Units.cornerRadius
    color: tile.on ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15) : "transparent"
    border.width: tile.on ? 1 : 0
    border.color: Kirigami.Theme.highlightColor
    Accessible.role: Accessible.ListItem
    Accessible.name: String(tile.n.ssid) + ", " + page.securityText(tile.n.security) + ", signal " + tile.n.strength + "%" + (tile.on ? ", connected" : "")

    RowLayout {
        id: tileRow
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.largeSpacing
        anchors.rightMargin: Kirigami.Units.smallSpacing
        spacing: Kirigami.Units.smallSpacing

        TipIcon {
            source: ["network-wireless-signal-none", "network-wireless-signal-weak", "network-wireless-signal-ok", "network-wireless-signal-good", "network-wireless-signal-excellent"][tile.n.bars] || "network-wireless"
            tip: "Signal " + tile.n.strength + "%"
            implicitWidth: Kirigami.Units.iconSizes.smallMedium
            implicitHeight: Kirigami.Units.iconSizes.smallMedium
        }
        TipIcon {
            source: page.securityIcon(tile.n.security)
            tip: page.securityText(tile.n.security)
            opacity: tile.n.security === "open" ? 0.5 : 1
        }
        UqLabel {
            text: String(tile.n.ssid)
            font.bold: tile.on
            elide: Text.ElideRight
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.smallSpacing
        }
        UqLabel {
            visible: tile.n.state === "starting" || tile.n.state === "stopping"
            text: page.stateText(tile.n.state)
            opacity: 0.7
        }
        TipIcon {
            visible: !!tile.n.saved
            source: "bookmarks"
            tip: "Saved"
            opacity: 0.7
        }
        Chip {
            visible: String(tile.n.band) !== ""
            label: String(tile.n.band)
        }
        QQC2.Button {
            text: "Connect"
            icon.name: "network-connect"
            visible: tile.n.state === "off"
            enabled: page.idle
            onClicked: page.join(tile.n)
        }
        QQC2.Button {
            text: "Disconnect"
            icon.name: "network-disconnect"
            visible: tile.n.state !== "off" && !!tile.n.saved
            enabled: page.idle
            onClicked: nm.deactivate(String(tile.n.saved))
        }
        IconButton {
            icon.name: "configure"
            tip: "Details"
            enabled: page.idle && !!tile.n.saved
            opacity: tile.n.saved ? 1 : 0
            onClicked: page.details({
                uuid: tile.n.saved
            })
        }
    }
}
