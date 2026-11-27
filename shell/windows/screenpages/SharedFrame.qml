pragma ComponentBehavior: Bound
// Windows that share a screen get a light frame round them together, in the gaps around them; a window on its own has none.
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
    id: frameWin
    // Passed in from ScreenPages.qml.
    property var root

    required property var modelData
    readonly property var mon: root.monitors[modelData.name] || null
    readonly property var rect: {
        var m = frameWin.mon;
        if (!root.enabled || !WindowLook.known || !m || !m.activeWorkspace)
            return null;
        var wins = root.tiled[m.activeWorkspace.id], g = Pages.geometry(m, WindowLook);
        if (!wins || g === null)
            return null;
        var sh = Pages.shared(wins, root.anchorOf(wins), g.room, g.extra, root.starts, root.right);
        if (sh === null)
            return null;
        var scale = m.scale > 0 ? m.scale : 1;
        var res = m.reserved && m.reserved.length >= 4 ? m.reserved : [0, 0, 0, 0];
        var gin = WindowLook.windowGap / 2;
        // The windows' frames (borders included), then out into the gaps around them.
        var pad = Math.max(2, Math.min(gin, WindowLook.gapTop, WindowLook.gapBottom) - 1);
        var left = res[0] + WindowLook.gapLeft + sh.from + gin;
        var top = res[1] + WindowLook.gapTop;
        return { x: left - pad, y: top - pad, w: sh.width - 2 * gin + 2 * pad,
                 h: Math.round(m.height / scale) - res[3] - WindowLook.gapBottom - top + 2 * pad,
                 radius: WindowLook.frameRadius + pad };
    }

    screen: modelData
    visible: rect !== null
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Bottom
    WlrLayershell.namespace: "uq-page-frame"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Ignore
    anchors { top: true; bottom: true; left: true; right: true }
    mask: Region {}

    Rectangle {
        visible: frameWin.rect !== null
        x: frameWin.rect ? frameWin.rect.x : 0
        y: frameWin.rect ? frameWin.rect.y : 0
        width: frameWin.rect ? frameWin.rect.w : 0
        height: frameWin.rect ? frameWin.rect.h : 0
        radius: frameWin.rect ? frameWin.rect.radius : 0
        color: "transparent"
        border.width: 2
        border.color: Qt.alpha(Theme.text, 0.35)
    }
}
