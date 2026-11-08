import QtQuick
import "../core"

// A rounded floating panel; children go into the default content slot.
Rectangle {
    id: root
    default property alias content: slot.data

    color: Theme.surface
    radius: Theme.radius
    border.color: Theme.border
    border.width: Theme.borderWidth
    implicitHeight: 36
    implicitWidth: slot.childrenRect.width + Theme.gapIn * 2

    Item {
        id: slot
        anchors.fill: parent
        anchors.margins: Theme.gapIn
    }
}
