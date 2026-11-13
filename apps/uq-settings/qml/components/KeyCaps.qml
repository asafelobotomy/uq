// A key combination drawn as keycaps: "Super + Shift + Q" as three small raised keys. The words
// come from uqd (or user.lua), so each key is a UqLabel (plain text).
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

RowLayout {
    id: caps

    /// The keys in words, joined by " + ", as uqd gives `combo_text`.
    property string combo: ""
    readonly property var keys: caps.combo === "" ? [] : caps.combo.split(" + ")

    spacing: Kirigami.Units.smallSpacing / 2
    Accessible.role: Accessible.StaticText
    Accessible.name: caps.combo

    Repeater {
        model: caps.keys
        delegate: Rectangle {
            id: cap

            required property string modelData

            objectName: "keycap"
            implicitWidth: Math.max(capLabel.implicitWidth + Kirigami.Units.largeSpacing, implicitHeight)
            implicitHeight: capLabel.implicitHeight + Kirigami.Units.smallSpacing * 2
            radius: Kirigami.Units.cornerRadius
            color: Kirigami.Theme.backgroundColor
            border.width: 1
            border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.3)

            // The key's lower edge, a little darker, so it reads as a key.
            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.margins: 1
                height: 2
                radius: Kirigami.Units.cornerRadius
                color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.15)
            }
            UqLabel {
                id: capLabel
                objectName: "keycapText"
                anchors.centerIn: parent
                anchors.verticalCenterOffset: -1
                text: cap.modelData
                font.weight: Font.DemiBold
                Accessible.ignored: true
            }
        }
    }
}
