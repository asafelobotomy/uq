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
    readonly property var here: Work.windowsHere(windows, monitorName, wsId, focusedAddr)
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

    function toplevelFor(address) {
        var vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++)
            if (Work.normAddr(vals[i].address) === address)
                return vals[i];
        return null;
    }
    // Focus a window; a minimised one comes back to the workspace it left.
    function choose(w) {
        var kids = w.children || [];
        if (!w.minimised && kids.length > 0) {
            // A window with a dialog open: its page comes into view, and the dialog (the last
            // one opened) gets focus, as it is the one waiting for an answer.
            var dialog = kids[kids.length - 1];
            Bridge.call("Dispatch", ["focus_window", { address: w.address, keep_pointer: true }])
                .then(function () {
                    return Bridge.call("Dispatch", ["focus_window", { address: dialog, keep_pointer: true }]);
                }).catch(function (e) { console.warn("uq: focus a window's dialog: " + e); });
            return;
        }
        if (w.minimised) {
            var t = toplevelFor(w.address);
            var ws = t && t.lastIpcObject ? Min.wsFromTags(t.lastIpcObject.tags) : null;
            if (ws === null)
                ws = root.wsId > 0 ? root.wsId : 1;
            // At the end of the queue: after the last window on the workspace (the scrolling
            // layout puts it next to the focused one).
            var last = Work.lastLive(root.windows, ws, w.address);
            var args = { address: w.address, to_ws: ws };
            if (last !== null)
                args.after = last;
            Bridge.call("Dispatch", ["restore", args]);
            // Its dialogs come back with it, after it.
            kids.forEach(function (c) {
                Bridge.call("Dispatch", ["restore", { address: c, to_ws: ws }]).catch(function () {});
            });
        } else if (!w.focused) {
            // The pointer stays put; a window out of view (a scrolled-away column) is brought
            // into view by the focus itself.
            Bridge.call("Dispatch", ["focus_window", { address: w.address, keep_pointer: true }]);
        }
    }
    function act(w, name, args) {
        var d = Work.buildDispatch(name, { addr: w.address, ws: root.wsId }, args);
        if (d !== null)
            Bridge.call("Dispatch", d);
        // Minimised or moved to another workspace, its dialogs go with it.
        if (name === "minimise" || name === "move")
            (w.children || []).forEach(function (c) {
                var dc = Work.buildDispatch(name, { addr: c, ws: root.wsId }, args);
                if (dc !== null)
                    Bridge.call("Dispatch", dc);
            });
    }
    // A tiled window in the scrolling layout, sized by its column (not real fullscreen, 2).
    function inColumn(w) {
        return w.scrolling && !w.floating && w.fullscreen !== 2;
    }
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
        for (var k = 0; k < tabsRep.count; k++) {
            var it = tabsRep.itemAt(k);
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
            .catch(function (e) { console.warn("uq: move column: " + e); });
        refresh();
    }

    // Full: the window spans its screen. In the scrolling layout that is a maximized column
    // (Hyprland's own Full: fullscreen 1) or one as wide as the screen; elsewhere, maximized or
    // fullscreen.
    function isFull(w) {
        return root.inColumn(w) ? (w.fullscreen === 1 || w.full === true) : w.fullscreen > 0;
    }
    // Make the window Full (the screen's width) or Half (half of it). Outside the scrolling
    // layout, Full maximizes and Half puts a maximized window back.
    function setFull(w, full) {
        if (w.minimised)
            return;
        if (root.inColumn(w))
            PageActions.setWidth(w.address, full)
                .catch(function (e) { console.warn("uq: window width: " + e); });
        else if (full !== root.isFull(w))
            root.act(w, "fullscreen", { mode: "maximized" });
        // The scrolling layout resizes columns without an event.
        root.refresh();
    }
    function iconFor(cls) {
        if (cls === "")
            return Quickshell.iconPath("application-x-executable", true);
        var a = root.appOf(cls);
        var name = a.icon ? a.icon : cls.toLowerCase();
        return Quickshell.iconPath(name, true) || Quickshell.iconPath("application-x-executable", true);
    }
    function togglePin(w) {
        var pins = Dock.togglePin(Settings.values.pinned || [], appOf(w.cls).id, Settings.loaded);
        if (pins !== null)
            Bridge.call("SetSetting", ["pinned", pins]);
    }
    function isPinned(w) {
        return (Settings.values.pinned || []).indexOf(appOf(w.cls).id) >= 0;
    }

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
    Connections {
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
    Timer {
        id: titleTimer
        interval: 100
        onTriggered: root.rebuild()
    }
    // lastIpcObject (the class, tags, pid) arrives a moment after the refresh.
    Connections {
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
    // A new notification: once latest and count have both been updated.
    Connections {
        target: NotifBackend
        ignoreUnknownSignals: true
        function onLatestChanged() { Qt.callLater(root.checkNote); }
        function onCountChanged() { Qt.callLater(root.checkNote); }
    }
    // "By system usage": the windows' processes, every 5 s while that is the order.
    Timer {
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

    // A tab's small round button: Half, Full or close. The marks are drawn, not font glyphs (whose
    // sizes and baselines differ from font to font), all in the same box, so they sit level.
    component TabButton: Rectangle {
        id: tb
        // "half", "full" or "close".
        property string mark: "close"
        property string label: ""
        // The window is already this way (Full on a Full window): drawn as on.
        property bool on: false
        signal pressed()
        implicitWidth: root.closeW
        implicitHeight: root.closeW
        radius: width / 2
        color: tbMouse.containsMouse ? Theme.overlay : "transparent"
        Accessible.role: Accessible.Button
        Accessible.name: label
        Accessible.onPressAction: tb.pressed()
        // The way the window is now (Full or Half) in the accent colour.
        readonly property color ink: tb.on ? Theme.accent : (tbMouse.containsMouse ? Theme.text : Theme.textMuted)
        readonly property real box: Math.round(root.closeW * 0.46)
        readonly property real line: Math.max(1, Math.round(root.closeW / 14))
        Item {
            width: tb.box
            height: tb.box
            anchors.centerIn: parent
            // Half and Full: a window's outline, Half with its left half filled.
            Rectangle {
                visible: tb.mark !== "close"
                anchors.fill: parent
                radius: tb.line
                color: "transparent"
                border.width: tb.line
                border.color: tb.ink
            }
            Rectangle {
                visible: tb.mark === "half"
                width: Math.round(parent.width / 2)
                height: parent.height
                radius: tb.line
                color: tb.ink
            }
            // Close: two crossed strokes.
            Repeater {
                model: tb.mark === "close" ? [45, -45] : []
                delegate: Rectangle {
                    required property real modelData
                    anchors.centerIn: parent
                    width: parent.width * 1.2
                    height: tb.line * 1.5
                    radius: height / 2
                    rotation: modelData
                    color: tb.ink
                }
            }
        }
        MouseArea {
            id: tbMouse
            anchors.fill: parent
            hoverEnabled: true
            onClicked: tb.pressed()
        }
    }

    // The dot on a window's icon when it wants attention: the theme's accent, ringed in the
    // island's colour so it stands off any icon. Sits on its parent's top right corner.
    component AlertDot: Rectangle {
        property bool shown: false
        visible: shown
        width: Math.round(root.iconSize * 0.45)
        height: width
        radius: width / 2
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.rightMargin: -Math.round(width / 4)
        anchors.topMargin: -Math.round(width / 4)
        color: Theme.accent
        border.width: Math.max(1, Math.round(width / 5))
        border.color: Theme.surface
        z: 1
    }

    component Arrow: Rectangle {
        id: arrow
        property string glyph: ""
        property string label: ""
        property bool can: true
        signal pressed()
        Layout.preferredWidth: root.metrics.arrow
        Layout.fillHeight: true
        radius: Theme.radius - 2
        opacity: can ? 1 : 0.35
        color: arrowMouse.containsMouse && can ? Theme.surfaceAlt : "transparent"
        Accessible.role: Accessible.Button
        Accessible.name: label
        UqText {
            anchors.centerIn: parent
            text: arrow.glyph
        }
        MouseArea {
            id: arrowMouse
            anchors.fill: parent
            hoverEnabled: true
            enabled: arrow.can
            onClicked: arrow.pressed()
        }
    }

    RowLayout {
        id: row
        anchors.fill: parent
        spacing: root.metrics.gap

        // No windows here.
        UqText {
            visible: root.fit.mode === "empty" && !root.isIcon
            text: "Desktop"
            color: Theme.textMuted
            Layout.fillWidth: true
        }
        IconImage {
            visible: root.fit.mode === "empty" && root.isIcon
            Layout.alignment: Qt.AlignCenter
            implicitSize: root.iconSize
            source: Quickshell.iconPath("user-desktop", true)
        }

        // Not even one pill fits: the lead window's icon (and the count, when there's room).
        Rectangle {
            id: compact
            visible: root.fit.mode === "compact"
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Theme.radius - 2
            color: compactMouse.containsMouse ? Theme.surfaceAlt : "transparent"
            Accessible.role: Accessible.Button
            Accessible.name: root.ordered.length + " windows open; list them"
            Row {
                anchors.centerIn: parent
                spacing: 2
                IconImage {
                    anchors.verticalCenter: parent.verticalCenter
                    implicitSize: root.iconSize
                    source: root.lead ? root.iconFor(root.lead.cls) : ""
                    AlertDot { shown: root.alertHere }
                }
                UqText {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: root.room >= root.iconSize + Theme.fontSize * 2
                    text: String(root.ordered.length)
                    color: Theme.textMuted
                }
            }
            MouseArea {
                id: compactMouse
                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                onClicked: function (m) {
                    if (m.button === Qt.RightButton && root.lead)
                        root.openMenu(compact, root.lead);
                    else
                        root.showList(compact);
                }
            }
        }

        Arrow {
            visible: root.fit.mode === "scroll"
            glyph: "‹"
            label: "Scroll the windows left"
            can: root.start > 0
            onPressed: root.offset = root.start - 1
        }

        Repeater {
            id: tabsRep
            // The windows' addresses: replaced only when windows come, go or change places, so
            // tabs aren't rebuilt (and their widths re-animated) every time a window moves.
            model: root.tabKeys
            delegate: Rectangle {
                id: pill
                required property var modelData
                required property int index
                // This tab's window, as last read.
                // The placeholder (tabKeys can run ahead of tabInfo for a moment) has every field
                // a binding reads as a bool, so none of them gets undefined.
                readonly property var win: root.tabInfo[modelData]
                    || ({ address: modelData, title: "", cls: "", focused: false, minimised: false })
                // Dragging: this tab follows the pointer; the tabs it passes step aside.
                transform: Translate {
                    x: root.dragIndex < 0 ? 0
                        : pill.index === root.dragIndex ? root.dragDx
                        : (pill.index > root.dragIndex && pill.index <= root.dropIndex) ? -(root.dragWidth + root.metrics.gap)
                        : (pill.index < root.dragIndex && pill.index >= root.dropIndex) ? root.dragWidth + root.metrics.gap
                        : 0
                    Behavior on x {
                        enabled: !Settings.reduceMotion && pill.index !== root.dragIndex
                        NumberAnimation { duration: Motion.base; easing.type: Motion.easing }
                    }
                }
                z: pill.index === root.dragIndex ? 1 : 0
                // A minimised window's tab is an icon, greyed, at the end of the queue.
                readonly property real tabW: win.minimised ? root.metrics.minPill
                    : (win.focused ? root.fit.focusW : root.fit.pillW)
                readonly property string tier: win.minimised ? "icon" : Work.tabTier(tabW, root.tiers)
                readonly property bool full: root.isFull(win)
                readonly property string name: win.title || win.cls
                Layout.preferredWidth: tabW
                Layout.fillHeight: true
                radius: Theme.radius - 2
                color: win.focused ? Theme.surfaceAlt : (pillHover.hovered ? Theme.overlay : "transparent")
                border.color: win.focused ? Theme.accent : "transparent"
                border.width: win.focused ? 1 : 0
                opacity: win.minimised ? 0.7 : 1
                Accessible.role: Accessible.PageTab
                readonly property bool alert: root.alerts[modelData] === true
                Accessible.name: name + (win.minimised ? ", minimised" : "")
                    + (win.focused ? ", focused" : "") + (full ? ", full width" : "")
                    + (alert ? ", wants attention" : "")
                Accessible.onPressAction: root.choose(win)
                Behavior on Layout.preferredWidth {
                    enabled: !Settings.reduceMotion
                    NumberAnimation { duration: Motion.base; easing.type: Motion.easing }
                }

                HoverHandler {
                    id: pillHover
                }
                // At icon size the title shows in a tooltip under the pointer.
                Timer {
                    id: tipDelay
                    interval: 350
                    running: pillHover.hovered && pill.tier === "icon" && root.dragIndex < 0
                    onTriggered: pill.tipShown = true
                }
                property bool tipShown: false
                Connections {
                    target: pillHover
                    function onHoveredChanged() { if (!pillHover.hovered) pill.tipShown = false; }
                }
                Loader {
                    active: pill.tipShown && pill.tier === "icon" && pill.name !== ""
                    sourceComponent: PopupWindow {
                        anchor.item: pill
                        anchor.edges: Edges.Bottom
                        anchor.gravity: Edges.Bottom
                        implicitWidth: tipText.implicitWidth + Theme.gapIn * 2
                        implicitHeight: tipText.implicitHeight + Theme.gapIn * 2
                        color: "transparent"
                        visible: true
                        Rectangle {
                            anchors.fill: parent
                            color: Theme.surface
                            radius: Theme.radius
                            border.color: Theme.border
                            border.width: Theme.borderWidth
                            UqText {
                                id: tipText
                                anchors.centerIn: parent
                                width: Math.min(implicitWidth, 320)
                                text: pill.name + (pill.win.minimised ? " (minimised)" : "")
                            }
                        }
                    }
                }
                MouseArea {
                    id: pillMouse
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
                    property real pressX: 0
                    property real pressY: 0
                    property bool dragged: false
                    // Dragged below the bar, onto the screen: dropped there, the window joins the
                    // page shown (PageActions); along the bar, the tabs are reordered.
                    property bool overScreen: false
                    property real screenX: 0
                    // Where in the tab it was taken hold of, so the floating copy keeps that spot
                    // under the pointer.
                    property real holdX: 0
                    property real holdY: 0
                    cursorShape: dragged && pressed ? Qt.ClosedHandCursor : Qt.ArrowCursor
                    onPressed: function (m) {
                        var p = mapToItem(null, m.x, m.y);
                        pressX = p.x;
                        pressY = p.y;
                        holdX = m.x;
                        holdY = m.y;
                        dragged = false;
                        overScreen = false;
                    }
                    onPositionChanged: function (m) {
                        if (!(pressedButtons & Qt.LeftButton))
                            return;
                        var reorder = Work.canDrag(pill.win), drop = Work.canDropOnScreen(pill.win);
                        if (!reorder && !drop)
                            return;
                        var p = mapToItem(null, m.x, m.y), dx = p.x - pressX, dy = p.y - pressY;
                        if (!dragged && Math.max(Math.abs(dx), Math.abs(dy)) < Theme.gapIn * 2)
                            return;
                        if (!dragged) {
                            dragged = true;
                            if (reorder)
                                root.startDrag(pill.index);
                        }
                        var bar = pill.QsWindow.window;
                        var below = bar !== null && p.y > bar.height + Theme.gapIn;
                        overScreen = drop && below;
                        screenX = p.x + WindowLook.edgeLeft;
                        // Off the bar (or where the tab can't follow), a see-through copy of the
                        // tab rides under the pointer; over the screen, where the window would go
                        // is outlined (PageActions, drawn by ScreenPages).
                        PageActions.dropHint = below || !reorder ? {
                            screen: root.screen ? root.screen.name : "", address: pill.win.address,
                            x: screenX, y: p.y + WindowLook.gapTop, drop: overScreen,
                            holdX: holdX, holdY: holdY, width: pill.width, height: pill.height,
                            title: pill.name, icon: root.iconFor(pill.win.cls)
                        } : null;
                        if (reorder)
                            root.moveDrag(overScreen ? 0 : dx);
                    }
                    onReleased: {
                        if (!dragged)
                            return;
                        PageActions.dropHint = null;
                        if (overScreen) {
                            root.cancelDrag();
                            PageActions.drop(root.screen ? root.screen.name : "", pill.win.address, screenX);
                        } else if (root.dragIndex >= 0) {
                            root.endDrag();
                        }
                        overScreen = false;
                    }
                    onCanceled: {
                        PageActions.dropHint = null;
                        overScreen = false;
                        if (dragged)
                            root.cancelDrag();
                    }
                    onClicked: function (m) {
                        if (dragged)
                            return;
                        if (m.button === Qt.MiddleButton)
                            root.act(pill.win, "close");
                        else if (m.button === Qt.RightButton)
                            root.openMenu(pill, pill.win);
                        else
                            root.choose(pill.win);
                    }
                }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: pill.tier === "icon" ? 0 : root.tabPad
                    anchors.rightMargin: pill.tier === "icon" ? 0 : 2
                    spacing: Theme.gapIn
                    // Icon size: the icon, centred, turns into the close button under the pointer.
                    Item {
                        id: iconSlot
                        // Focused tab only. The × waits for the pointer to rest on the icon, so passing over a
                        // tab doesn't hide which app it is, and a quick click can't close it.
                        property bool closeShown: false
                        Timer {
                            interval: 400
                            running: iconClose.containsMouse && pill.tier === "icon" && pill.win.focused && !pill.win.minimised
                            onTriggered: iconSlot.closeShown = true
                        }
                        Connections {
                            target: iconClose
                            function onContainsMouseChanged() { if (!iconClose.containsMouse) iconSlot.closeShown = false; }
                        }
                        Layout.preferredWidth: root.iconSize
                        Layout.preferredHeight: root.iconSize
                        Layout.alignment: Qt.AlignVCenter | (pill.tier === "icon" ? Qt.AlignHCenter : Qt.AlignLeft)
                        Layout.fillWidth: pill.tier === "icon"
                        Layout.maximumWidth: root.iconSize
                        IconImage {
                            anchors.fill: parent
                            visible: !(pill.tier === "icon" && iconSlot.closeShown)
                            implicitSize: root.iconSize
                            backer.sourceSize.width: root.iconSize * root.dpr
                            backer.sourceSize.height: root.iconSize * root.dpr
                            source: root.iconFor(pill.win.cls)
                            layer.enabled: pill.win.minimised === true
                            layer.effect: MultiEffect {
                                saturation: -1.0
                            }
                        }
                        AlertDot {
                            shown: pill.alert && !(pill.tier === "icon" && iconSlot.closeShown)
                        }
                        Rectangle {
                            anchors.fill: parent
                            visible: pill.tier === "icon" && iconSlot.closeShown
                            radius: width / 2
                            color: Theme.overlay
                            UqText {
                                anchors.centerIn: parent
                                text: "✕"
                            }
                        }
                        MouseArea {
                            id: iconClose
                            anchors.fill: parent
                            // Only the focused tab's icon turns into a close button; an unfocused or minimised
                            // tab's icon never does, so clicking it just focuses or restores the window.
                            enabled: pill.tier === "icon" && pill.win.focused && !pill.win.minimised
                            hoverEnabled: true
                            // Before the × shows, a click goes on to the tab (it focuses the window).
                            propagateComposedEvents: true
                            Accessible.role: Accessible.Button
                            Accessible.name: root.closeLabel(pill.win)
                            onClicked: mouse => {
                                if (iconSlot.closeShown)
                                    root.closePressed(pill, pill.win);
                                else
                                    mouse.accepted = false;
                            }
                        }
                    }
                    UqText {
                        visible: pill.tier !== "icon"
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: pill.name
                        color: pill.win.focused ? Theme.text : Theme.textMuted
                    }
                    // The buttons, kept together at the right end, 2 px apart: the widths in
                    // root.tiers allow one gap between the icon (or title) and this group, and
                    // 2 px between buttons, so a tab at its narrowest still holds them all.
                    Item {
                        visible: pill.tier !== "icon"
                        Layout.fillWidth: pill.tier !== "large"
                        Layout.fillHeight: true
                        Layout.preferredWidth: buttonRow.implicitWidth
                        Layout.minimumWidth: buttonRow.implicitWidth
                        Row {
                            id: buttonRow
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 2
                            // Small: whichever the window isn't, under the pointer (its place is kept).
                            TabButton {
                                visible: pill.tier === "small" && !pill.win.minimised
                                opacity: pillHover.hovered ? 1 : 0
                                enabled: pillHover.hovered
                                mark: pill.full ? "half" : "full"
                                label: (pill.full ? "Half width: " : "Full width: ") + pill.name
                                onPressed: root.setFull(pill.win, !pill.full)
                            }
                            TabButton {
                                visible: (pill.tier === "medium" || pill.tier === "large") && !pill.win.minimised
                                mark: "half"
                                label: "Half width: " + pill.name
                                on: !pill.full
                                onPressed: root.setFull(pill.win, false)
                            }
                            TabButton {
                                visible: (pill.tier === "medium" || pill.tier === "large") && !pill.win.minimised
                                mark: "full"
                                label: "Full width: " + pill.name
                                on: pill.full
                                onPressed: root.setFull(pill.win, true)
                            }
                            TabButton {
                                id: closeButton
                                visible: pill.tier !== "icon"
                                mark: "close"
                                label: root.closeLabel(pill.win)
                                onPressed: root.closePressed(closeButton, pill.win)
                            }
                        }
                    }
                }
            }
        }

        Arrow {
            visible: root.fit.mode === "scroll"
            glyph: "›"
            label: "Scroll the windows right"
            can: root.start + root.fit.shown < root.ordered.length
            onPressed: root.offset = root.start + 1
        }
        // "+N": every window, in a list.
        Rectangle {
            id: moreChip
            visible: root.fit.mode === "scroll"
            Layout.preferredWidth: root.metrics.chip
            Layout.fillHeight: true
            radius: Theme.radius - 2
            color: moreMouse.containsMouse ? Theme.surfaceAlt : "transparent"
            Accessible.role: Accessible.Button
            Accessible.name: (root.ordered.length - root.fit.shown) + " more windows"
                + (root.alertHidden ? ", one wants attention" : "") + "; list them all"
            UqText {
                anchors.centerIn: parent
                text: "+" + (root.ordered.length - root.fit.shown)
                color: Theme.textMuted
                elide: Text.ElideNone
            }
            AlertDot {
                shown: root.alertHidden
                anchors.rightMargin: 1
                anchors.topMargin: 1
            }
            MouseArea {
                id: moreMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: root.showList(moreChip)
            }
        }
        // Pills that don't fill the island leave the rest empty.
        Item {
            visible: root.fit.mode === "pills"
            Layout.fillWidth: true
        }
    }

    // Every window here: click or Enter focuses one (or brings it back).
    LazyPopup {
        id: listPopup
    }
    Component {
        id: listBody
        ColumnLayout {
            spacing: 2
            width: Theme.fontSize * 26
            UqText {
                text: root.ordered.length === 1 ? "1 window on this workspace" : root.ordered.length + " windows on this workspace"
                color: Theme.textMuted
            }
            Repeater {
                model: root.ordered
                delegate: MenuRow {
                    required property var modelData
                    required property int index
                    Layout.fillWidth: true
                    text: modelData.title || modelData.cls
                    detail: modelData.focused ? "Focused"
                        : (root.alerts[modelData.address] === true ? "Wants attention"
                        : (modelData.minimised ? "Minimised" : ""))
                    onActivated: {
                        root.choose(modelData);
                        listPopup.hide();
                    }
                    Component.onCompleted: if (index === 0) forceActiveFocus()
                }
            }
        }
    }

    // "Minimize to Tab?": a tab's X, until an answer is kept (the island's close option).
    LazyPopup {
        id: askPopup
        onOpenChanged: if (!open) root.askWindow = null
    }
    Component {
        id: askBody
        ColumnLayout {
            spacing: 2
            width: Theme.fontSize * 20
            UqText {
                text: "Minimize to Tab?"
                baseBold: true
                Layout.fillWidth: true
            }
            UqText {
                text: root.askWindow ? (root.askWindow.title || root.askWindow.cls) : ""
                color: Theme.textMuted
                Layout.fillWidth: true
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Always minimize instead of close"
                onActivated: root.answerAsk("minimize")
                Component.onCompleted: forceActiveFocus()
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Never minimize instead of close"
                onActivated: root.answerAsk("close")
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Cancel"
                onActivated: root.answerAsk("")
            }
            UqText {
                text: "Change it later in this island's settings."
                color: Theme.textMuted
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
        }
    }

    // A window's menu (right click on its pill).
    LazyPopup {
        id: menuPopup
    }
    Component {
        id: menuBody
        ColumnLayout {
            id: mcol
            spacing: 2
            width: Theme.fontSize * 20
            readonly property var w: root.menuWindow
            readonly property var tl: w ? root.toplevelFor(w.address) : null
            readonly property var ipc: tl && tl.lastIpcObject ? tl.lastIpcObject : ({})
            function done() { menuPopup.hide(); }

            UqText {
                text: mcol.w ? (mcol.w.title || mcol.w.cls) : ""
                baseBold: true
                Layout.fillWidth: true
            }
            MenuRow {
                id: firstRow
                Layout.fillWidth: true
                text: mcol.w && mcol.w.minimised ? "Restore" : "Minimise"
                onActivated: {
                    if (mcol.w.minimised)
                        root.choose(mcol.w);
                    else
                        root.act(mcol.w, "minimise");
                    mcol.done();
                }
                Component.onCompleted: forceActiveFocus()
            }
            MenuRow {
                visible: mcol.w !== null && !mcol.w.minimised
                Layout.fillWidth: true
                text: mcol.w && root.isFull(mcol.w) ? "Half width" : "Full width"
                onActivated: { root.setFull(mcol.w, !root.isFull(mcol.w)); mcol.done(); }
            }
            MenuRow {
                visible: mcol.w !== null && !mcol.w.minimised
                Layout.fillWidth: true
                text: mcol.ipc["floating"] === true ? "Tile" : "Float"
                onActivated: { root.act(mcol.w, "toggle_float"); mcol.done(); }
            }
            MenuRow {
                visible: mcol.w !== null && Dock.canPin({ id: root.appOf(mcol.w.cls).id || "class:" })
                    && root.appOf(mcol.w.cls).id !== ""
                Layout.fillWidth: true
                text: mcol.w && root.isPinned(mcol.w) ? "Unpin the app" : "Pin the app"
                onActivated: { root.togglePin(mcol.w); mcol.done(); }
            }
            UqText {
                visible: mcol.w !== null && !mcol.w.minimised
                text: "Move to workspace"
                color: Theme.textMuted
            }
            Flow {
                visible: mcol.w !== null && !mcol.w.minimised
                Layout.fillWidth: true
                spacing: 2
                Repeater {
                    model: Work.wsChoices()
                    delegate: MenuRow {
                        required property int modelData
                        text: String(modelData)
                        Accessible.name: "Move to workspace " + modelData
                        onActivated: { root.act(mcol.w, "move", { workspace: modelData }); mcol.done(); }
                    }
                }
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Close"
                detail: "Middle click"
                onActivated: { root.act(mcol.w, "close"); mcol.done(); }
            }
            MenuRow {
                Layout.fillWidth: true
                Layout.topMargin: Theme.gapIn
                text: "uQ Island Menu…"
                onActivated: {
                    mcol.done();
                    root.menuRequested(root);
                }
            }
        }
    }
}
