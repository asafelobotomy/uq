pragma ComponentBehavior: Bound
// A window moved with the pointer: while it is held, the screen shows where it would go if let go here (the frame a dragged tab shows); when it is let go, it is placed there.
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

Connections {
    // Passed in from ScreenPages.qml.
    property var root

    target: WindowDrag
    function onCursorChanged() {
        var c = WindowDrag.cursor, lifted = Hold.address(WindowDrag.seen.lifted);
        var at = c === null || lifted === "" ? null : Hold.place(root.monitors, c.x, c.y);
        PageActions.dropHint = at === null ? null : {
            screen: at.screen, address: lifted, x: at.x, y: at.y, drop: true,
            holdX: 0, holdY: 0, width: 0, height: 0, title: "", icon: "", ghost: false
        };
    }
    function onActiveChanged() {
        if (WindowDrag.active) {
            root.preDrag = root.tiled;
            root.preStarts = root.starts;
            root.preRight = root.right;
        }
    }
    function onEnded() {
        PageActions.dropHint = null;
        root.pendingDrop = WindowDrag.drop;
    }
}
