import QtQuick
import Quickshell
import Quickshell.Widgets
import "../../core"
import "../../logic/net.js" as Net

// One app's tray icon (a StatusNotifierItem), as the Tray island and the Status island draw it:
// left click activates the app (or opens its menu when it only has a menu), right click opens
// its menu, middle click is its secondary action, the wheel scrolls it. Hovering shows its
// tooltip title, which is untrusted and only reaches UqText.
Item {
    id: trayItem
    required property var modelData
    property int cell: 26
    property int iconSize: 18

    implicitWidth: cell
    implicitHeight: cell
    Accessible.role: Accessible.Button
    Accessible.name: modelData.tooltipTitle || modelData.title || modelData.id || "Tray icon"
    Accessible.onPressAction: modelData.activate()

    function openMenu() {
        if (trayItem.modelData.hasMenu)
            trayMenu.open();
    }

    IconImage {
        anchors.centerIn: parent
        implicitSize: trayItem.iconSize
        width: trayItem.iconSize
        height: trayItem.iconSize
        source: Net.safeIconSource(trayItem.modelData.icon,
            Quickshell.iconPath("application-x-executable", true))
    }
    QsMenuAnchor {
        id: trayMenu
        menu: trayItem.modelData.menu
        anchor.item: trayItem
        anchor.edges: Edges.Bottom
        anchor.gravity: Edges.Bottom
    }
    MouseArea {
        id: trayMouse
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        onClicked: function (mouse) {
            if (mouse.button === Qt.RightButton || trayItem.modelData.onlyMenu)
                trayItem.openMenu();
            else if (mouse.button === Qt.MiddleButton)
                trayItem.modelData.secondaryActivate();
            else
                trayItem.modelData.activate();
        }
        onWheel: function (w) {
            trayItem.modelData.scroll(w.angleDelta.y, false);
        }
    }
    PopupWindow {
        anchor.item: trayItem
        anchor.edges: Edges.Bottom
        anchor.gravity: Edges.Bottom
        implicitWidth: tipText.implicitWidth + Theme.gapIn * 2
        implicitHeight: tipText.implicitHeight + Theme.gapIn * 2
        color: "transparent"
        visible: trayMouse.containsMouse && !trayMenu.visible && tipText.text !== ""
        Rectangle {
            anchors.fill: parent
            color: Theme.surface
            radius: Theme.radius
            border.color: Theme.border
            border.width: Theme.borderWidth
            UqText {
                id: tipText
                anchors.centerIn: parent
                width: Math.min(implicitWidth, 260)
                text: trayItem.modelData.tooltipTitle || trayItem.modelData.title || ""
            }
        }
    }
}
