// A small rounded tag: a band ("5 GHz") or a speed.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Rectangle {
    id: chip
    property string label: ""
    implicitWidth: chipText.implicitWidth + Kirigami.Units.smallSpacing * 2
    implicitHeight: chipText.implicitHeight + 2
    radius: Kirigami.Units.cornerRadius
    color: "transparent"
    border.width: 1
    border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.3)
    UqLabel {
        id: chipText
        anchors.centerIn: parent
        text: chip.label
        font.pointSize: Kirigami.Theme.smallFont.pointSize
        opacity: 0.8
    }
}
