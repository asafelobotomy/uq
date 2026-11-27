pragma ComponentBehavior: Bound
// Beside a Half window alone on its page, a transparent surface over the hidden next page, so its window can't be clicked by accident (a right click opens the desktop menu).
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
    id: cov
    // Passed in from ScreenPages.qml.
    property var root

    required property var modelData
    readonly property var mon: root.monitors[modelData.name] || null
    readonly property var area: {
        var m = cov.mon;
        if (!root.enabled || !WindowLook.known || !m || !m.activeWorkspace)
            return null;
        var wins = root.tiled[m.activeWorkspace.id], g = Pages.geometry(m, WindowLook);
        if (!wins || g === null)
            return null;
        var c = Pages.cover(wins, root.anchorOf(wins), g.room, g.extra, root.starts, root.right);
        if (c === null)
            return null;
        var scale = m.scale > 0 ? m.scale : 1;
        var res = m.reserved && m.reserved.length >= 4 ? m.reserved : [0, 0, 0, 0];
        // In the screen's own coordinates, between the reserved strips (the bar): the
        // empty part, out to the screen's edge on the side it reaches.
        var sw = Math.round(m.width / scale), base = res[0] + WindowLook.gapLeft;
        var x0 = c.from <= Pages.TOLERANCE ? 0 : Math.round(base + c.from);
        var x1 = c.from + c.width >= g.room - Pages.TOLERANCE ? sw : Math.round(base + c.from + c.width);
        return x1 > x0 ? { x: x0, top: res[1], bottom: res[3], w: x1 - x0 } : null;
    }

    screen: modelData
    visible: area !== null
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.namespace: "uq-page-cover"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Ignore
    anchors { top: true; bottom: true; left: true }
    margins { top: area ? area.top : 0; bottom: area ? area.bottom : 0; left: area ? area.x : 0 }
    implicitWidth: area ? area.w : 1

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.RightButton
        onClicked: function (ev) {
            Surfaces.toggle("desktopmenu", cov.modelData,
                            { x: ev.x + (cov.area ? cov.area.x : 0), y: ev.y + (cov.area ? cov.area.top : 0) });
        }
    }
}
