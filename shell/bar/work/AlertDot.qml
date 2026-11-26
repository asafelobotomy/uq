// The dot on a window's icon when it wants attention: the theme's accent, ringed in the
// island's colour so it stands off any icon. Sits on its parent's top right corner.
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
    // Passed in from WorkIsland.qml.
    property var root

    property bool shown: false
    visible: shown
    width: Math.round(root.iconSize * 0.45)
    height: width
    radius: width / 2
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.rightMargin: -Math.round(width / 4)
    anchors.topMargin: -Math.round(width / 4)
    color: Theme.accent
    border.width: Math.max(1, Math.round(width / 5))
    border.color: Theme.surface
    z: 1
}
