// "By system usage": the windows' processes, every 5 s while that is the order.
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

Timer {
    // Passed in from WorkIsland.qml.
    property var root

    interval: 5000
    repeat: true
    triggeredOnStart: true
    running: root.sortMode === "usage" && root.visible
    onTriggered: {
        var pids = root.here.map(function (w) { return w.pid; }).filter(function (p) { return p > 0; });
        if (pids.length === 0)
            return;
        Bridge.call("ProcessUsage", [pids]).then(function (u) {
            if (root && u && typeof u === "object")
                root.usageByPid = u;
        }).catch(function () {});
    }
}
