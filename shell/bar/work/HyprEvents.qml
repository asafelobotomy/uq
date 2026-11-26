// Hyprland's events: a window opened, closed, moved, floated or pinned, a title changed, or a window wants attention.
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
    property var titleTimer

    target: Hyprland
    function onRawEvent(event) {
        if (event.name === "changefloatingmode" || event.name === "pin"
                || event.name === "openwindow" || event.name === "closewindow"
                || event.name === "movewindowv2" || event.name === "workspacev2") {
            root.refresh();
            if (event.name === "openwindow") {
                var opened = Work.normAddr("0x" + String(event.data).split(",")[0]), was = root.activeAddr();
                if (opened !== null && was !== "" && was !== opened) {
                    var of = {};
                    for (var ka in root.openedFrom)
                        if (root.openedSeq[ka] !== undefined)
                            of[ka] = root.openedFrom[ka];
                    of[opened] = was;
                    root.openedFrom = of;
                }
            }
            if (event.name === "movewindowv2") {
                // "address,workspace id,workspace name"
                var parts = String(event.data).split(",");
                var moved = parts.length >= 3 ? Work.normAddr("0x" + parts[0]) : null;
                if (moved !== null) {
                    var mv = Object.assign({}, root.wsMoved);
                    var was = root.windows.find(function (w) { return Work.normAddr(w.address) === moved; });
                    mv[moved] = { name: parts.slice(2).join(","), at: Date.now(),
                                  from: was && typeof was.wsId === "number" && was.wsId > 0 ? was.wsId : null };
                    root.wsMoved = mv;
                    Qt.callLater(root.rebuild);
                }
            }
        }
        // Quickshell sets the title itself from this event: no need to ask Hyprland again.
        // Throttled, not debounced: a title that never stops changing still shows.
        else if (event.name === "windowtitlev2") {
            if (!titleTimer.running)
                titleTimer.start();
        }
        // A window asked for attention (urgency hint, or an activation request not honoured).
        else if (event.name === "urgent")
            root.alertFor(String(event.data));
    }
}
