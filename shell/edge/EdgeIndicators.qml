pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Hyprland
import "../core"
import "../logic/edges.js" as Edges
import "../logic/shellprefs.js" as ShellPrefs
import "../logic/work.js" as Work

// Screen-edge indicators: on each screen's left and right edge, when there is a window (or a
// workspace with windows) on that side, a thin strip that shows a tab with that window's app
// icon when the pointer reaches the edge. Clicking or wheeling over it focuses that window,
// which scrolls it into view or switches workspace. The rules for what counts as "beside" are
// in logic/edges.js. Off with shell.edge_indicators = false and while locked.
Scope {
    id: root

    readonly property bool enabled: ShellPrefs.edgeIndicators(Settings.values ? Settings.values.shell : null)
        && !Bridge.locked
    // shell.edge_titles: the tab shows the window's title too; off, it is the icon only.
    readonly property bool showTitles: ShellPrefs.edgeTitles(Settings.values ? Settings.values.shell : null)
    // Every window, as logic/edges.js reads them; rebuilt when Hyprland's list or a window's
    // last IPC reply changes.
    property var windows: []
    // Bumped when workspaces move between monitors, so each edge re-reads its monitor's list.
    property int wsSerial: 0

    function rebuild() {
        var out = [], vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++) {
            var w = Edges.fromIpc(vals[i].lastIpcObject, vals[i].title);
            if (w !== null)
                out.push(w);
        }
        root.windows = out;
    }

    // Ask Hyprland for fresh window positions now (an edge was reached, or a step was taken):
    // the scrolling layout moves columns without an event.
    function refresh() {
        refreshTimer.restart();
    }

    // Focus `n` (a neighbour from Edges.neighbour). Returns whether a dispatch was sent.
    function go(n) {
        var addr = n ? Work.normAddr(n.address) : null;
        if (addr === null)
            return false;
        Bridge.call("Dispatch", ["focus_window", { address: addr, keep_pointer: true }]).catch(function () {});
        root.refresh();
        return true;
    }

    function monitorWorkspaces(monitorName) {
        var out = [], vals = Hyprland.workspaces.values;
        for (var i = 0; i < vals.length; i++)
            if (vals[i].id > 0 && vals[i].monitor && vals[i].monitor.name === monitorName)
                out.push(vals[i].id);
        return out;
    }

    Component.onCompleted: {
        rebuild();
        refresh();
    }

    Timer {
        id: refreshTimer
        interval: 40
        onTriggered: Hyprland.refreshToplevels()
    }
    Timer {
        id: debounce
        interval: 30
        onTriggered: root.rebuild()
    }
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            switch (event.name) {
            case "activewindowv2":
            case "openwindow":
            case "closewindow":
            case "movewindowv2":
            case "changefloatingmode":
            case "fullscreen":
            case "pin":
            case "workspacev2":
            case "focusedmonv2":
            case "moveworkspacev2":
            case "createworkspacev2":
            case "destroyworkspacev2":
            case "monitoraddedv2":
            case "monitorremovedv2":
                root.refresh();
                if (event.name.indexOf("workspace") >= 0 || event.name.indexOf("monitor") >= 0)
                    root.wsSerial++;
                break;
            }
        }
    }
    Connections {
        target: Hyprland.toplevels
        function onValuesChanged() { debounce.restart(); }
    }
    Instantiator {
        model: Hyprland.toplevels.values
        delegate: QtObject {
            required property var modelData
            property Connections watch: Connections {
                target: modelData
                function onLastIpcObjectChanged() { debounce.restart(); }
                function onTitleChanged() { debounce.restart(); }
            }
        }
    }

    Variants {
        model: Quickshell.screens
        delegate: Scope {
            id: perScreen
            required property var modelData
            EdgeWindow {
                edge: root
                display: perScreen.modelData
                side: "left"
            }
            EdgeWindow {
                edge: root
                display: perScreen.modelData
                side: "right"
            }
        }
    }
}
