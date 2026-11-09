pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import "../../core"
import ".."

// The built-in server's popups (Builtin.popups, newest first) in a column at the top right of
// the focused monitor, under the bar. Never takes keyboard focus, and the window's input mask
// covers only the cards, so clicks beside them reach the windows below. Hidden while locked.
PanelWindow {
    id: root

    readonly property int panelWidth: 380
    // Where the bar puts its islands: Bar.qml's top margin plus its island height.
    readonly property int barBottom: Theme.gapOut + 36

    function screenFor(monitor) {
        if (!monitor)
            return null;
        var list = Quickshell.screens;
        for (var i = 0; i < list.length; i++)
            if (list[i].name === monitor.name)
                return list[i];
        return null;
    }

    visible: !Bridge.locked && Builtin.popups.length > 0
    screen: screenFor(Hyprland.focusedMonitor)
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.namespace: "uq-notif"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Ignore
    anchors.top: true
    anchors.right: true
    margins.top: barBottom + Theme.gapIn
    margins.right: Theme.gapOut
    implicitWidth: panelWidth
    implicitHeight: Math.max(1, column.implicitHeight)
    mask: Region { item: column }

    Column {
        id: column
        width: root.panelWidth
        spacing: Theme.gapIn * 2

        Repeater {
            model: Builtin.popups
            delegate: NotifCard {
                required property var modelData
                notification: modelData
                width: column.width
                dpr: root.screen ? root.screen.devicePixelRatio : 1
            }
        }
    }
}
