import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import "../core"
import "../notif"
import "islands"
import "../logic/minimise.js" as Min
import "../logic/work.js" as Work
import "../logic/dock.js" as Dock

// Open windows (`work`): one pill per window on this screen's current workspace, like browser
// tabs: its app's icon, its title (untrusted, elided) and a close button. Windows minimised from
// this workspace show here too, dimmed. As windows open the pills share the island evenly and
// shrink, down to the icon and close button; past that, arrows scroll the strip and "+N" lists
// every window. When not even one pill fits (Icon size), the lead window's icon lists them all.
// Left click focuses a window (or brings a minimised one back), middle click closes it, right
// click opens its menu (minimise, maximise, float, pin the app, move it, close it, and the uQ
// Island Menu). The order is the island's setting (shell.bar.options.work, chosen in edit mode).
Island {
    id: root
    property var screen: null
    readonly property real dpr: screen ? screen.devicePixelRatio : 1
    readonly property int iconSize: 20
    readonly property int cell: 26
    readonly property var opts: BarLayout.options.work || ({})
    readonly property string sortMode: Work.SORTS.indexOf(opts.sort) >= 0 ? opts.sort : "opened"

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
                pid: typeof o.pid === "number" ? o.pid : 0
            });
        }
        openedSeq = seq;
        windows = out;
    }
    Component.onCompleted: { rebuild(); refreshTimer.restart(); }

    function appOf(cls) {
        var d = cls ? DesktopEntries.heuristicLookup(cls) : null;
        return { cls: cls, id: d ? String(d.id) : "", name: d && d.name ? String(d.name) : cls, icon: d && d.icon ? d.icon : "" };
    }
    readonly property var here: Work.windowsHere(windows, monitorName, wsId, focusedAddr)
    readonly property var ordered: {
        var ctx = { opened: openedSeq, activity: {}, pinIndex: {}, usage: {}, name: {} };
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

    // --- the strip ---
    readonly property real closeW: Theme.fontSize + Theme.gapIn
    readonly property var metrics: ({
        minPill: 8 + iconSize + Theme.gapIn + closeW, maxPill: BarLayout.unit * 5, gap: 2,
        arrow: Theme.fontSize + Theme.gapIn, chip: Theme.fontSize * 2 + Theme.gapIn * 2
    })
    readonly property var fit: Work.strip(ordered.length, room, metrics)
    readonly property bool showTitles: fit.pillW >= metrics.minPill + Theme.fontSize * 3
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
    onFocusedIndexChanged: if (fit.mode === "scroll") offset = Work.scrollOffset(offset, ordered.length, fit.shown, focusedIndex)
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
            Bridge.call("Dispatch", ["focus_window", { address: w.address }]);
        }
    }
    function act(w, name, args) {
        var d = Work.buildDispatch(name, { addr: w.address, ws: root.wsId }, args);
        if (d !== null)
            Bridge.call("Dispatch", d);
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
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "activewindowv2" || event.name === "windowtitlev2"
                    || event.name === "changefloatingmode" || event.name === "pin"
                    || event.name === "openwindow" || event.name === "closewindow"
                    || event.name === "movewindowv2" || event.name === "workspacev2")
                refreshTimer.restart();
        }
    }
    Timer {
        id: refreshTimer
        interval: 60
        onTriggered: {
            Hyprland.refreshToplevels();
            rebuildTimer.restart();
        }
    }
    // lastIpcObject (the class, tags, pid) arrives a moment after the refresh.
    Timer {
        id: rebuildTimer
        interval: 120
        onTriggered: root.rebuild()
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
                Layout.preferredWidth: root.fit.pillW
                Layout.fillHeight: true
                radius: Theme.radius - 2
                color: modelData.focused ? Theme.surfaceAlt : (pillMouse.containsMouse ? Theme.overlay : "transparent")
                border.color: modelData.focused ? Theme.accent : "transparent"
                border.width: modelData.focused ? 1 : 0
                opacity: modelData.minimised ? 0.55 : 1
                Accessible.role: Accessible.PageTab
                Accessible.name: (modelData.title || modelData.cls) + (modelData.minimised ? ", minimised" : "")
                    + (modelData.focused ? ", focused" : "")
                Accessible.onPressAction: root.choose(modelData)

                MouseArea {
                    id: pillMouse
                    anchors.fill: parent
                    hoverEnabled: true
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
                    anchors.leftMargin: 4
                    anchors.rightMargin: 2
                    spacing: Theme.gapIn
                    IconImage {
                        implicitSize: root.iconSize
                        Layout.preferredWidth: root.iconSize
                        Layout.preferredHeight: root.iconSize
                        backer.sourceSize.width: root.iconSize * root.dpr
                        backer.sourceSize.height: root.iconSize * root.dpr
                        source: root.iconFor(pill.modelData.cls)
                    }
                    UqText {
                        visible: root.showTitles
                        Layout.fillWidth: true
                        text: pill.modelData.title || pill.modelData.cls
                        color: pill.modelData.focused ? Theme.text : Theme.textMuted
                    }
                    Item {
                        visible: !root.showTitles
                        Layout.fillWidth: true
                    }
                    Rectangle {
                        Layout.preferredWidth: root.closeW
                        Layout.preferredHeight: root.closeW
                        radius: width / 2
                        color: closeMouse.containsMouse ? Theme.overlay : "transparent"
                        Accessible.role: Accessible.Button
                        Accessible.name: "Close " + (pill.modelData.title || pill.modelData.cls)
                        UqText {
                            anchors.centerIn: parent
                            text: "✕"
                            color: closeMouse.containsMouse ? Theme.text : Theme.textMuted
                        }
                        MouseArea {
                            id: closeMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: root.act(pill.modelData, "close")
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
                font.bold: true
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
                text: "Maximise"
                onActivated: { root.act(mcol.w, "fullscreen", { mode: "maximized" }); mcol.done(); }
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
