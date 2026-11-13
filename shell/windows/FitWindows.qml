pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Hyprland
import "../core"
import "../logic/fit.js" as Fit
import "../logic/shellprefs.js" as ShellPrefs

// Keep windows on screen (shell.fit_windows, on unless set to false): when a window opens, turns
// floating or moves to another workspace, and when screens are added, removed or rearranged, any
// floating window that hangs over its screen's usable area is moved back inside, and one bigger
// than it is shrunk to fit, so none of its UI is cut off. The rules are in logic/fit.js. A window
// the user drags partly off screen afterwards is left there: only those events are acted on.
Scope {
    id: root

    readonly property bool enabled: ShellPrefs.fitWindows(Settings.values ? Settings.values.shell : null)
        && !Bridge.locked
    // Windows to look at once Hyprland's lists are fresh: address -> true; "*" for all.
    property var pending: ({})
    // Windows that just opened, looked at again a moment later: apps often set their size
    // after they first show.
    property var late: ({})

    function want(addr, opened) {
        if (!root.enabled || addr === null)
            return;
        root.pending[addr] = true;
        if (opened)
            root.late[addr] = true;
        refreshTimer.restart();
        if (opened)
            lateTimer.restart();
    }

    function check(which) {
        if (!root.enabled)
            return;
        var mons = [], mvals = Hyprland.monitors.values;
        for (var i = 0; i < mvals.length; i++) {
            var m = Fit.monitorFromIpc(mvals[i].lastIpcObject);
            if (m !== null)
                mons.push(m);
        }
        var all = which["*"] === true, tvals = Hyprland.toplevels.values;
        for (var j = 0; j < tvals.length; j++) {
            var w = Fit.windowFromIpc(tvals[j].lastIpcObject);
            if (w === null || !(all || which[w.address]))
                continue;
            var p = Fit.placement(w, Fit.monitorFor(w, mons), Theme.gapOut);
            if (p !== null)
                Bridge.call("Dispatch", ["place", { address: w.address, x: p.x, y: p.y, w: p.w, h: p.h }])
                    .catch(function (e) { console.warn("uq: keep on screen: " + e); });
        }
    }

    Timer {
        id: refreshTimer
        interval: 60
        onTriggered: {
            Hyprland.refreshMonitors();
            Hyprland.refreshToplevels();
            checkTimer.restart();
        }
    }
    Timer {
        id: checkTimer
        interval: 200
        onTriggered: {
            var which = root.pending;
            root.pending = {};
            root.check(which);
        }
    }
    Timer {
        id: lateTimer
        interval: 1200
        onTriggered: {
            var which = root.late;
            root.late = {};
            for (var k in which)
                root.pending[k] = true;
            refreshTimer.restart();
        }
    }

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (Fit.screensChanged(event.name)) {
                root.want("*", false);
                return;
            }
            var addr = Fit.eventAddress(event.name, event.data);
            if (addr !== null)
                root.want(addr, event.name === "openwindow");
        }
    }
}
