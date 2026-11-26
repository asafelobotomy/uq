// A tab's small round button: Half, Full or close. The marks are drawn, not font glyphs (whose
// sizes and baselines differ from font to font), all in the same box, so they sit level.
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
    id: tb
    // Passed in from WorkIsland.qml.
    property var root

    // "half", "full" or "close".
    property string mark: "close"
    property string label: ""
    // The window is already this way (Full on a Full window): drawn as on.
    property bool on: false
    signal pressed()
    implicitWidth: root.closeW
    implicitHeight: root.closeW
    radius: width / 2
    color: tbMouse.containsMouse ? Theme.overlay : "transparent"
    Accessible.role: Accessible.Button
    Accessible.name: label
    Accessible.onPressAction: tb.pressed()
    // The way the window is now (Full or Half) in the accent colour.
    readonly property color ink: tb.on ? Theme.accent : (tbMouse.containsMouse ? Theme.text : Theme.textMuted)
    readonly property real box: Math.round(root.closeW * 0.46)
    readonly property real line: Math.max(1, Math.round(root.closeW / 14))
    Item {
        width: tb.box
        height: tb.box
        anchors.centerIn: parent
        // Half and Full: a window's outline, Half with its left half filled.
        Rectangle {
            visible: tb.mark !== "close"
            anchors.fill: parent
            radius: tb.line
            color: "transparent"
            border.width: tb.line
            border.color: tb.ink
        }
        Rectangle {
            visible: tb.mark === "half"
            width: Math.round(parent.width / 2)
            height: parent.height
            radius: tb.line
            color: tb.ink
        }
        // Close: two crossed strokes.
        Repeater {
            model: tb.mark === "close" ? [45, -45] : []
            delegate: Rectangle {
                required property real modelData
                anchors.centerIn: parent
                width: parent.width * 1.2
                height: tb.line * 1.5
                radius: height / 2
                rotation: modelData
                color: tb.ink
            }
        }
    }
    MouseArea {
        id: tbMouse
        anchors.fill: parent
        hoverEnabled: true
        onClicked: tb.pressed()
    }
}
