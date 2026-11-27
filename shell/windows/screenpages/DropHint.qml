pragma ComponentBehavior: Bound
// While a tab is dragged over a screen: where its window would go, drawn as a window frame over that part of the page (nothing where it can't go).
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import "../../core"
import "../../services"
import "../../wallpaper"
import "../../logic/edges.js" as Edges
import "../../logic/mousehold.js" as Hold
import "../../logic/pages.js" as Pages
import "../../logic/work.js" as Work

PanelWindow {
    id: hintWin
    // Passed in from ScreenPages.qml.
    property var root

    required property var modelData
    readonly property var hint: PageActions.dropHint
    readonly property var place: {
        var h = hintWin.hint;
        if (!h || h.screen !== hintWin.modelData.name || h.drop !== true)
            return null;
        var d = root.dropPlanFor(h.screen, h.address, h.x);
        if (d === null)
            return null;
        var m = d.m, res = m.reserved && m.reserved.length >= 4 ? m.reserved : [0, 0, 0, 0];
        return { x: res[0] + WindowLook.gapLeft + d.plan.area.from + WindowLook.windowGap / 2,
                 y: res[1] + WindowLook.gapTop,
                 w: d.plan.area.width - WindowLook.windowGap,
                 h: hintWin.modelData.height - res[1] - res[3] - WindowLook.gapTop - WindowLook.gapBottom };
    }

    screen: modelData
    visible: hint !== null && hint.screen === modelData.name
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "uq-tab-drop"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Ignore
    anchors { top: true; bottom: true; left: true; right: true }
    // Draws only: the dragged tab keeps the pointer.
    mask: Region {}

    Rectangle {
        visible: hintWin.place !== null
        x: hintWin.place ? hintWin.place.x : 0
        y: hintWin.place ? hintWin.place.y : 0
        width: hintWin.place ? hintWin.place.w : 0
        height: hintWin.place ? hintWin.place.h : 0
        radius: WindowLook.frameRadius
        color: Qt.alpha(WindowLook.activeBorder, 0.18)
        border.width: Math.max(2, WindowLook.border)
        border.color: WindowLook.activeBorder
    }

    // The tab, see-through, held by the pointer where it was taken.
    Rectangle {
        id: ghost
        readonly property var h: hintWin.hint
        // Not for a window moved with the pointer: it is the window itself.
        visible: h !== null && h.ghost !== false
        x: h ? h.x - h.holdX : 0
        y: h ? h.y - h.holdY : 0
        width: h ? Math.max(h.width, Theme.fontSize * 9) : 0
        height: h ? h.height : 0
        radius: Theme.radiusControl
        color: Theme.surfaceAlt
        border.width: 1
        border.color: h && h.drop ? WindowLook.activeBorder : Theme.border
        opacity: 0.8
        Row {
            anchors.fill: parent
            anchors.leftMargin: Theme.gapIn
            anchors.rightMargin: Theme.gapIn
            spacing: Theme.gapIn
            Image {
                anchors.verticalCenter: parent.verticalCenter
                width: Math.round(ghost.height * 0.6)
                height: width
                sourceSize.width: width * 2
                sourceSize.height: height * 2
                source: ghost.h ? ghost.h.icon : ""
            }
            UqText {
                anchors.verticalCenter: parent.verticalCenter
                width: ghost.width - ghost.height - Theme.gapIn * 3
                text: ghost.h ? ghost.h.title : ""
                elide: Text.ElideRight
            }
        }
    }
}
