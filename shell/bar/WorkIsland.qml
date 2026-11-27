import QtQuick
import QtQuick.Effects
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import "../core"
import "../services"
import "../notif"
import "islands"
import "../logic/minimise.js" as Min
import "../logic/work.js" as Work
import "../logic/dock.js" as Dock
import "work"

// Open windows (`work`): one tab per window on this screen's current workspace, like browser
// tabs, in the order the windows sit on screen (left to right, as the scrolling layout's columns
// run; the island's setting shell.bar.options.work can pick another order in edit mode). The
// focused window's tab grows to Medium and the others share what is left. What a tab shows
// depends on its width (Work.tabTier): the title at every size but Icon (untrusted, elided), and
// beside it, Small, Full or Half (whichever the window isn't, under the pointer) and close;
// Medium, Half, Full and close. At Icon size the tab is the app icon, which turns into a close
// button under the pointer, and the title shows in a tooltip. Full makes the window's column the
// screen's width and Half half of it (outside the scrolling layout, Full maximizes and Half puts
// it back). A minimised window's tab is always an icon, greyed, at the end of the tabs; choosing
// it brings the window back as the last column, focused. Past the narrowest tabs, arrows scroll
// the strip and "+N" lists every window; when a window out of view gets focus the strip centres
// on it. When not even one tab fits, the lead window's icon lists them all. Left click focuses a
// window (or brings a minimised one back), middle click closes it, right click opens its menu
// (minimise, full or half width, float, pin the app, move it, close it, and the uQ Island Menu).
// A window that wants attention (Hyprland marks it urgent, or its app sends a notification) gets
// a dot in the accent colour on its tab's icon until it is focused.
Island {
    id: root
    property var screen: null
    readonly property real dpr: screen ? screen.devicePixelRatio : 1
    readonly property int iconSize: 20
    readonly property int cell: 26
    readonly property var opts: BarLayout.options.work || ({})
    readonly property string sortMode: Work.SORTS.indexOf(opts.sort) >= 0 ? opts.sort : Work.DEFAULT_SORT
    // What a tab's X does to an open window: "ask", "minimize" or "close".
    readonly property string closeMode: Work.CLOSES.indexOf(opts.close) >= 0 ? opts.close : "ask"

    readonly property string monitorName: {
        var m = root.screen ? Hyprland.monitorFor(root.screen) : null;
        return m ? m.name : "";
    }
    readonly property int wsId: {
        var m = root.screen ? Hyprland.monitorFor(root.screen) : null;
        return m && m.activeWorkspace ? m.activeWorkspace.id : -1;
    }
    // The focused window, once focus has held for a moment: resizing a column (ResizeGrips, the
    // bridge's `columns`) focuses each column in turn and gives focus back within a few
    // milliseconds, and those flips mustn't move the highlight or regrow the tabs. Followed
    // through activeToplevel, not the activewindowv2 event: Hyprland sends that again, for the
    // same window, on every title change of the focused window.
    function activeAddr() {
        return Hyprland.activeToplevel ? (Work.normAddr(Hyprland.activeToplevel.address) || "") : "";
    }
    // Set by focusSettle only (not a binding, which would follow every flip at once).
    property string focusedAddr: ""
    Timer {
        id: focusSettle
        interval: 60
        onTriggered: {
            var a = root.activeAddr();
            if (a === root.focusedAddr)
                return;
            root.focusedAddr = a;
            root.pruneAlerts();
            // Focus moved: the scrolling layout may have moved the columns too.
            root.refresh();
        }
    }
    Connections {
        target: Hyprland
        function onActiveToplevelChanged() { focusSettle.restart(); }
    }

    // Every open window, as logic/work.js reads them; rebuilt when Hyprland's list changes.
    property var windows: []
    // When each window was first seen (its opening order), by address.
    property var openedSeq: ({})
    property int nextSeq: 0
    // The last notifications ({appName, appIcon, at}), newest last, for "Last activity".
    property var notes: []
    // Processor and memory use by pid, for "By system usage".
    property var usageByPid: ({})

    function rebuild() {
        // Opening order of the windows still open: closed ones drop out, so the map stays small.
        var out = [], vals = Hyprland.toplevels.values, seq = ({});
        for (var i = 0; i < vals.length; i++) {
            var t = vals[i], o = t.lastIpcObject || ({});
            var wo = t.workspace && t.workspace.lastIpcObject ? t.workspace.lastIpcObject : null;
            var a = Work.normAddr(t.address);
            if (a !== null)
                seq[a] = openedSeq[a] !== undefined ? openedSeq[a] : nextSeq++;
            // A move Hyprland has announced, before Quickshell's lists show it (wsMoved).
            var wsName = t.workspace ? t.workspace.name : "", moved = a !== null ? wsMoved[a] : undefined;
            if (moved !== undefined && moved.name !== wsName)
                wsName = moved.name;
            var minimised = wsName === "special:minimized";
            // Its tag says which workspace it came from; until the tag arrives, the one it was on.
            var from = minimised ? Min.wsFromTags(o.tags) : null;
            if (minimised && from === null && moved !== undefined)
                from = moved.from;
            out.push({
                address: t.address,
                cls: o["class"] ? String(o["class"]) : "",
                title: t.title || "",
                wsId: t.workspace ? t.workspace.id : null,
                minimised: minimised,
                minimisedFrom: from,
                monitor: t.monitor ? t.monitor.name : "",
                pid: typeof o.pid === "number" ? o.pid : 0,
                x: o.at ? Number(o.at[0]) || 0 : 0,
                y: o.at ? Number(o.at[1]) || 0 : 0,
                w: o.size ? Number(o.size[0]) || 0 : 0,
                h: o.size ? Number(o.size[1]) || 0 : 0,
                floating: o.floating === true,
                fullscreen: typeof o.fullscreen === "number" ? o.fullscreen : 0,
                full: Work.spansScreen(o, t.monitor ? t.monitor.lastIpcObject : null),
                scrolling: !!wo && wo.tiledLayout === "scrolling"
            });
        }
        // Unchanged (a refresh after a focus change often is): keep the lists, so the tabs and
        // everything sorted from them aren't recomputed.
        var key = JSON.stringify(out);
        if (key !== windowsKey) {
            windowsKey = key;
            openedSeq = seq;
            windows = out;
            pruneAlerts();
        }
    }
    property string windowsKey: ""
    // Windows Hyprland said it moved (movewindowv2), by address: {name (the workspace name it
    // gave), at, from (the workspace it was on: a minimised window's tab needs it before its tag
    // arrives)}. The toplevels' workspaces only follow with the next refresh, ~200 ms later; until
    // then the tabs go by this, so a minimised window's tab greys at once (it used to drop out of
    // the strip for that moment and come back greyed). Dropped a second on, once the refreshes
    // have long caught up.
    property var wsMoved: ({})
    Component.onCompleted: {
        focusedAddr = activeAddr();
        noteSeen = Work.noteArrived(null, NotifBackend.latest, NotifBackend.count).seen;
        rebuild();
        refresh();
    }

    // Windows that want attention, address → true: a dot on their tab's icon until focused.
    property var alerts: ({})
    // The notifications already seen (Work.noteArrived), so only new ones mark windows.
    property var noteSeen: null
    function pruneAlerts() {
        alerts = Work.keepAlerts(alerts, windows.map(function (w) { return w.address; }), focusedAddr);
    }
    function alertFor(addr) {
        alerts = Work.withAlerts(alerts, [addr], activeAddr());
    }
    // A new notification marks its app's windows, on every workspace, unless that app is the one
    // in use (its window is focused).
    function checkNote() {
        var l = NotifBackend.latest;
        var r = Work.noteArrived(noteSeen, l, NotifBackend.count);
        noteSeen = r.seen;
        if (!r.fresh)
            return;
        // "Last activity": only a notification that is really new (a backend re-sends `latest`
        // on every state change, which isn't activity).
        if (l.appName || l.appIcon)
            notes = Work.withNote(notes, l, Date.now());
        var f = activeAddr(), hits = [];
        for (var i = 0; i < windows.length; i++) {
            var w = windows[i];
            if (!Work.notifMatches(l, appOf(w.cls)))
                continue;
            if (Work.normAddr(w.address) === f)
                return;
            hits.push(w.address);
        }
        alerts = Work.withAlerts(alerts, hits, f);
    }
    readonly property bool alertHere: ordered.some(function (w) { return root.alerts[w.address] === true; })
    // A window out of view in the scrolled strip wants attention: a dot on "+N".
    readonly property bool alertHidden: fit.mode === "scroll" && ordered.some(function (w, i) {
        return root.alerts[w.address] === true && (i < root.start || i >= root.start + root.fit.shown);
    })
    // Set when this asked for fresh windows, so the settle after it rebuilds.
    property bool asked: false
    function refresh() {
        root.asked = true;
        HyprRefresh.request();
    }

    // The app for a window class, looked up once per class (every sort and tab asks): emptied
    // when the installed apps change.
    property var appCache: ({})
    function appOf(cls) {
        var hit = appCache[cls];
        if (hit !== undefined)
            return hit;
        var d = cls ? DesktopEntries.heuristicLookup(cls) : null;
        hit = { cls: cls, id: d ? String(d.id) : "", name: d && d.name ? String(d.name) : cls, icon: d && d.icon ? d.icon : "" };
        appCache[cls] = hit;
        return hit;
    }
    Connections {
        target: DesktopEntries.applications
        function onValuesChanged() { root.appCache = {}; }
    }
    // A dialog or other window an app opened from one of its windows shares that window's tab
    // (Work.groupChildren): `children` on each.
    readonly property var here: Work.groupChildren(Work.windowsHere(windows, monitorName, wsId, focusedAddr),
                                                   openedSeq, openedFrom)
    // The window that had focus when each window opened (address -> address): the likely parent
    // of a dialog. Kept for the windows still open.
    property var openedFrom: ({})
    readonly property var ordered: {
        var ctx = { opened: openedSeq, activity: {}, pinIndex: {}, usage: {}, name: {}, lastOrder: memo.order, lastMin: memo.min };
        var pins = Settings.values.pinned || [];
        for (var i = 0; i < here.length; i++) {
            var w = here[i], app = appOf(w.cls);
            ctx.name[w.address] = app.name;
            var pi = pins.indexOf(app.id);
            if (pi >= 0)
                ctx.pinIndex[w.address] = pi;
            for (var j = notes.length - 1; j >= 0; j--)
                if (Work.notifMatches(notes[j], app)) {
                    ctx.activity[w.address] = notes[j].at;
                    break;
                }
            var u = usageByPid[w.pid];
            if (u)
                ctx.usage[w.address] = (u.cpu || 0) + (u.rss || 0) / 1e8;
        }
        return Work.sortWindows(here, sortMode, opts.descending === true, ctx);
    }

    // The order last shown, for "Screen order" to keep minimised windows in their place. Held
    // inside an object so updating it doesn't re-run `ordered`.
    readonly property var memo: ({ order: [], min: [] })
    onOrderedChanged: {
        memo.order = ordered.map(function (w) { return w.address; });
        memo.min = ordered.filter(function (w) { return w.minimised; }).map(function (w) { return w.address; });
    }

    // --- the strip ---
    readonly property real closeW: Theme.fontSize + Theme.gapIn
    // The narrowest width of each tab size (Work.tabTier): the icon with its margins, then a
    // button more each step, then room for a short title.
    readonly property real tabPad: 6
    // The title is shown at every size but Icon, so even the smallest tab with buttons has room
    // for a few letters of it (past that, the tab is an icon and the title is in its tooltip).
    readonly property real titleMin: Theme.fontSize * 3 + Theme.gapIn
    readonly property var tiers: ({
        small: tabPad * 2 + iconSize + Theme.gapIn + closeW * 2 + 2 + titleMin,
        medium: tabPad * 2 + iconSize + Theme.gapIn + closeW * 3 + 4 + titleMin,
        large: tabPad * 2 + iconSize + Theme.gapIn * 2 + closeW * 3 + 4 + titleMin + Theme.fontSize * 4
    })
    readonly property var metrics: ({
        minPill: tabPad * 2 + iconSize, maxPill: BarLayout.unit * 5, gap: 2, medium: tiers.medium,
        arrow: Theme.fontSize + Theme.gapIn, chip: Theme.fontSize * 2 + Theme.gapIn * 2
    })
    readonly property var fit: Work.strip(ordered.length, room, metrics, focusedIndex, minimisedCount)
    readonly property int minimisedCount: ordered.filter(function (w) { return w.minimised; }).length
    readonly property int focusedIndex: {
        for (var i = 0; i < ordered.length; i++)
            if (ordered[i].focused)
                return i;
        return -1;
    }
    property int offset: 0
    readonly property int start: fit.mode === "scroll" ? Work.scrollOffset(offset, ordered.length, fit.shown, -1) : 0
    // What the strip's tabs draw: each window by address, and the addresses in order, the
    // latter replaced only when it really changes.
    readonly property var tabInfo: {
        var m = {};
        for (var i = 0; i < shownWindows.length; i++)
            m[shownWindows[i].address] = shownWindows[i];
        return m;
    }
    property var tabKeys: []
    onShownWindowsChanged: {
        var keys = shownWindows.map(function (w) { return w.address; });
        if (JSON.stringify(keys) !== JSON.stringify(tabKeys))
            tabKeys = keys;
    }
    readonly property var shownWindows: fit.mode === "pills" ? ordered
        : (fit.mode === "scroll" ? ordered.slice(start, start + fit.shown) : [])
    // A window out of view got focus (from the "+N" list, a key or a click on screen): centre
    // the strip on it, so its neighbours on screen show beside it.
    function keepFocusShown() {
        if (!fit || fit.mode !== "scroll" || focusedIndex < 0)
            return;
        if (focusedIndex < start || focusedIndex >= start + fit.shown)
            offset = Work.centreOffset(ordered.length, fit.shown, focusedIndex);
    }
    onFocusedIndexChanged: keepFocusShown()
    onFitChanged: keepFocusShown()
    readonly property var lead: focusedIndex >= 0 ? ordered[focusedIndex] : (ordered.length > 0 ? ordered[0] : null)

    Accessible.role: Accessible.PageTabList
    Accessible.name: ordered.length === 0 ? "No windows open" : ordered.length + (ordered.length === 1 ? " window open" : " windows open")

    WindowActions {
        id: windowActions
        root: root
    }
    function toplevelFor(address) { return windowActions.toplevelFor(address); }
    // Focus a window; a minimised one comes back to the workspace it left.
    function choose(w) { return windowActions.choose(w); }
    function act(w, name, args) { windowActions.act(w, name, args); }
    // A tiled window in the scrolling layout, sized by its column (not real fullscreen, 2).
    function inColumn(w) { return windowActions.inColumn(w); }
    // --- dragging tabs into a new order (the windows' columns follow) ---
    // The tab being dragged (its index among the tabs shown), -1 for none; how far it has moved;
    // its width; and where it would land.
    property int dragIndex: -1
    property real dragDx: 0
    property real dragWidth: 0
    property int dropIndex: -1
    // The tabs' left edges and widths as they were when the drag started.
    property var dragLefts: []
    property var dragWidths: []
    function startDrag(i) {
        var lefts = [], widths = [];
        for (var k = 0; k < row.tabsRep.count; k++) {
            var it = row.tabsRep.itemAt(k);
            lefts.push(it ? it.x : 0);
            widths.push(it ? it.width : 0);
        }
        dragLefts = lefts;
        dragWidths = widths;
        dragWidth = widths[i] || 0;
        dragDx = 0;
        dropIndex = i;
        dragIndex = i;
    }
    function moveDrag(dx) {
        dragDx = dx;
        var centre = dragLefts[dragIndex] + dragWidth / 2 + dx;
        dropIndex = Work.dropIndex(dragLefts, dragWidths, dragIndex, centre);
    }
    function cancelDrag() {
        dragIndex = -1;
        dropIndex = -1;
        dragDx = 0;
    }
    // Dropped: move the window's column as many places as the tab moved past other columns. The
    // tabs then follow the windows: in another sort, the island switches to screen order, so the
    // tab stays where it was dropped.
    function endDrag() {
        var off = fit.mode === "scroll" ? start : 0;
        var from = dragIndex + off, to = dropIndex + off;
        cancelDrag();
        var screenList = sortMode === "screen" ? ordered : Work.sortWindows(here, "screen", false,
            { lastOrder: memo.order });
        var mv = Work.screenMove(ordered, screenList, from, to);
        if (mv === null)
            return;
        var w = screenList[mv.from], steps = Work.columnSteps(screenList, mv.from, mv.to);
        if (sortMode !== "screen")
            BarLayout.setOptions("work", Object.assign({}, opts, { sort: "screen", descending: false }));
        if (steps === 0 || !w)
            return;
        Bridge.call("Dispatch", ["move_column", { address: w.address, steps: steps }])
            .catch(function (e) { console.warn("uq[WIN-001] move column: " + e); });
        refresh();
    }

    // Full: the window spans its screen. In the scrolling layout that is a maximized column
    // (Hyprland's own Full: fullscreen 1) or one as wide as the screen; elsewhere, maximized or
    // fullscreen.
    function isFull(w) { return windowActions.isFull(w); }
    // Make the window Full (the screen's width) or Half (half of it). Outside the scrolling
    // layout, Full maximizes and Half puts a maximized window back.
    function setFull(w, full) { windowActions.setFull(w, full); }
    function iconFor(cls) { return windowActions.iconFor(cls); }
    function togglePin(w) { windowActions.togglePin(w); }
    function isPinned(w) { return windowActions.isPinned(w); }

    function showList(anchor) {
        if (ordered.length === 0)
            return;
        if (listPopup.open && listPopup.anchorItem === anchor)
            listPopup.hide();
        else
            listPopup.show(anchor, listBody);
    }
    // A tab's X: minimise the window to its tab or close it, as the island's close option says,
    // asking first ("Minimize to Tab?") until an answer is kept. A minimised window's X closes it.
    function closePressed(anchor, w) {
        var a = Work.closeAction(root.closeMode, w.minimised);
        if (a === "ask") {
            askWindow = w;
            askPopup.show(anchor, askBody);
        } else {
            root.act(w, a);
        }
    }
    function closeLabel(w) {
        var a = Work.closeAction(root.closeMode, w.minimised);
        return (a === "close" ? "Close " : "Minimise ") + (w.title || w.cls);
    }
    // The window the "Minimize to Tab?" popup is about; the answer is kept as the close option.
    property var askWindow: null
    function answerAsk(mode) {
        var w = askWindow;
        askPopup.hide();
        askWindow = null;
        if (!w || mode === "")
            return;
        BarLayout.setOptions("work", Object.assign({}, root.opts, { close: mode }));
        root.act(w, mode === "minimize" ? "minimise" : "close");
    }

    // The window menu, for window `w` under `anchor`.
    property var menuWindow: null
    function openMenu(anchor, w) {
        menuWindow = w;
        menuPopup.show(anchor, menuBody);
    }

    Connections {
        target: Hyprland.toplevels
        function onValuesChanged() { root.rebuild(); }
    }
    HyprEvents {
        root: root
        titleTimer: titleTimer
    }
    Timer {
        id: titleTimer
        interval: 100
        onTriggered: root.rebuild()
    }
    // lastIpcObject (the class, tags, pid) arrives a moment after the refresh.
    RefreshSettled {
        root: root
    }
    // A new notification: once latest and count have both been updated.
    Connections {
        target: NotifBackend
        ignoreUnknownSignals: true
        function onLatestChanged() { Qt.callLater(root.checkNote); }
        function onCountChanged() { Qt.callLater(root.checkNote); }
    }
    // "By system usage": the windows' processes, every 5 s while that is the order.
    UsagePoll {
        root: root
    }

    WorkStrip {
        id: row
        root: root
    }

    // Every window here: click or Enter focuses one (or brings it back).
    LazyPopup {
        id: listPopup
    }
    Component {
        id: listBody
        WindowList {
            listPopup: listPopup
            root: root
        }
    }

    // "Minimize to Tab?": a tab's X, until an answer is kept (the island's close option).
    LazyPopup {
        id: askPopup
        onOpenChanged: if (!open) root.askWindow = null
    }
    Component {
        id: askBody
        CloseAsk {
            root: root
        }
    }

    // A window's menu (right click on its pill).
    LazyPopup {
        id: menuPopup
    }
    Component {
        id: menuBody
        WindowMenu {
            id: mcol
            menuPopup: menuPopup
            root: root
        }
    }
}
