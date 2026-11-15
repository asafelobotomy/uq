pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import "../core"
import "../services"
import "../logic/edges.js" as Edges
import "../logic/pages.js" as Pages
import "../logic/work.js" as Work

// Whole screen pages in the scrolling layout: the columns fall into pages a screen wide (a Full
// column, two Half ones, or a pair a resize grip shared out), and after focus moves or windows
// open, close or move, the view is scrolled so the focused window's page fills the screen, with
// no window cut by its edge. Hyprland on its own only brings the focused column on screen, which
// can leave the column before it half there. The pages are in logic/pages.js; the view is moved
// with the bridge's `scroll`, which leaves focus and the pointer alone. Beside a Half window alone
// on its page, the next page's windows are made invisible (they keep their place, and slide with
// the view), so the rest of the screen is empty desktop. Held while `paused` (a
// resize drag, which keeps its own columns in place) and while locked.
Scope {
    id: root

    property bool paused: false
    readonly property bool enabled: !Bridge.locked && !root.paused

    // Set when this asked for fresh lists, so the settle after it looks.
    property bool asked: false
    function refresh() {
        root.asked = true;
        HyprRefresh.request({ monitors: true, workspaces: true });
    }

    // From the last look: per workspace id, its tiled windows when it uses the scrolling layout
    // and no window on it is fullscreen; and per screen name, its monitor (lastIpcObject).
    property var tiled: ({})
    property var monitors: ({})
    // The focused window, as of the last look.
    property string focused: ""

    function read() {
        var scrolling = {}, wvals = Hyprland.workspaces.values;
        for (var i = 0; i < wvals.length; i++) {
            var o = wvals[i].lastIpcObject;
            if (o && o.tiledLayout === "scrolling" && typeof o.id === "number")
                scrolling[o.id] = true;
        }
        var byWs = {}, full = {}, tvals = Hyprland.toplevels.values;
        for (var j = 0; j < tvals.length; j++) {
            var w = Edges.fromIpc(tvals[j].lastIpcObject, tvals[j].title);
            if (w === null || !scrolling[w.ws] || w.floating || w.pinned || w.hidden)
                continue;
            w.monitorName = tvals[j].monitor ? tvals[j].monitor.name : "";
            // A fullscreen window covers the screen: no pages to keep.
            if (w.fullscreen === 2)
                full[w.ws] = true;
            (byWs[w.ws] = byWs[w.ws] || []).push(w);
        }
        for (var ws in full)
            delete byWs[ws];
        var mons = {}, mvals = Hyprland.monitors.values;
        for (var k = 0; k < mvals.length; k++)
            if (mvals[k].lastIpcObject && mvals[k].name)
                mons[mvals[k].name] = mvals[k].lastIpcObject;
        root.tiled = byWs;
        root.monitors = mons;
        // The windows beginning each page keep doing so (Pages.pages): carried over from the
        // last read, then taken from the pages as they are now.
        var starts = {};
        for (var wsName in byWs) {
            var g = root.geometryOf(byWs[wsName]);
            if (g !== null)
                Object.assign(starts, Pages.startsOf(Pages.pages(byWs[wsName], g.room, g.extra, root.starts)));
        }
        root.starts = starts;
        var top = Hyprland.activeToplevel;
        root.focused = top ? (Work.normAddr(top.address) || "") : "";
    }

    // Address -> true for the windows beginning a page (Pages.startsOf), as of the last read.
    property var starts: ({})
    // Pages.geometry for a workspace's windows (from read()), from the screen they are on.
    function geometryOf(wins) {
        if (!WindowLook.known || !wins || wins.length === 0)
            return null;
        return Pages.geometry(root.monitors[wins[0].monitorName] || null, WindowLook);
    }

    // The window a screen's pages follow: the focused one when it is there, else the one most
    // recently focused on that workspace.
    function anchorOf(wins) {
        var best = null;
        for (var i = 0; i < wins.length; i++) {
            if (wins[i].address === root.focused)
                return wins[i].address;
            if (best === null || wins[i].history < best.history)
                best = wins[i];
        }
        return best ? best.address : "";
    }

    // Focus moved: act at once on the pages as last read, rather than after fresh lists arrive
    // (a moment in which Hyprland's own scroll would show the window a cover hides). Moving focus
    // doesn't change the pages, only which one is shown: the cover follows `focused`, and the view
    // is scrolled to the page's first window, whose position the bridge reads live. A window not
    // on the pages yet (just opened) waits for the look after the refresh.
    // A scroll is under way. The layout's `move` focuses the window it brings into view and the
    // bridge gives focus back, so the focus changes meanwhile are this scroll's own: they are
    // left alone, and focus is looked at once more when it is done.
    property bool scrolling: false
    property bool focusMoved: false
    function scrollTo(to) {
        root.scrolling = true;
        root.focusMoved = false;
        Bridge.call("Dispatch", ["scroll", to])
            .catch(function (e) { console.warn("uq: screen pages: " + e); })
            .then(function () {
                root.scrolling = false;
                if (root.focusMoved)
                    root.followFocus();
            });
    }

    function followFocus() {
        if (root.scrolling) {
            root.focusMoved = true;
            return;
        }
        var top = Hyprland.activeToplevel, mon = Hyprland.focusedMonitor;
        var addr = top ? (Work.normAddr(top.address) || "") : "";
        var m = mon ? root.monitors[mon.name] : null;
        if (addr === "" || !root.enabled || !WindowLook.known || !m || !m.activeWorkspace)
            return;
        var wins = root.tiled[m.activeWorkspace.id], g = Pages.geometry(m, WindowLook);
        if (!wins || g === null)
            return;
        var p = Pages.pageOf(Pages.pages(wins, g.room, g.extra, root.starts), addr);
        if (p === null)
            return;
        root.focused = addr;
        root.scrollTo({ address: p.cols[0].wins[0].address, x: Math.round(g.left) });
    }

    // Beside a Half alone on its page, the next page's windows are hidden (Pages.hidden). Hyprland
    // does the hiding itself, the moment focus moves (a round trip through here would show them
    // for a frame or two): it is handed, per workspace, the windows to hide while each window is
    // focused (the bridge's `page_hiding`). Sent when it changes; the first time, every window is
    // shown first (a shell that stopped may have left some hidden).
    property string hidingSent: ""
    property bool hidingReset: true
    // `tiled`: the windows to work from (root.tiled, or a forecast of it). Returns the call's
    // promise (resolved at once when there is nothing new to send).
    function syncHidden(tiled) {
        var spaces = [], from = tiled || root.tiled;
        if (root.enabled && WindowLook.known) {
            for (var ws in from) {
                var wins = from[ws], g = root.geometryOf(wins);
                if (g === null || wins.length === 0)
                    continue;
                var list = wins.slice(0, 64).map(function (w) {
                    return { address: w.address,
                             hide: Object.keys(Pages.hidden(wins, w.address, g.room, g.extra, root.starts)).slice(0, 64) };
                });
                spaces.push({ id: Number(ws), anchor: root.anchorOf(wins), windows: list });
            }
        }
        var args = { workspaces: spaces.slice(0, 32) };
        var key = JSON.stringify(args);
        if (key === root.hidingSent && !root.hidingReset)
            return Promise.resolve();
        if (root.hidingReset) {
            var all = Hyprland.toplevels.values, reset = [];
            for (var j = 0; j < all.length && reset.length < 256; j++) {
                var a = Work.normAddr(all[j].address);
                if (a !== null)
                    reset.push(a);
            }
            args.reset = reset;
            root.hidingReset = false;
        }
        root.hidingSent = key;
        return Bridge.call("Dispatch", ["page_hiding", args])
            .catch(function (e) { console.warn("uq: screen pages: " + e); });
    }

    // A column made Full or Half from a tab (ColumnWidths). Made Half, it can leave the rest of
    // its screen empty, where the next page's window would show until the pages were read
    // again: the pages are worked out as they will be (its window half the room wide) and
    // Hyprland handed them first, so that window is hidden before the width changes.
    function setWidth(address, full) {
        var call = function () {
            return Bridge.call("Dispatch", ["column_width", { address: address, width: full ? "full" : "half" }]);
        };
        if (full || !root.enabled)
            return call();
        var forecast = null;
        for (var ws in root.tiled) {
            var wins = root.tiled[ws], g = root.geometryOf(wins);
            if (g === null || !wins.some(function (w) { return w.address === address; }))
                continue;
            forecast = Object.assign({}, root.tiled);
            forecast[ws] = wins.map(function (w) {
                return w.address !== address ? w
                    : Object.assign({}, w, { w: Math.round(g.room / 2 - g.extra) });
            });
        }
        if (forecast === null)
            return call();
        return root.syncHidden(forecast).then(call);
    }
    Component.onDestruction: if (ColumnWidths.handler === root.setWidth) ColumnWidths.handler = null

    function look() {
        read();
        syncHidden();
        var mon = Hyprland.focusedMonitor;
        var m = mon ? root.monitors[mon.name] : null;
        // The pages need Hyprland's gaps and borders, which arrive a moment after start.
        if (!WindowLook.known || !m || !m.activeWorkspace || root.focused === "")
            return;
        var wins = root.tiled[m.activeWorkspace.id], g = Pages.geometry(m, WindowLook);
        if (!wins || g === null)
            return;
        var to = Pages.snap(wins, root.focused, g.room, g.extra, g.left, root.starts);
        if (to !== null)
            root.scrollTo(to);
    }

    // Beside a Half window alone on its page, the next page's windows are hidden (above); each
    // screen also lays a transparent surface over that part, so the hidden window there can't be
    // clicked by accident (a right click opens the desktop menu, as on empty desktop).
    Variants {
        model: Quickshell.screens
        delegate: PanelWindow {
            id: cov
            required property var modelData
            readonly property var mon: root.monitors[modelData.name] || null
            readonly property var area: {
                var m = cov.mon;
                if (!root.enabled || !WindowLook.known || !m || !m.activeWorkspace)
                    return null;
                var wins = root.tiled[m.activeWorkspace.id], g = Pages.geometry(m, WindowLook);
                if (!wins || g === null)
                    return null;
                var c = Pages.cover(wins, root.anchorOf(wins), g.room, g.extra, root.starts);
                if (c === null)
                    return null;
                var scale = m.scale > 0 ? m.scale : 1;
                var res = m.reserved && m.reserved.length >= 4 ? m.reserved : [0, 0, 0, 0];
                // In the screen's own coordinates: from the empty part's left edge to the
                // screen's right edge, between the reserved strips (the bar).
                var x = Math.round(res[0] + WindowLook.gapLeft + c.from);
                var w = Math.round(m.width / scale) - x;
                return w > 0 ? { x: x, top: res[1], bottom: res[3], w: w } : null;
            }

            screen: modelData
            visible: area !== null
            color: "transparent"
            WlrLayershell.layer: WlrLayer.Top
            WlrLayershell.namespace: "uq-page-cover"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            exclusionMode: ExclusionMode.Ignore
            anchors { top: true; bottom: true; right: true }
            margins { top: area ? area.top : 0; bottom: area ? area.bottom : 0 }
            implicitWidth: area ? area.w : 1

            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.RightButton
                onClicked: function (ev) {
                    Surfaces.toggle("desktopmenu", cov.modelData,
                                    { x: ev.x + (cov.area ? cov.area.x : 0), y: ev.y + (cov.area ? cov.area.top : 0) });
                }
            }
        }
    }

    Component.onCompleted: {
        ColumnWidths.handler = root.setWidth;
        refresh();
    }
    Connections {
        target: HyprRefresh
        function onSettled() {
            if (!root.asked)
                return;
            root.asked = false;
            if (root.enabled)
                root.look();
        }
    }
    onPausedChanged: if (!root.paused) root.refresh()
    Connections {
        target: WindowLook
        function onKnownChanged() { root.refresh(); }
    }
    Connections {
        target: Hyprland
        // Focus moves the view, without an event of its own.
        function onActiveToplevelChanged() {
            root.followFocus();
            root.refresh();
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
                root.refresh();
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
    Connections {
        target: Bridge
        function onLayoutChanged(ws, layout) { root.refresh(); }
    }
}
