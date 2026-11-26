pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import "../core"
import "../services"
import "../wallpaper"
import "../logic/edges.js" as Edges
import "../logic/mousehold.js" as Hold
import "../logic/pages.js" as Pages
import "../logic/work.js" as Work
import "screenpages"

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
    // windowsOnly: a focus change, which moves no screen and changes no layout, needs only the
    // window list (as ColumnPolicy does); everything else reads the screens and workspaces too.
    function refresh(windowsOnly) {
        root.asked = true;
        HyprRefresh.request(windowsOnly === true ? null : { monitors: true, workspaces: true });
    }
    // After a change that moves windows, Hyprland goes on placing them for a while with no event
    // of its own: a window made Full (fullscreen 1) is reported at once, but the columns beside
    // it reach their new places only a few hundred ms later. The pages and the edge strips would
    // keep the places from the first read, so they are read again twice more.
    function refreshMoved(windowsOnly) {
        root.refresh(windowsOnly);
        root.lateReads = 2;
        lateRead.restart();
    }
    property int lateReads: 0
    property Timer lateRead: Timer {
        interval: 350
        onTriggered: {
            root.refresh(true);
            if (--root.lateReads > 0)
                lateRead.restart();
        }
    }

    // From the last look: per workspace id, its tiled windows when it uses the scrolling layout
    // and no window on it is fullscreen; and per screen name, its monitor (lastIpcObject).
    property var tiled: ({})
    property var monitors: ({})
    // Per workspace id, its floating and pinned windows ({x, w}), which the edge strips keep clear of.
    property var loose: ({})
    // The focused window, as of the last look.
    property string focused: ""

    function read() {
        var scrolling = {}, wvals = Hyprland.workspaces.values;
        for (var i = 0; i < wvals.length; i++) {
            var o = wvals[i].lastIpcObject;
            if (o && o.tiledLayout === "scrolling" && typeof o.id === "number")
                scrolling[o.id] = true;
        }
        var byWs = {}, full = {}, loose = {}, tvals = Hyprland.toplevels.values;
        for (var j = 0; j < tvals.length; j++) {
            var w = Edges.fromIpc(tvals[j].lastIpcObject, tvals[j].title);
            if (w !== null && scrolling[w.ws] && !w.hidden && (w.floating || w.pinned))
                (loose[w.ws] = loose[w.ws] || []).push({ x: w.x, w: w.w });
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
        root.loose = loose;
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
        var right = {};
        for (var wsR in byWs) {
            var gR = root.geometryOf(byWs[wsR]);
            if (gR !== null)
                Object.assign(right, Pages.rightOf(byWs[wsR], gR.room, gR.extra, starts, root.right));
        }
        root.right = right;
        var top = Hyprland.activeToplevel;
        root.focused = top ? (Work.normAddr(top.address) || "") : "";
    }

    // Address -> true for the windows beginning a page (Pages.startsOf), as of the last read.
    property var starts: ({})
    // Address -> true for the first windows of pages put on the right of the screen (a Half alone
    // dropped on its screen's right half), kept while they still can be (Pages.rightOf).
    property var right: ({})
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
            .catch(function (e) { console.warn("uq[WIN-001] screen pages: " + e); })
            .then(function () {
                root.scrolling = false;
                if (root.focusMoved)
                    root.followFocus();
                // The scroll moved the windows without an event of its own: read them again, so
                // the edge strips cover the sliver it left (they were worked out before it).
                root.refreshMoved(true);
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
        var t = Pages.target(wins, addr, g.room, g.extra, g.left, root.starts, root.right);
        if (t === null)
            return;
        root.focused = addr;
        root.scrollTo({ address: t.address, x: t.x });
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
                             hide: Object.keys(Pages.hidden(wins, w.address, g.room, g.extra, root.starts, root.right)).slice(0, 64) };
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
            .catch(function (e) { console.warn("uq[WIN-001] screen pages: " + e); });
    }

    // A column made Full or Half from a tab (PageActions). Made Half, it can leave the rest of
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
    // A tab dropped on screen `screenName` at `x` (the screen's own coordinates): how window
    // `address` would join the page shown there (Pages.dropPlan), with that screen's monitor and
    // geometry; null when it can't.
    function dropPlanFor(screenName, address, x, listed) {
        var m = root.monitors[screenName] || null;
        // Not `enabled`: the pages are held while a window is moved with the pointer, and this
        // is what shows where it would go.
        if (Bridge.locked || !WindowLook.known || !m || !m.activeWorkspace)
            return null;
        var wins = listed || root.tiled[m.activeWorkspace.id], g = Pages.geometry(m, WindowLook);
        if (!wins || g === null)
            return null;
        var res = m.reserved && m.reserved.length >= 4 ? m.reserved : [0, 0, 0, 0];
        var side = Pages.sideAt(wins, g.room, g.extra, root.starts, root.anchorOf(wins), root.right,
                                x - res[0] - WindowLook.gapLeft);
        var plan = Pages.dropPlan(wins, g.room, g.extra, root.starts, root.anchorOf(wins), address, side, root.right);
        return plan === null ? null : { plan: plan, m: m, g: g };
    }
    // The tab's window joins that page: its column moves there, the widths are set, and the
    // pages begin where the plan says (so the window pushed out of a pair gets a page of its
    // own). The window is focused at the end.
    // `listed` (the windows as they were) and `adjust` (columns to move it besides what the plan
    // says): for a window moved with the pointer, which Hyprland has already put somewhere
    // else, see applyPendingDrop.
    function dropTab(screenName, address, x, listed, adjust) {
        var d = root.dropPlanFor(screenName, address, x, listed);
        if (d === null)
            return;
        var plan = d.plan, moves = plan.steps + (adjust || 0), st = Object.assign({}, root.starts);
        for (var a in plan.starts) {
            if (plan.starts[a])
                st[a] = true;
            else
                delete st[a];
        }
        root.starts = st;
        var rt = Object.assign({}, root.right);
        for (var b in plan.right) {
            if (plan.right[b])
                rt[b] = true;
            else
                delete rt[b];
        }
        root.right = rt;
        // A page moved to the other side moves no column: hide what now shows beside it and
        // scroll there.
        if (moves === 0 && plan.half.length === 0) {
            root.syncHidden();
            root.refresh();
            return;
        }
        var warn = function (e) { console.warn("uq[WIN-001] drop a tab on the screen: " + e); };
        var chain = moves !== 0
            ? Bridge.call("Dispatch", ["move_column", { address: address, steps: moves }])
            : Promise.resolve();
        plan.half.forEach(function (h) {
            chain = chain.then(function () {
                return Bridge.call("Dispatch", ["column_width", { address: h, width: "half", keep_focus: true }]);
            });
        });
        chain.then(function () {
            return Bridge.call("Dispatch", ["focus_window", { address: address, keep_pointer: true }]);
        }).catch(warn).then(function () { root.refresh(); });
    }

    Component.onDestruction: {
        if (PageActions.widthHandler === root.setWidth)
            PageActions.widthHandler = null;
        if (PageActions.dropHandler === root.dropTab)
            PageActions.dropHandler = null;
    }

    // The windows (per workspace) as they were when a move with the pointer began, and where
    // their pages began (starts, right).
    property var preDrag: ({})
    property var preStarts: ({})
    property var preRight: ({})
    // A window moved with the pointer (WindowDrag.drop), waiting for the look after it let go.
    property var pendingDrop: null

    // The window's own list among `byWs`, or null.
    function listHolding(byWs, addr) {
        for (var ws in byWs)
            if (Pages.columnIndex(byWs[ws], addr) >= 0)
                return byWs[ws];
        return null;
    }

    // The same rule as a tab dropped on the screen: where the pointer let go decides which half
    // of the page the window joins, worked out on the windows as they were before the move (the
    // preview showed that). Hyprland has put the window back in the layout by its own rule
    // meanwhile, which moved the columns about; the window is moved from where it is to where the
    // rule puts it. A window that was Full and isn't placed anywhere else is made Full again
    // (Hyprland un-maximizes it when it lifts it). Returns true when it did something.
    function applyPendingDrop() {
        var d = root.pendingDrop;
        root.pendingDrop = null;
        if (d === null || !root.enabled)
            return false;
        var addr = d.address !== "" ? d.address : d.full;
        var restoreFull = function () {
            if (d.full === "")
                return false;
            Bridge.call("Dispatch", ["column_width", { address: d.full, width: "full", keep_focus: false }])
                .catch(function (e) { console.warn("uq[WIN-001] drag: " + e); });
            return true;
        };
        var pre = root.listHolding(root.preDrag, addr), now = root.listHolding(root.tiled, addr);
        var at = d.x === null ? null : Hold.place(root.monitors, d.x, d.y);
        // The drop is worked out on the pages as they were when the move began (what the preview
        // showed): the reads since then saw the windows reflowed around the lifted one, and
        // carried the pages' starts over from that.
        if (pre !== null) {
            root.starts = root.preStarts;
            root.right = root.preRight;
        }
        // Hyprland dropped it into another window's column (the columns closed up behind the
        // lifted window, so a window ended up under the pointer), while the screen showed it would
        // get a half of its own: give it its own column back, then look again with that.
        if (pre !== null && now !== null && d.promoted !== true && at !== null
                && Pages.columnCount(now) === Pages.columnCount(pre) - 1
                && root.dropPlanFor(at.screen, addr, at.x, pre) !== null) {
            root.pendingDrop = Object.assign({}, d, { promoted: true });
            Bridge.call("Dispatch", ["own_column", { address: addr }])
                .catch(function (e) { console.warn("uq[WIN-001] drag: " + e); })
                .then(function () { root.refresh(); });
            return true;
        }
        // Anything else (it stacked into another column, or moved to another workspace): leave it
        // where Hyprland put it.
        if (pre === null || now === null || Pages.columnCount(pre) !== Pages.columnCount(now))
            return restoreFull();
        var back = Pages.columnIndex(pre, addr) - Pages.columnIndex(now, addr);
        if (at !== null && root.dropPlanFor(at.screen, addr, at.x, pre) !== null) {
            root.dropTab(at.screen, addr, at.x, pre, back);
            return true;
        }
        // Nothing changes by the rule: it goes back where it was.
        if (back !== 0) {
            var warn = function (e) { console.warn("uq[WIN-001] drag: " + e); };
            Bridge.call("Dispatch", ["move_column", { address: addr, steps: back }]).catch(warn).then(function () {
                if (d.full !== "")
                    restoreFull();
                root.refresh();
            });
            return true;
        }
        return restoreFull();
    }

    function look() {
        read();
        syncHidden();
        if (root.applyPendingDrop())
            return;
        var mon = Hyprland.focusedMonitor;
        var m = mon ? root.monitors[mon.name] : null;
        // The pages need Hyprland's gaps and borders, which arrive a moment after start.
        if (!WindowLook.known || !m || !m.activeWorkspace || root.focused === "")
            return;
        var wins = root.tiled[m.activeWorkspace.id], g = Pages.geometry(m, WindowLook);
        if (!wins || g === null)
            return;
        var to = Pages.snap(wins, root.focused, g.room, g.extra, g.left, root.starts, root.right);
        if (to !== null)
            root.scrollTo(to);
    }

    // Beside a Half window alone on its page, the next page's windows are hidden (above); each
    // screen also lays a transparent surface over that part, so the hidden window there can't be
    // clicked by accident (a right click opens the desktop menu, as on empty desktop).
    Variants {
        model: Quickshell.screens
        delegate: HiddenCover {
            id: cov
            root: root
        }
    }

    Variants {
        model: Quickshell.screens
        delegate: EdgeStrip {
            root: root
        }
    }
    Variants {
        model: Quickshell.screens
        delegate: EdgeStrip {
            root: root
            rightSide: true
        }
    }

    // Windows that share a screen (two Halves, or a pair a resize grip shared out) get a light
    // frame round them together, in the gaps around them; a window on its own has none. Drawn below
    // the windows, where the page shown on each screen sits once the view is on it.
    Variants {
        model: Quickshell.screens
        delegate: SharedFrame {
            id: frameWin
            root: root
        }
    }

    // While a tab is dragged over a screen: where its window would go, drawn as a window frame
    // over that part of the page (nothing where it can't go).
    Variants {
        model: Quickshell.screens
        delegate: DropHint {
            id: hintWin
            root: root
        }
    }

    // A window moved with the pointer: while it is held, the screen shows where it would go if
    // let go here (the frame a dragged tab shows); when it is let go, it is placed there.
    WindowDragDrop {
        root: root
    }

    Component.onCompleted: {
        PageActions.widthHandler = root.setWidth;
        PageActions.dropHandler = root.dropTab;
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
    HyprlandEvents {
        root: root
    }
    Connections {
        target: Bridge
        function onLayoutChanged(ws, layout) { root.refresh(); }
    }
    // Column widths changed elsewhere (ColumnPolicy), with no Hyprland event either.
    Connections {
        target: PageActions
        function onWidthsChanged() { root.refreshMoved(true); }
    }
}
