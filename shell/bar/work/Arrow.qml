import QtQuick
import QtQuick.Effects
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import "../../core"
import "../../services"
import "../../notif"
import "../islands"
import "../../logic/minimise.js" as Min
import "../../logic/work.js" as Work
import "../../logic/dock.js" as Dock

Rectangle {
    id: arrow
    // Passed in from WorkIsland.qml.
    property var root

    property string glyph: ""
    property string label: ""
    property bool can: true
    signal pressed()
    Layout.preferredWidth: root.metrics.arrow
    Layout.fillHeight: true
    radius: Theme.radius - 2
    opacity: can ? 1 : 0.35
    color: arrowMouse.containsMouse && can ? Theme.surfaceAlt : "transparent"
    Accessible.role: Accessible.Button
    Accessible.name: label
    UqText {
        anchors.centerIn: parent
        text: arrow.glyph
    }
    MouseArea {
        id: arrowMouse
        anchors.fill: parent
        hoverEnabled: true
        enabled: arrow.can
        onClicked: arrow.pressed()
    }
}
