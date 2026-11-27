pragma ComponentBehavior: Bound
// The gaps at a screen's edges (gaps_out) are wider than the gap between two columns, so a
// few pixels of the window on the page beside the one shown show there. A strip at that edge
// draws the wallpaper over them (Pages.edgeStrips): only while there is such a sliver, not
// where a floating window reaches the edge, and not over a special workspace. Draws only.
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
    id: strip
    // Passed in from ScreenPages.qml.
    property var root

    required property var modelData
    property bool rightSide: false
    readonly property var mon: root.monitors[modelData.name] || null
    readonly property var box: {
        var m = strip.mon;
        if (!root.enabled || !WindowLook.known || !m || !m.activeWorkspace)
            return null;
        if (m.specialWorkspace && m.specialWorkspace.name)
            return null;
        var wins = root.tiled[m.activeWorkspace.id];
        if (!wins)
            return null;
        var scale = m.scale > 0 ? m.scale : 1;
        var sw = Math.round(m.width / scale), sh = Math.round(m.height / scale);
        var res = m.reserved && m.reserved.length >= 4 ? m.reserved : [0, 0, 0, 0];
        var bands = { left: res[0] + WindowLook.edgeLeft, right: res[2] + WindowLook.edgeRight };
        var want = Pages.edgeStrips(wins, root.loose[m.activeWorkspace.id] || [], m.x, sw, bands, WindowLook.border);
        if (!(strip.rightSide ? want.right : want.left))
            return null;
        var w = strip.rightSide ? bands.right : bands.left;
        return { x: strip.rightSide ? sw - w : 0, y: res[1], w: w, h: sh - res[1] - res[3], sw: sw, sh: sh };
    }
    readonly property var paper: WallpaperShown.screens[modelData.name] || null

    screen: modelData
    visible: box !== null && paper !== null
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.namespace: "uq-edge-strip"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Ignore
    anchors { top: true; bottom: true; left: !strip.rightSide; right: strip.rightSide }
    margins { top: box ? box.y : 0; bottom: box && paper ? box.sh - box.y - box.h : 0 }
    implicitWidth: box ? box.w : 1
    mask: Region {}

    // The whole screen's wallpaper, moved so the strip shows its own part of it.
    Loader {
        active: strip.visible
        x: strip.box ? -strip.box.x : 0
        y: strip.box ? -strip.box.y : 0
        width: strip.box ? strip.box.sw : 0
        height: strip.box ? strip.box.sh : 0
        sourceComponent: Item {
            UqBackdrop {
                anchors.fill: parent
                visible: strip.paper.backdrop === true
            }
            Crossfade {
                id: paperView
                anchors.fill: parent
                // A target set from the start changes nothing, so show it here.
                Component.onCompleted: paperView.show()
                target: strip.paper.target
                fit: strip.paper.fit
                span: strip.paper.span
                color: strip.paper.color
                reduceMotion: true
                clearOnFail: true
            }
        }
    }
}
