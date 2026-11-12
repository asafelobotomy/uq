import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import "../core"
import "../logic/minimise.js" as Min
import "../logic/work.js" as Work

// Open windows on this bar's screen: one button each (icon and title, untrusted and elided), in
// Hyprland's order. Clicking one focuses it (or brings a minimised one back); the focused one
// also has the window buttons and the dropdown. Past Work.MAX_TABS windows the island folds to
// one icon-sized button, the focused app's icon and the count; clicking it shows every window
// again until one is chosen or it is folded back. With no windows it shows "Desktop".
Island {
    id: root
    property var screen: null
    property real barWidth: screen ? screen.width : 1280
    readonly property real dpr: screen ? screen.devicePixelRatio : 1
    readonly property int iconSize: 20
    readonly property int cell: 26

    readonly property var tl: Hyprland.activeToplevel
    readonly property bool hasWindow: tl !== null && tl !== undefined
    readonly property string addr: hasWindow ? (Work.normAddr(tl.address) || "") : ""
    readonly property bool actionable: addr !== ""
    readonly property var ipc: hasWindow && tl.lastIpcObject ? tl.lastIpcObject : ({})
    readonly property string cls: ipc["class"] ? String(ipc["class"]) : ""
    readonly property var fromWs: hasWindow ? Work.minimiseFrom(tl.workspace) : null
    readonly property bool isFloating: ipc["floating"] === true
    readonly property bool isPinned: ipc["pinned"] === true

    implicitWidth: row.implicitWidth + Theme.gapIn * 2

    // Every open window, as logic/work.js reads them; rebuilt when Hyprland's list changes.
    property var windows: []
    property bool expanded: false
    readonly property string monitorName: {
        var m = root.screen ? Hyprland.monitorFor(root.screen) : null;
        return m ? m.name : "";
    }
    readonly property var view: Work.taskView(windows, monitorName, addr, expanded)
    // Fold again once there are few enough windows to show them all anyway.
    onViewChanged: if (expanded && view.count <= Work.MAX_TABS) expanded = false

    function rebuild() {
        var out = [], vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++) {
            var t = vals[i], o = t.lastIpcObject || ({});
            out.push({
                address: t.address,
                cls: o["class"] ? String(o["class"]) : "",
                title: t.title || "",
                workspace: t.workspace ? t.workspace.name : "",
                minimised: t.workspace ? t.workspace.name === "special:minimized" : false,
                monitor: t.monitor ? t.monitor.name : ""
            });
        }
        windows = out;
    }
    Component.onCompleted: { rebuild(); refreshTimer.restart(); }

    function toplevelFor(address) {
        var vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++)
            if (Work.normAddr(vals[i].address) === address)
                return vals[i];
        return null;
    }
    // Focus a window from its button; a minimised one comes back to the workspace it left.
    function choose(tab) {
        if (tab.minimised) {
            var t = toplevelFor(tab.address);
            var ws = t && t.lastIpcObject ? Min.wsFromTags(t.lastIpcObject.tags) : null;
            if (ws === null)
                ws = Hyprland.focusedWorkspace ? Hyprland.focusedWorkspace.id : 1;
            Bridge.call("Dispatch", ["restore", { address: tab.address, to_ws: ws }]);
        } else if (!tab.focused) {
            Bridge.call("Dispatch", ["focus_window", { address: tab.address }]);
        }
        expanded = false;
    }

    function iconFor(cls) {
        if (cls === "")
            return Quickshell.iconPath("application-x-executable");
        var d = DesktopEntries.heuristicLookup(cls);
        var name = d && d.icon ? d.icon : cls.toLowerCase();
        return Quickshell.iconPath(name, true) || Quickshell.iconPath("application-x-executable");
    }
    // The menu acts on the window captured when it opened, never on the live focus.
    property var menuTarget: null   // { addr, ws, floating, pinned }
    function dispatch(name, args) {
        var d = Work.buildDispatch(name, { addr: addr, ws: tl && tl.workspace ? tl.workspace.id : null }, args);
        if (d !== null)
            Bridge.call("Dispatch", d);
    }
    function menuDispatch(name, args) {
        var d = Work.buildDispatch(name, menuTarget, args);
        if (d !== null)
            Bridge.call("Dispatch", d);
        menu.visible = false;
    }
    function openMenu(item) {
        if (!actionable)
            return;
        menuTarget = { addr: addr, ws: tl.workspace ? tl.workspace.id : null,
            floating: isFloating, pinned: isPinned };
        menu.anchor.item = item;
        menu.visible = true;
    }
    function menuTargetAlive() {
        if (!menuTarget)
            return false;
        var vals = Hyprland.toplevels.values;
        for (var i = 0; i < vals.length; i++)
            if (Work.normAddr(vals[i].address) === menuTarget.addr)
                return true;
        return false;
    }
    function checkMenu() {
        if (menu.visible && (!actionable || !menuTargetAlive()))
            menu.visible = false;
    }
    onActionableChanged: checkMenu()
    Connections {
        target: Hyprland.toplevels
        function onValuesChanged() { root.checkMenu(); root.rebuild(); }
    }

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "activewindowv2" || event.name === "windowtitlev2"
                    || event.name === "changefloatingmode" || event.name === "pin"
                    || event.name === "openwindow" || event.name === "closewindow"
                    || event.name === "movewindowv2")
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
    // lastIpcObject (the class) arrives a moment after the refresh.
    Timer {
        id: rebuildTimer
        interval: 120
        onTriggered: root.rebuild()
    }

    RowLayout {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        spacing: 2

        UqText {
            visible: root.view.count === 0
            text: "Desktop"
            color: Theme.textMuted
        }

        // Folded: one icon-sized button, the focused app's icon (else the first window's) and how
        // many windows are open.
        Rectangle {
            id: foldBtn
            visible: root.view.folded
            readonly property var lead: {
                var ts = root.view.tabs;
                for (var i = 0; i < ts.length; i++)
                    if (ts[i].focused)
                        return ts[i];
                return ts.length > 0 ? ts[0] : null;
            }
            Layout.preferredWidth: foldRow.implicitWidth + 8
            Layout.preferredHeight: root.cell
            radius: Theme.radius - 2
            color: foldMouse.containsMouse ? Theme.surfaceAlt : "transparent"
            Accessible.role: Accessible.Button
            Accessible.name: root.view.count + " windows open; show them"
            RowLayout {
                id: foldRow
                anchors.centerIn: parent
                spacing: 2
                IconImage {
                    implicitSize: root.iconSize
                    Layout.preferredWidth: root.iconSize
                    Layout.preferredHeight: root.iconSize
                    backer.sourceSize.width: root.iconSize * root.dpr
                    backer.sourceSize.height: root.iconSize * root.dpr
                    source: foldBtn.lead ? root.iconFor(foldBtn.lead.cls) : ""
                }
                UqText {
                    text: String(root.view.count)
                    color: Theme.textMuted
                }
            }
            MouseArea {
                id: foldMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: root.expanded = true
            }
        }

        Repeater {
            model: root.view.folded ? [] : root.view.tabs
            delegate: Rectangle {
                id: tab
                required property var modelData
                readonly property bool isFocused: modelData.focused
                Layout.preferredHeight: root.cell
                Layout.preferredWidth: tabRow.implicitWidth + 8
                radius: Theme.radius - 2
                color: tab.isFocused ? Theme.surfaceAlt : (tabMouse.containsMouse ? Theme.surfaceAlt : "transparent")
                opacity: tab.modelData.minimised ? 0.6 : 1
                Accessible.role: Accessible.Button
                Accessible.name: (tab.modelData.title || tab.modelData.cls) + (tab.modelData.minimised ? ", minimised" : "")

                MouseArea {
                    id: tabMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: root.choose(tab.modelData)
                }

                RowLayout {
                    id: tabRow
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.left: parent.left
                    anchors.leftMargin: 4
                    spacing: Theme.gapIn

                    IconImage {
                        implicitSize: root.iconSize
                        Layout.preferredWidth: root.iconSize
                        Layout.preferredHeight: root.iconSize
                        backer.sourceSize.width: root.iconSize * root.dpr
                        backer.sourceSize.height: root.iconSize * root.dpr
                        source: root.iconFor(tab.modelData.cls)
                    }
                    UqText {
                        Layout.maximumWidth: Work.titleWidth(root.barWidth, root.view.count)
                        elide: Text.ElideRight
                        text: tab.modelData.title || tab.modelData.cls
                        color: tab.isFocused ? Theme.text : Theme.textMuted
                    }

                    // The focused window's buttons.
                    Repeater {
                        model: tab.isFocused && root.actionable ? [
                            { glyph: "▾", tip: "menu", kind: "menu" },
                            { glyph: "–", tip: "Minimise", kind: "min" },
                            { glyph: "□", tip: "Maximise", kind: "max" },
                            { glyph: "✕", tip: "Close", kind: "close" }
                        ].filter(function (b) { return b.kind !== "min" || root.fromWs !== null; }) : []
                        delegate: Rectangle {
                            id: btn
                            required property var modelData
                            Layout.preferredWidth: root.cell
                            Layout.preferredHeight: root.cell
                            radius: Theme.radius - 2
                            color: btnMouse.containsMouse ? Theme.surface : "transparent"
                            Accessible.role: Accessible.Button
                            Accessible.name: btn.modelData.tip
                            UqText {
                                anchors.centerIn: parent
                                text: btn.modelData.glyph
                                color: Theme.text
                            }
                            MouseArea {
                                id: btnMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                onClicked: {
                                    switch (btn.modelData.kind) {
                                    case "menu":
                                        if (menu.visible)
                                            menu.visible = false;
                                        else
                                            root.openMenu(btn);
                                        break;
                                    case "min":
                                        root.dispatch("minimise", { from_ws: root.fromWs });
                                        break;
                                    case "max":
                                        root.dispatch("fullscreen", { mode: "maximized" });
                                        break;
                                    case "close":
                                        root.dispatch("close");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Shown while a folded island is open: fold it back.
        Rectangle {
            visible: root.expanded && root.view.count > Work.MAX_TABS
            Layout.preferredWidth: root.cell
            Layout.preferredHeight: root.cell
            radius: Theme.radius - 2
            color: unfoldMouse.containsMouse ? Theme.surfaceAlt : "transparent"
            Accessible.role: Accessible.Button
            Accessible.name: "Show fewer"
            UqText {
                anchors.centerIn: parent
                text: "‹"
                color: Theme.text
            }
            MouseArea {
                id: unfoldMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: root.expanded = false
            }
        }
    }

    PopupWindow {
        id: menu
        anchor.window: root.QsWindow.window
        anchor.edges: Edges.Bottom
        anchor.gravity: Edges.Bottom
        implicitWidth: 200
        implicitHeight: col.implicitHeight + Theme.gapIn * 2
        color: "transparent"
        visible: false

        Rectangle {
            anchors.fill: parent
            color: Theme.surface
            radius: Theme.radius
            border.color: Theme.border
            border.width: Theme.borderWidth

            ColumnLayout {
                id: col
                anchors.fill: parent
                anchors.margins: Theme.gapIn
                spacing: 2

                Repeater {
                    model: [
                        { label: (root.menuTarget && root.menuTarget.floating) ? "Tile" : "Float", act: "toggle_float", args: {} },
                        { label: (root.menuTarget && root.menuTarget.pinned) ? "Unpin" : "Pin", act: "pin", args: {} },
                        { label: "Fullscreen", act: "fullscreen", args: { mode: "full" } }
                    ]
                    delegate: Rectangle {
                        id: item
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.preferredHeight: root.cell
                        radius: Theme.radius - 2
                        color: itemMouse.containsMouse ? Theme.surfaceAlt : "transparent"
                        UqText {
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: parent.left
                            anchors.leftMargin: Theme.gapIn
                            text: item.modelData.label
                        }
                        MouseArea {
                            id: itemMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: {
                                root.menuDispatch(item.modelData.act, item.modelData.args);
                            }
                        }
                    }
                }

                UqText {
                    text: "Move to workspace"
                    color: Theme.textMuted
                    Layout.leftMargin: Theme.gapIn
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 2
                    Repeater {
                        model: Work.wsChoices()
                        delegate: Rectangle {
                            id: wsBtn
                            required property int modelData
                            width: 34
                            height: root.cell
                            radius: Theme.radius - 2
                            color: wsMouse.containsMouse ? Theme.surfaceAlt : "transparent"
                            UqText {
                                anchors.centerIn: parent
                                text: String(wsBtn.modelData)
                            }
                            MouseArea {
                                id: wsMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                onClicked: {
                                    root.menuDispatch("move", { workspace: wsBtn.modelData });
                                }
                            }
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: root.cell
                    radius: Theme.radius - 2
                    color: closeMouse.containsMouse ? Theme.surfaceAlt : "transparent"
                    UqText {
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.leftMargin: Theme.gapIn
                        text: "Close"
                    }
                    MouseArea {
                        id: closeMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: {
                            root.menuDispatch("close");
                        }
                    }
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
