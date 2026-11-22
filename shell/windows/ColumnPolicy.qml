pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Hyprland
import "../core"
import "../services"
import "../logic/columns.js" as Columns
import "../logic/work.js" as Work

// Screens in the scrolling layout: a window opened from a Full column gets a Full screen of its
// own right after it, one opened from a Half column is Half beside it, and a lone window is made
// Full. The rules are in logic/columns.js; the
// widths are set with the bridge's column_width action (Full is Hyprland's maximize), which
// gives focus back afterwards. Nothing is done while the pointer has hold of a window (WindowDrag):
// Hyprland lifts it out of its column for the move, which is not a window closing, and the others
// are looked at once it is let go.
Scope {
    id: root

    readonly property bool enabled: !Bridge.locked
    // Per workspace name: Columns.summary of its tiled windows, as last read.
    property var summaries: ({})
    // Windows that just opened, with their workspace's summary from just before:
    // [{address, ws, before}].
    property var opened: []
    // Windows closed or moved away since the last look, so a lone window can be made Full.
    property bool shrank: false

    function scrollingWorkspaces() {
        var out = {}, vals = Hyprland.workspaces.values;
        for (var i = 0; i < vals.length; i++) {
            var o = vals[i].lastIpcObject;
            if (o && o.tiledLayout === "scrolling" && typeof o.name === "string" && o.name.indexOf("special:") !== 0)
                out[o.name] = true;
        }
        return out;
    }
    // Every scrolling workspace's summary, from Hyprland's current lists.
    function read() {
        var scrolling = scrollingWorkspaces(), wins = [], vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++) {
            var t = vals[i], o = t.lastIpcObject;
            if (!o || !o.workspace || !scrolling[o.workspace.name] || o.hidden === true)
                continue;
            var a = Work.normAddr(o.address);
            if (a === null)
                continue;
            wins.push({
                address: a, ws: o.workspace.name, floating: o.floating === true || o.pinned === true,
                history: typeof o.focusHistoryID === "number" ? o.focusHistoryID : 1e9,
                // Full: a maximized column (fullscreen 1) or one as wide as the screen; not
                // real fullscreen (2), whose width isn't the column's.
                full: o.fullscreen !== 2 && (o.fullscreen === 1
                    || Work.spansScreen(o, t.monitor ? t.monitor.lastIpcObject : null))
            });
        }
        var out = {};
        for (var ws in scrolling)
            out[ws] = Columns.summary(wins, ws);
        return { wins: wins, summaries: out };
    }

    // Set each window's width ([{address, full}]), one after another.
    function setWidths(list) {
        var chain = Promise.resolve();
        list.forEach(function (x) {
            chain = chain.then(function () {
                return Bridge.call("Dispatch", ["column_width",
                    { address: x.address, width: x.full ? "full" : "half", keep_focus: true }]);
            });
        });
        chain.catch(function (e) { console.warn("uq: screens: " + e); })
            .then(function () { PageActions.widthsChanged(); });
    }

    function look() {
        // Left as it is, so the look after the drag compares with the windows from before it.
        if (WindowDrag.active)
            return;
        var now = read();
        if (root.enabled) {
            var todo = [];
            for (var i = 0; i < root.opened.length; i++) {
                var p = root.opened[i];
                var tiled = now.wins.some(function (w) { return w.address === p.address && !w.floating; });
                if (tiled)
                    todo = todo.concat(Columns.onOpen(p.before, p.address));
            }
            if (root.shrank)
                Columns.leftAlone(root.summaries, now.summaries).forEach(function (l) {
                    if (!todo.some(function (x) { return x.address === l.address; }))
                        todo.push({ address: l.address, full: true });
                });
            if (todo.length > 0)
                setWidths(todo);
        }
        root.opened = [];
        root.shrank = false;
        root.summaries = now.summaries;
    }

    // Set when this asked for fresh lists, so the settle after it looks again.
    property bool asked: false
    // Focus changes only need the windows; the screens (whose widths say what spans one) and
    // workspace layouts are read again on the rarer events.
    function refresh(all) {
        root.asked = true;
        HyprRefresh.request(all ? { monitors: true, workspaces: true } : null);
    }
    Component.onCompleted: refresh(true)
    // lastIpcObject arrives a moment after the refresh.
    Connections {
        target: HyprRefresh
        function onSettled() {
            if (!root.asked)
                return;
            root.asked = false;
            root.look();
        }
    }
    // The pointer let go of a window: look at the windows as they are now.
    Connections {
        target: WindowDrag
        function onEnded() { root.refresh(true); }
    }
    // A workspace switched to or from the scrolling layout.
    Connections {
        target: Bridge
        function onLayoutChanged(ws, layout) { root.refresh(true); }
    }

    // Focus moved. Not the activewindowv2 event: Hyprland sends that again, for the same window,
    // on every title change of the focused window.
    Connections {
        target: Hyprland
        function onActiveToplevelChanged() { root.refresh(false); }
    }
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            var n = event.name;
            if (n === "openwindow") {
                // ADDRESS,WORKSPACENAME,CLASS,TITLE (the address without 0x)
                var parts = String(event.data).split(",");
                var a = Work.normAddr("0x" + parts[0]);
                if (a !== null && parts.length >= 2) {
                    var ws = parts[1];
                    var before = root.summaries[ws] || { count: 0, active: "", activeFull: false };
                    root.opened = root.opened.concat([{ address: a, ws: ws, before: before }]);
                }
                root.refresh(true);
            } else if (n === "closewindow" || n === "movewindowv2" || n === "changefloatingmode") {
                root.shrank = true;
                root.refresh(true);
            } else if (n === "workspacev2" || n === "configreloaded" || n === "monitoraddedv2") {
                root.refresh(true);
            }
        }
    }
}
