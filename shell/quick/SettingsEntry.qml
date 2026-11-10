import QtQuick
import QtQuick.Layouts
import Quickshell
import "../core"

// The "Settings…" button at the bottom of quick settings: runs `uqctl settings` (which raises
// uq-settings, or starts it) with a fixed argv, then closes the panel.
Rectangle {
    id: root

    function open() {
        Quickshell.execDetached(["uqctl", "settings"]);
        Surfaces.close();
    }

    Layout.fillWidth: true
    implicitHeight: 32
    radius: Theme.radius - 2
    color: mouse.containsMouse ? Theme.overlay : Theme.surfaceAlt
    activeFocusOnTab: true
    border.color: Theme.accent
    border.width: activeFocus ? 1 : 0
    Accessible.role: Accessible.Button
    Accessible.name: "Settings"
    Accessible.onPressAction: root.open()
    Keys.onSpacePressed: root.open()
    Keys.onReturnPressed: root.open()

    UqText {
        anchors.fill: parent
        anchors.leftMargin: Theme.gapIn * 2
        anchors.rightMargin: Theme.gapIn * 2
        verticalAlignment: Text.AlignVCenter
        text: "Settings…"
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        onClicked: root.open()
    }
}
