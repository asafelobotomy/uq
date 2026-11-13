pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import "../core"
import "../session"
import "../logic/desktopmenu.js" as D
import "../logic/appmenu.js" as M
import "../logic/quickui.js" as U

// The desktop menu: a right click on empty desktop space (the wallpaper layer, Wallpaper.qml)
// opens it at the pointer, on that screen. It is the "desktopmenu" surface in Surfaces, so it
// closes when another surface opens, on lock, on Esc, and on a click outside (on its own screen
// through the full-screen catcher, on any other through HyprlandFocusGrab).
//
//   Create workspace +   the lowest free workspace id, focused (Dispatch focus_workspace)
//   Open ›               the default terminal, file manager and text editor (DefaultApps,
//                        launched through Launch), and uQ Settings
//   Appearance ›         Change Wallpaper (Settings' Wallpaper page), Light Mode, Dark Mode
//                        (SetTheme, as quick settings' toggle; the current one is ticked)
//   System ›             Log Off, Lock, Restart, Shutdown: what the app menu's power buttons do
//                        (appmenu.js powerClick), so Restart and Shutdown open the power menu at
//                        its confirm countdown
//
// Keyboard: Up/Down move, Right/Enter/Space open a submenu, Left/Esc leave it, Enter/Space run
// an entry, Esc closes. Clicks and keys only ever make bridge calls, `uqctl settings` or
// Session.lock().
PanelWindow {
    id: win

    readonly property bool shown: Surfaces.open === "desktopmenu" && !Bridge.locked
    // Where it opened: the screen and the click on it. Kept while shown.
    property var openedOn: null
    property real px: 0
    property real py: 0

    // DefaultApps' reply, or null until it comes (or when it fails).
    property var apps: null
    readonly property var top: D.entries()
    // The open submenu ("" for none), the row it hangs from, and its entries.
    property string sub: ""
    property int subFrom: -1
    readonly property var subList: D.subEntries(sub, apps, Theme.variant, Bridge.uqdUp)
    // Keyboard and hover selection: the level (0 the menu, 1 the submenu) and an index in each.
    property int level: 0
    property int mainIndex: -1
    property int subIndex: -1

    visible: shown && openedOn !== null
    screen: openedOn
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "uq-desktop-menu"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    exclusionMode: ExclusionMode.Ignore
    anchors { top: true; bottom: true; left: true; right: true }

    function reset() {
        sub = "";
        subFrom = -1;
        level = 0;
        mainIndex = -1;
        subIndex = -1;
    }

    onShownChanged: {
        reset();
        if (!shown) {
            openedOn = null;
            return;
        }
        var p = Surfaces.anchor;
        openedOn = Surfaces.screen;
        px = p && typeof p.x === "number" ? p.x : 0;
        py = p && typeof p.y === "number" ? p.y : 0;
        Bridge.call("DefaultApps", []).then(function (v) {
            win.apps = v && typeof v === "object" ? v : null;
        }).catch(function (e) {
            console.warn("uq: DefaultApps failed: " + e);
            win.apps = null;
        });
        Qt.callLater(function () { body.forceActiveFocus(); });
    }

    function openSub(i, withKeys) {
        var e = top[i];
        if (!e || !e.sub) {
            sub = "";
            subFrom = -1;
            return;
        }
        mainIndex = i;
        if (sub !== e.sub) {
            sub = e.sub;
            subFrom = i;
            subIndex = -1;
        }
        if (withKeys) {
            level = 1;
            subIndex = D.first(subList);
        }
    }

    function closeSub() {
        level = 0;
        sub = "";
        subFrom = -1;
        subIndex = -1;
    }

    function activate(e) {
        if (!e || !D.enabled(e) || !shown)
            return;
        if (e.sub) {
            openSub(top.indexOf(e), true);
            return;
        }
        var screen = openedOn;
        switch (e.action) {
        case "workspace":
            var ids = Hyprland.workspaces.values.map(function (w) { return w.id; });
            Surfaces.close();
            Bridge.call("Dispatch", ["focus_workspace", { id: D.newWorkspaceId(ids) }]).catch(function (err) {
                console.warn("uq: creating a workspace failed: " + err);
            });
            break;
        case "launch":
            Surfaces.close();
            Bridge.call("Launch", [{ desktop_id: e.appId }]).catch(function (err) {
                console.warn("uq: launching " + e.appId + " failed: " + err);
            });
            break;
        case "settings":
            Surfaces.close();
            Quickshell.execDetached(["uqctl", "settings"]);
            break;
        case "wallpaper":
            Surfaces.close();
            Quickshell.execDetached(["uqctl", "settings", "wallpaper"]);
            break;
        case "variant":
            Surfaces.close();
            setVariant(e.variant);
            break;
        case "power":
            power(e.power, screen);
            break;
        }
    }

    // Light or Dark Mode: the current theme in the other variant, as quick settings does.
    function setVariant(want) {
        Bridge.call("GetTheme", []).then(function (v) {
            var t = U.themeReply(v);
            if (t === null || t.variant === want)
                return;
            return Bridge.call("SetTheme", [t.name, want]);
        }).catch(function (err) {
            console.warn("uq: switching to " + want + " mode failed: " + err);
        });
    }

    // The app menu's power buttons: lock through Session.lock(), log off at once, restart and
    // shut down through the power menu's confirm countdown on this screen.
    function power(action, screen) {
        switch (M.powerClick(action)) {
        case "lock":
            Surfaces.close();
            Session.lock();
            break;
        case "fire":
            Surfaces.close();
            Bridge.call("Power", [action]).catch(function (err) {
                console.warn("uq: Power " + action + " failed: " + err);
            });
            break;
        case "powermenu":
            Surfaces.toggle("powermenu", screen, null, action);
            break;
        }
    }

    function key(k) {
        var list = level === 1 ? subList : top;
        var idx = level === 1 ? subIndex : mainIndex;
        switch (k) {
        case "down":
        case "up":
            var n = D.step(list, idx, k === "down" ? 1 : -1);
            if (level === 1) {
                subIndex = n;
            } else {
                mainIndex = n;
                if (sub !== "" && (n < 0 || top[n].sub !== sub))
                    closeSub();
            }
            break;
        case "right":
            if (level === 0 && mainIndex >= 0 && top[mainIndex].sub)
                openSub(mainIndex, true);
            break;
        case "left":
            if (level === 1)
                closeSub();
            break;
        case "enter":
            if (level === 1)
                activate(subList[subIndex]);
            else if (mainIndex >= 0)
                activate(top[mainIndex]);
            else
                mainIndex = D.first(top);
            break;
        case "escape":
            if (level === 1)
                closeSub();
            else
                Surfaces.close();
            break;
        }
    }

    Connections {
        target: Quickshell
        // Close if our monitor was unplugged.
        function onScreensChanged() {
            if (!win.shown || !win.openedOn)
                return;
            if (Quickshell.screens.indexOf(win.openedOn) < 0)
                Surfaces.close();
        }
    }

    HyprlandFocusGrab {
        windows: [win]
        active: win.visible
        onCleared: if (win.shown) Surfaces.close()
    }

    Item {
        id: body
        anchors.fill: parent
        focus: true
        Keys.onPressed: function (ev) {
            var k = {
                [Qt.Key_Down]: "down", [Qt.Key_Up]: "up", [Qt.Key_Right]: "right",
                [Qt.Key_Left]: "left", [Qt.Key_Return]: "enter", [Qt.Key_Enter]: "enter",
                [Qt.Key_Space]: "enter", [Qt.Key_Escape]: "escape"
            }[ev.key];
            if (k !== undefined) {
                win.key(k);
                ev.accepted = true;
            }
        }

        // A click anywhere off the menu closes it.
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
            onPressed: Surfaces.close()
        }

        MenuBox {
            id: mainBox
            entries: win.top
            selected: win.mainIndex
            accessibleName: "Desktop menu"
            readonly property var pos: D.place(win.px, win.py, width, height, body.width, body.height, Theme.gapOut)
            x: pos.x
            y: pos.y
            onHovered: function (i) {
                win.level = 0;
                win.mainIndex = i;
                if (win.top[i].sub)
                    win.openSub(i, false);
                else
                    win.closeSub();
            }
            onActivated: function (i) { win.activate(win.top[i]); }
        }

        MenuBox {
            id: subBox
            visible: win.sub !== ""
            entries: win.subList
            selected: win.subIndex
            accessibleName: win.subFrom >= 0 ? win.top[win.subFrom].label : ""
            readonly property var pos: D.placeSub(mainBox.x, mainBox.width,
                mainBox.y + mainBox.rowY(win.subFrom) - Theme.gapIn, width, height,
                body.width, body.height, Theme.gapOut, Theme.gapIn)
            x: pos.x
            y: pos.y
            onHovered: function (i) {
                win.level = 1;
                win.subIndex = i;
            }
            onActivated: function (i) { win.activate(win.subList[i]); }
        }
    }

    component MenuBox: Rectangle {
        id: box

        property var entries: []
        property int selected: -1
        property string accessibleName: ""
        signal hovered(int index)
        signal activated(int index)

        // The y of row i inside the box (0 when there is no such row).
        function rowY(i) {
            var r = rows.itemAt(i);
            return r ? col.y + r.y : 0;
        }

        width: Math.max(Theme.fontSize * 16, col.implicitWidth) + Theme.gapIn * 2
        height: col.implicitHeight + Theme.gapIn * 2
        radius: Theme.radius
        color: Theme.surface
        border.color: Theme.border
        border.width: Theme.borderWidth
        Accessible.role: Accessible.PopupMenu
        Accessible.name: accessibleName

        // Clicks on the box's padding don't reach the catcher behind it.
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        }

        ColumnLayout {
            id: col
            x: Theme.gapIn
            y: Theme.gapIn
            width: box.width - Theme.gapIn * 2
            spacing: 2

            Repeater {
                id: rows
                model: box.entries
                delegate: MenuRow {
                    required property var modelData
                    required property int index
                    Layout.fillWidth: true
                    entry: modelData
                    lit: index === box.selected
                    onHovered: box.hovered(index)
                    onActivated: box.activated(index)
                }
            }
        }
    }
}
