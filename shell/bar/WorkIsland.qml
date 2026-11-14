import QtQuick
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
// run; the island's setting shell.bar.options.work can pick another order in edit mode). Windows
// minimised from this workspace keep their place, dimmed. The focused window's tab grows to
// Medium and the others share what is left. What a tab shows depends on its width (Work.tabTier):
// Icon, the app icon, which turns into a close button under the pointer; Small, the icon, Full or
// Half (whichever the window isn't, under the pointer) and close; Medium, the icon, Half, Full and
// close; Large, Medium plus the title (untrusted, elided). Full makes the window's column the
// screen's width and Half half of it (outside the scrolling layout, Full maximizes and Half puts
// it back). Past the narrowest tabs, arrows scroll the strip and "+N" lists every window; when a
// window out of view gets focus the strip centres on it. When not even one tab fits, the lead
// window's icon lists them all. Left click focuses a window (or brings a minimised one back),
// middle click closes it, right click opens its menu (minimise, full or half width, float, pin
// the app, move it, close it, and the uQ Island Menu).
Island {
    id: root
    property var screen: null
    readonly property real dpr: screen ? screen.devicePixelRatio : 1
    readonly property int iconSize: 20
    readonly property int cell: 26
    readonly property var opts: BarLayout.options.work || ({})
    readonly property string sortMode: Work.SORTS.indexOf(opts.sort) >= 0 ? opts.sort : Work.DEFAULT_SORT

    readonly property string monitorName: {
        var m = root.screen ? Hyprland.monitorFor(root.screen) : null;
        return m ? m.name : "";
    }
    readonly property int wsId: {
        var m = root.screen ? Hyprland.monitorFor(root.screen) : null;
        return m && m.activeWorkspace ? m.activeWorkspace.id : -1;
    }
    readonly property string focusedAddr: Hyprland.activeToplevel ? (Work.normAddr(Hyprland.activeToplevel.address) || "") : ""

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
        var out = [], vals = Hyprland.toplevels.values, seq = Object.assign({}, openedSeq);
        for (var i = 0; i < vals.length; i++) {
            var t = vals[i], o = t.lastIpcObject || ({});
            var wo = t.workspace && t.workspace.lastIpcObject ? t.workspace.lastIpcObject : null;
            var a = Work.normAddr(t.address);
            if (a !== null && seq[a] === undefined)
                seq[a] = nextSeq++;
            var minimised = t.workspace ? t.workspace.name === "special:minimized" : false;
            out.push({
                address: t.address,
                cls: o["class"] ? String(o["class"]) : "",
                title: t.title || "",
                wsId: t.workspace ? t.workspace.id : null,
                minimised: minimised,
                minimisedFrom: minimised ? Min.wsFromTags(o.tags) : null,
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
        }
    }
    property string windowsKey: ""
    Component.onCompleted: { rebuild(); refresh(); }
    // Set when this asked for fresh windows, so the settle after it rebuilds.
    property bool asked: false
    function refresh() {
        root.asked = true;
        HyprRefresh.request();
    }

    function appOf(cls) {
        var d = cls ? DesktopEntries.heuristicLookup(cls) : null;
        return { cls: cls, id: d ? String(d.id) : "", name: d && d.name ? String(d.name) : cls, icon: d && d.icon ? d.icon : "" };
    }
    readonly property var here: Work.windowsHere(windows, monitorName, wsId, focusedAddr)
    readonly property var ordered: {
        var ctx = { opened: openedSeq, activity: {}, pinIndex: {}, usage: {}, name: {}, lastOrder: memo.order };
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
    readonly property var memo: ({ order: [] })
    onOrderedChanged: memo.order = ordered.map(function (w) { return w.address; })

    // --- the strip ---
    readonly property real closeW: Theme.fontSize + Theme.gapIn
    // The narrowest width of each tab size (Work.tabTier): the icon with its margins, then a
    // button more each step, then room for a short title.
    readonly property real tabPad: 6
    readonly property var tiers: ({
        small: tabPad * 2 + iconSize + Theme.gapIn + closeW * 2 + 2,
        medium: tabPad * 2 + iconSize + Theme.gapIn + closeW * 3 + 4,
        large: tabPad * 2 + iconSize + Theme.gapIn * 2 + closeW * 3 + 4 + Theme.fontSize * 4
    })
    readonly property var metrics: ({
        minPill: tabPad * 2 + iconSize, maxPill: BarLayout.unit * 5, gap: 2, medium: tiers.medium,
        arrow: Theme.fontSize + Theme.gapIn, chip: Theme.fontSize * 2 + Theme.gapIn * 2
    })
    readonly property var fit: Work.strip(ordered.length, room, metrics, focusedIndex)
    readonly property int focusedIndex: {
        for (var i = 0; i < ordered.length; i++)
            if (ordered[i].focused)
                return i;
        return -1;
    }
    property int offset: 0
    readonly property int start: fit.mode === "scroll" ? Work.scrollOffset(offset, ordered.length, fit.shown, -1) : 0
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
        if (w.minimised) {
            var t = toplevelFor(w.address);
            var ws = t && t.lastIpcObject ? Min.wsFromTags(t.lastIpcObject.tags) : null;
            if (ws === null)
                ws = root.wsId > 0 ? root.wsId : 1;
            Bridge.call("Dispatch", ["restore", { address: w.address, to_ws: ws }]);
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
    }
    // A tiled window in the scrolling layout, sized by its column (not real fullscreen, 2).
    function inColumn(w) {
        return w.scrolling && !w.floating && w.fullscreen !== 2;
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
            Bridge.call("Dispatch", ["column_width", { address: w.address, width: full ? "full" : "half" }])
                .catch(function (e) { console.warn("uq: window width: " + e); });
        else if (full !== root.isFull(w))
            root.act(w, "fullscreen", { mode: "maximized" });
        // The scrolling layout resizes columns without an event.
        root.refresh();
    }
    function iconFor(cls) {
        if (cls === "")
            return Quickshell.iconPath("application-x-executable");
        var d = DesktopEntries.heuristicLookup(cls);
        var name = d && d.icon ? d.icon : cls.toLowerCase();
        return Quickshell.iconPath(name, true) || Quickshell.iconPath("application-x-executable");
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
    // Focus moved. Not the activewindowv2 event: Hyprland sends that again, for the same window,
    // on every title change of the focused window.
    Connections {
        target: Hyprland
        function onActiveToplevelChanged() { root.refresh(); }
    }
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "changefloatingmode" || event.name === "pin"
                    || event.name === "openwindow" || event.name === "closewindow"
                    || event.name === "movewindowv2" || event.name === "workspacev2")
                root.refresh();
            // Quickshell sets the title itself from this event: no need to ask Hyprland again.
            else if (event.name === "windowtitlev2")
                titleTimer.restart();
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
            root.rebuild();
        }
    }
    // "Last activity": every new notification, matched to its app's windows.
    Connections {
        target: NotifBackend
        ignoreUnknownSignals: true
        function onLatestChanged() {
            var l = NotifBackend.latest;
            if (!l || (!l.appName && !l.appIcon))
                return;
            var n = root.notes.slice(Math.max(0, root.notes.length - 49));
            n.push({ appName: String(l.appName || ""), appIcon: String(l.appIcon || ""), at: Date.now() });
            root.notes = n;
        }
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

    // A tab's small round button (close, Full, Half).
    component TabButton: Rectangle {
        id: tb
        property string glyph: ""
        property string label: ""
        // The window is already this way (Full on a Full window): drawn as on.
        property bool on: false
        signal pressed()
        Layout.preferredWidth: root.closeW
        Layout.preferredHeight: root.closeW
        radius: width / 2
        color: tbMouse.containsMouse ? Theme.overlay : "transparent"
        Accessible.role: Accessible.Button
        Accessible.name: label
        Accessible.onPressAction: tb.pressed()
        UqText {
            anchors.centerIn: parent
            text: tb.glyph
            // The way the window is now (Full or Half) in the accent colour.
            color: tb.on ? Theme.accent : (tbMouse.containsMouse ? Theme.text : Theme.textMuted)
        }
        MouseArea {
            id: tbMouse
            anchors.fill: parent
            hoverEnabled: true
            onClicked: tb.pressed()
        }
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
            model: root.shownWindows
            delegate: Rectangle {
                id: pill
                required property var modelData
                readonly property real tabW: modelData.focused ? root.fit.focusW : root.fit.pillW
                readonly property string tier: Work.tabTier(tabW, root.tiers)
                readonly property bool full: root.isFull(modelData)
                readonly property string name: modelData.title || modelData.cls
                Layout.preferredWidth: tabW
                Layout.fillHeight: true
                radius: Theme.radius - 2
                color: modelData.focused ? Theme.surfaceAlt : (pillHover.hovered ? Theme.overlay : "transparent")
                border.color: modelData.focused ? Theme.accent : "transparent"
                border.width: modelData.focused ? 1 : 0
                opacity: modelData.minimised ? 0.55 : 1
                Accessible.role: Accessible.PageTab
                Accessible.name: name + (modelData.minimised ? ", minimised" : "")
                    + (modelData.focused ? ", focused" : "") + (full ? ", full width" : "")
                Accessible.onPressAction: root.choose(modelData)
                Behavior on Layout.preferredWidth {
                    enabled: !Settings.reduceMotion
                    NumberAnimation { duration: 120; easing.type: Easing.OutCubic }
                }

                HoverHandler {
                    id: pillHover
                }
                MouseArea {
                    id: pillMouse
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
                    onClicked: function (m) {
                        if (m.button === Qt.MiddleButton)
                            root.act(pill.modelData, "close");
                        else if (m.button === Qt.RightButton)
                            root.openMenu(pill, pill.modelData);
                        else
                            root.choose(pill.modelData);
                    }
                }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: pill.tier === "icon" ? 0 : root.tabPad
                    anchors.rightMargin: pill.tier === "icon" ? 0 : 2
                    spacing: Theme.gapIn
                    // Icon size: the icon, centred, turns into the close button under the pointer.
                    Item {
                        Layout.preferredWidth: root.iconSize
                        Layout.preferredHeight: root.iconSize
                        Layout.alignment: Qt.AlignVCenter | (pill.tier === "icon" ? Qt.AlignHCenter : Qt.AlignLeft)
                        Layout.fillWidth: pill.tier === "icon"
                        Layout.maximumWidth: root.iconSize
                        IconImage {
                            anchors.fill: parent
                            visible: !(pill.tier === "icon" && iconClose.containsMouse)
                            implicitSize: root.iconSize
                            backer.sourceSize.width: root.iconSize * root.dpr
                            backer.sourceSize.height: root.iconSize * root.dpr
                            source: root.iconFor(pill.modelData.cls)
                        }
                        Rectangle {
                            anchors.fill: parent
                            visible: pill.tier === "icon" && iconClose.containsMouse
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
                            enabled: pill.tier === "icon"
                            hoverEnabled: true
                            Accessible.role: Accessible.Button
                            Accessible.name: "Close " + pill.name
                            onClicked: root.act(pill.modelData, "close")
                        }
                    }
                    UqText {
                        visible: pill.tier === "large"
                        Layout.fillWidth: true
                        text: pill.name
                        color: pill.modelData.focused ? Theme.text : Theme.textMuted
                    }
                    Item {
                        visible: pill.tier === "small" || pill.tier === "medium"
                        Layout.fillWidth: true
                    }
                    // Small: whichever the window isn't, under the pointer (its place is kept).
                    TabButton {
                        visible: pill.tier === "small" && !pill.modelData.minimised
                        opacity: pillHover.hovered ? 1 : 0
                        enabled: pillHover.hovered
                        glyph: pill.full ? "◧" : "□"
                        label: (pill.full ? "Half width: " : "Full width: ") + pill.name
                        onPressed: root.setFull(pill.modelData, !pill.full)
                    }
                    TabButton {
                        visible: (pill.tier === "medium" || pill.tier === "large") && !pill.modelData.minimised
                        glyph: "◧"
                        label: "Half width: " + pill.name
                        on: !pill.full
                        onPressed: root.setFull(pill.modelData, false)
                    }
                    TabButton {
                        visible: (pill.tier === "medium" || pill.tier === "large") && !pill.modelData.minimised
                        glyph: "□"
                        label: "Full width: " + pill.name
                        on: pill.full
                        onPressed: root.setFull(pill.modelData, true)
                    }
                    TabButton {
                        visible: pill.tier !== "icon"
                        glyph: "✕"
                        label: "Close " + pill.name
                        onPressed: root.act(pill.modelData, "close")
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
            Accessible.name: (root.ordered.length - root.fit.shown) + " more windows; list them all"
            UqText {
                anchors.centerIn: parent
                text: "+" + (root.ordered.length - root.fit.shown)
                color: Theme.textMuted
                elide: Text.ElideNone
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
                    detail: modelData.focused ? "Focused" : (modelData.minimised ? "Minimised" : "")
                    onActivated: {
                        root.choose(modelData);
                        listPopup.hide();
                    }
                    Component.onCompleted: if (index === 0) forceActiveFocus()
                }
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
