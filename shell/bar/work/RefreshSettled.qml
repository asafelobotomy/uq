// lastIpcObject (the class, tags, pid) arrives a moment after the refresh.
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

Connections {
    // Passed in from WorkIsland.qml.
    property var root

    target: HyprRefresh
    function onSettled() {
        if (!root.asked)
            return;
        root.asked = false;
        var now = Date.now(), kept = ({});
        for (var k in root.wsMoved)
            if (now - root.wsMoved[k].at < 1000)
                kept[k] = root.wsMoved[k];
        root.wsMoved = kept;
        root.rebuild();
    }
}
