import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import "../core"
import "islands"
import "../logic/minimise.js" as Min
import "../logic/work.js" as Work

// Open windows on this bar's screen, about the focused one (else the first). Icon: its app's
// icon. Small: and how many windows are open. Medium: its icon and title (untrusted, elided)
// and "+N" for the others. Large: and, when it is focused, the window buttons and dropdown.
// At Icon and Small a click lists every window; at Medium and Large the title focuses it (or
// brings a minimised one back) and "+N" lists the others. With no windows: "Desktop".
Island {
    id: root
    property var screen: null
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


    // Every open window, as logic/work.js reads them; rebuilt when Hyprland's list changes.
    property var windows: []
    readonly property string monitorName: {
        var m = root.screen ? Hyprland.monitorFor(root.screen) : null;
        return m ? m.name : "";
    }
    readonly property var view: Work.taskView(windows, monitorName, addr, true)

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

    // The window the island is about: the focused one, else the first.
    readonly property var lead: {
        var ts = view.tabs;
        for (var i = 0; i < ts.length; i++)
            if (ts[i].focused)
                return ts[i];
        return ts.length > 0 ? ts[0] : null;
    }
    readonly property int others: Math.max(0, view.count - 1)
    readonly property bool withTitle: size === "medium" || size === "large"

    function showList(anchor) {
        if (view.count === 0)
            return;
        if (windowList.open && windowList.anchorItem === anchor)
            windowList.hide();
        else
            windowList.show(anchor, windowListBody);
    }

    RowLayout {
        id: row
        anchors.fill: parent
        spacing: 2

        UqText {
            visible: root.view.count === 0 && !root.isIcon
            text: "Desktop"
            color: Theme.textMuted
            Layout.fillWidth: true
        }
        IconImage {
            visible: root.view.count === 0 && root.isIcon
            Layout.alignment: Qt.AlignCenter
            implicitSize: root.iconSize
            source: Quickshell.iconPath("user-desktop", true)
        }

        // The lead window: its icon (Icon and Small, with the count at Small), or its icon and
        // title (Medium and Large, with the window buttons at Large when it is focused).
        Rectangle {
            id: tab
            visible: root.lead !== null
            readonly property bool isFocused: root.lead !== null && root.lead.focused
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Theme.radius - 2
            color: tab.isFocused && root.withTitle ? Theme.surfaceAlt : (tabMouse.containsMouse ? Theme.surfaceAlt : "transparent")
            opacity: root.lead && root.lead.minimised ? 0.6 : 1
            Accessible.role: Accessible.Button
            Accessible.name: root.lead ? (root.lead.title || root.lead.cls) + (root.lead.minimised ? ", minimised" : "")
                + (root.view.count > 1 ? ", " + root.view.count + " windows open" : "") : ""

            MouseArea {
                id: tabMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: root.withTitle ? root.choose(root.lead) : root.showList(tab)
            }

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: root.isIcon ? 0 : 4
                anchors.rightMargin: root.isIcon ? 0 : 4
                spacing: Theme.gapIn

                IconImage {
                    implicitSize: root.iconSize
                    Layout.preferredWidth: root.iconSize
                    Layout.preferredHeight: root.iconSize
                    Layout.alignment: root.isIcon ? Qt.AlignCenter : Qt.AlignVCenter
                    backer.sourceSize.width: root.iconSize * root.dpr
                    backer.sourceSize.height: root.iconSize * root.dpr
                    source: root.lead ? root.iconFor(root.lead.cls) : ""
                }
                UqText {
                    visible: root.size === "small"
                    text: String(root.view.count)
                    color: Theme.textMuted
                    Layout.fillWidth: true
                }
                UqText {
                    visible: root.withTitle
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    text: root.lead ? (root.lead.title || root.lead.cls) : ""
                    color: tab.isFocused ? Theme.text : Theme.textMuted
                }

                // The focused window's buttons, at Large.
                Repeater {
                    model: root.size === "large" && tab.isFocused && root.actionable ? [
                        { glyph: "▾", tip: "Window menu", kind: "menu" },
                        { glyph: "–", tip: "Minimise", kind: "min" },
                        { glyph: "□", tip: "Maximise", kind: "max" },
                        { glyph: "✕", tip: "Close", kind: "close" }
                    ].filter(function (b) { return b.kind !== "min" || root.fromWs !== null; }) : []
                    delegate: Rectangle {
                        id: btn
                        required property var modelData
                        Layout.preferredWidth: Theme.fontSize + Theme.gapIn * 2
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

        // "+N": the other windows, listed in a popup (Medium and Large).
        Rectangle {
            id: moreChip
            visible: root.withTitle && root.others > 0
            Layout.preferredWidth: moreText.implicitWidth + Theme.gapIn * 2
            Layout.fillHeight: true
            radius: Theme.radius - 2
            color: moreMouse.containsMouse ? Theme.surfaceAlt : "transparent"
            Accessible.role: Accessible.Button
            Accessible.name: root.others + " more windows; show them"
            UqText {
                id: moreText
                anchors.centerIn: parent
                text: "+" + root.others
                color: Theme.textMuted
            }
            MouseArea {
                id: moreMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: root.showList(moreChip)
            }
        }
    }

    // Every open window on this screen: click or Enter focuses one (or brings a minimised one
    // back). Titles are untrusted: plain text only.
    LazyPopup {
        id: windowList
    }
    Component {
        id: windowListBody
        ColumnLayout {
            spacing: 2
            width: Theme.fontSize * 26
            UqText {
                text: root.view.count === 1 ? "1 window" : root.view.count + " windows"
                color: Theme.textMuted
            }
            Repeater {
                model: root.view.tabs
                delegate: MenuRow {
                    required property var modelData
                    required property int index
                    Layout.fillWidth: true
                    text: modelData.title || modelData.cls
                    detail: modelData.focused ? "Focused" : (modelData.minimised ? "Minimised" : "")
                    onActivated: {
                        root.choose(modelData);
                        windowList.hide();
                    }
                    Component.onCompleted: if (index === 0) forceActiveFocus()
                }
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
