import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import "../core"
import "../logic/dock.js" as Dock
import "../logic/minimise.js" as Min

// App-menu button (inert until slice 2), workspaces, and the merged pinned/running dock.
Island {
    id: root
    property var screen: null
    readonly property real dpr: screen ? screen.devicePixelRatio : 1
    readonly property int iconSize: 22
    readonly property int cell: 28

    // "<id>" or "name:<name>" -> layout name
    property var layouts: ({})
    property var dockModel: []
    property var menuEntry: null
    property bool needRefresh: false
    readonly property var refreshEvents: ["openwindow", "closewindow", "movewindow", "movewindowv2",
        "windowtitle", "windowtitlev2", "changefloatingmode", "activewindowv2", "workspace", "workspacev2"]

    implicitWidth: row.implicitWidth + Theme.gapIn * 2

    // --- helpers ---
    function hexAddr(a) {
        var s = String(a);
        return s.indexOf("0x") === 0 ? s : "0x" + s;
    }
    function layoutFor(ws) {
        return layouts[String(ws.id)] || layouts["name:" + ws.name] || "dwindle";
    }
    function layoutGlyph(l) {
        switch (l) {
        case "master": return "▥";
        case "scrolling": return "▤";
        case "monocle": return "▣";
        default: return "◩";
        }
    }
    function sortedWorkspaces() {
        var out = [];
        var vals = Hyprland.workspaces.values;
        for (var i = 0; i < vals.length; i++)
            if (vals[i].id > 0)
                out.push(vals[i]);
        out.sort(function (a, b) { return a.id - b.id; });
        return out;
    }
    function lookupId(cls) {
        var d = DesktopEntries.heuristicLookup(cls);
        return d ? d.id : null;
    }
    function rebuildDock() {
        var tls = [], vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++) {
            var t = vals[i], o = t.lastIpcObject || ({});
            tls.push({
                address: hexAddr(t.address),
                cls: o["class"] ? String(o["class"]) : "",
                title: t.title,
                workspace: t.workspace ? t.workspace.name : "",
                minimised: t.workspace ? t.workspace.name === "special:minimized" : false,
                monitor: t.monitor ? t.monitor.name : ""
            });
        }
        var es = Dock.entries(Settings.values.pinned || [], tls, lookupId);
        es.forEach(function (e) {
            var d = e.id.indexOf("class:") === 0 ? null : DesktopEntries.byId(e.id);
            e.icon = d && d.icon ? d.icon : (e.id.indexOf("class:") === 0 ? e.id.substring(6).toLowerCase() : e.id);
        });
        dockModel = es;
    }
    function refreshLayouts() {
        Bridge.call("GetLayouts", []).then(function (m) {
            if (m && typeof m === "object")
                root.layouts = m;
        }).catch(function () {});
    }
    function toplevelFor(address) {
        var vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++)
            if (hexAddr(vals[i].address) === address)
                return vals[i];
        return null;
    }
    function activate(entry) {
        if (!entry.running) {
            var lid = Dock.launchId(entry);
            if (lid !== null)
                Bridge.call("Launch", [{ desktop_id: lid }]);
        } else if (entry.minimisedOnly) {
            var addr = entry.windows[0];
            var t = toplevelFor(addr);
            var ws = t && t.lastIpcObject ? Min.wsFromTags(t.lastIpcObject.tags) : null;
            if (ws === null)
                ws = Hyprland.focusedWorkspace ? Hyprland.focusedWorkspace.id : 1;
            Bridge.call("Dispatch", ["restore", { address: addr, to_ws: ws }]);
        } else {
            var cur = Hyprland.activeToplevel ? [hexAddr(Hyprland.activeToplevel.address)] : [];
            var next = Dock.nextWindow(entry, cur);
            if (next)
                Bridge.call("Dispatch", ["focus_window", { address: next }]);
        }
    }
    function togglePin(entry) {
        var pins = Dock.togglePin(Settings.values.pinned || [], entry.id, Settings.loaded);
        if (pins !== null)
            Bridge.call("SetSetting", ["pinned", pins]);
    }

    Component.onCompleted: { rebuildDock(); refreshLayouts(); }

    Timer {
        id: debounce
        interval: 60
        onTriggered: {
            if (root.needRefresh) {
                root.needRefresh = false;
                Hyprland.refreshToplevels();   // lastIpcObject (class, tags) is only re-read on refresh
            }
            root.rebuildDock();
        }
    }
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (root.refreshEvents.indexOf(event.name) >= 0) {
                root.needRefresh = true;
                debounce.restart();
            }
        }
    }
    Connections {
        target: Hyprland.toplevels
        function onValuesChanged() { debounce.restart(); }
    }
    Connections {
        target: Settings
        function onValuesChanged() { debounce.restart(); }
    }
    Connections {
        target: Bridge
        function onConnectedChanged() { if (Bridge.connected) root.refreshLayouts(); }
        function onLayoutChanged(ws, layout) {
            var m = Object.assign({}, root.layouts);
            m[String(ws)] = layout;
            root.layouts = m;
        }
    }

    RowLayout {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        spacing: Theme.gapIn

        // App menu: inert until slice 2.
        Rectangle {
            Layout.preferredWidth: root.cell
            Layout.preferredHeight: root.cell
            radius: Theme.radius - 2
            color: Theme.surfaceAlt
            UqText {
                anchors.centerIn: parent
                text: "☰"
                color: Theme.textMuted
            }
        }

        Repeater {
            model: root.sortedWorkspaces()
            delegate: Rectangle {
                required property var modelData
                Layout.preferredWidth: wsRow.implicitWidth + Theme.gapIn * 2
                Layout.preferredHeight: root.cell
                radius: Theme.radius - 2
                color: modelData.focused ? Theme.accent : (wsMouse.containsMouse ? Theme.surfaceAlt : "transparent")
                Row {
                    id: wsRow
                    anchors.centerIn: parent
                    spacing: 3
                    UqText {
                        text: String(modelData.id)
                        color: modelData.focused ? Theme.accentText : Theme.text
                    }
                    UqText {
                        text: root.layoutGlyph(root.layoutFor(modelData))
                        color: modelData.focused ? Theme.accentText : Theme.textMuted
                    }
                }
                MouseArea {
                    id: wsMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: Bridge.call("Dispatch", ["focus_workspace", { id: modelData.id }])
                }
            }
        }

        Rectangle {
            Layout.preferredWidth: 1
            Layout.preferredHeight: root.cell * 0.6
            color: Theme.border
            visible: root.dockModel.length > 0
        }

        Repeater {
            model: root.dockModel
            delegate: Item {
                id: dockItem
                required property var modelData
                Layout.preferredWidth: root.cell
                Layout.preferredHeight: root.cell

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.radius - 2
                    color: dockMouse.containsMouse ? Theme.surfaceAlt : "transparent"
                }
                IconImage {
                    anchors.centerIn: parent
                    width: root.iconSize
                    height: root.iconSize
                    implicitSize: root.iconSize
                    backer.sourceSize.width: root.iconSize * root.dpr
                    backer.sourceSize.height: root.iconSize * root.dpr
                    opacity: dockItem.modelData.minimisedOnly ? 0.5 : 1
                    source: Quickshell.iconPath(dockItem.modelData.icon, true)
                        || Quickshell.iconPath("application-x-executable")
                }
                // running indicator
                Rectangle {
                    visible: dockItem.modelData.running
                    width: 10; height: 2; radius: 1
                    color: Theme.accent
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.bottom: parent.bottom
                }
                MouseArea {
                    id: dockMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onClicked: function (mouse) {
                        if (mouse.button === Qt.RightButton) {
                            if (!Dock.canPin(dockItem.modelData) || !Settings.loaded)
                                return;
                            root.menuEntry = dockItem.modelData;
                            menu.anchor.item = dockItem;
                            menu.visible = true;
                        } else {
                            root.activate(dockItem.modelData);
                        }
                    }
                }
            }
        }
    }

    PopupWindow {
        id: menu
        anchor.window: root.QsWindow.window
        anchor.edges: Edges.Bottom
        anchor.gravity: Edges.Bottom
        implicitWidth: 120
        implicitHeight: 32
        color: "transparent"

        Rectangle {
            anchors.fill: parent
            color: Theme.surface
            radius: Theme.radius
            border.color: Theme.border
            border.width: Theme.borderWidth
            UqText {
                anchors.centerIn: parent
                text: root.menuEntry && root.menuEntry.pinned ? "Unpin" : "Pin"
            }
            MouseArea {
                anchors.fill: parent
                onClicked: {
                    root.togglePin(root.menuEntry);
                    menu.visible = false;
                }
            }
        }
    }
    HyprlandFocusGrab {
        windows: [menu]
        active: menu.visible
        onCleared: menu.visible = false
    }
}
