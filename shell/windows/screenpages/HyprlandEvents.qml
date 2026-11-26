pragma ComponentBehavior: Bound
// Hyprland's focus and window events: focus moves the view, and windows coming, going or moving re-read the pages.
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

    target: Hyprland
    // Focus moves the view, without an event of its own.
    function onActiveToplevelChanged() {
        root.followFocus();
        root.refresh(true);
    }
    function onRawEvent(event) {
        switch (event.name) {
        case "openwindow":
        case "closewindow":
        case "movewindowv2":
        case "changefloatingmode":
        case "fullscreen":
        case "workspacev2":
        case "monitoraddedv2":
            root.refreshMoved();
            break;
        case "configreloaded":
            // A reload starts Hyprland's Lua afresh: the hiding table and its handler (and
            // the focus fade as configured) are sent again.
            root.hidingSent = "";
            root.refresh();
            break;
        }
    }
}
