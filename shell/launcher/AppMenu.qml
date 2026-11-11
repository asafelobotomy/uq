pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Widgets
import "../core"
import "../session"
import "../logic/launcher.js" as L
import "../logic/rank.js" as Rank
import "../logic/appmenu.js" as M
import "../logic/failure.js" as F

// KDE-style app menu, hung under the System island's app-menu button: avatar and an apps-only
// search at the top, categories on the left, an app grid on the right, and user@host plus the
// power buttons at the bottom. Logic lives in appmenu.js; app names go through UqText, and
// clicks only ever make bridge calls (Launch / Power) or Session.lock(). Search is local (rank.searchApps).
PanelWindow {
    id: root

    readonly property bool shown: Surfaces.open === "appmenu" && !Bridge.locked
    // The focused monitor at the moment the surface opened, so an open surface stays put when
    // the cursor (and Hyprland's focus) crosses to another output. Followed live only while hidden.
    property var openedOn: null
    readonly property var target: Surfaces.screen ? Surfaces.screen
        : (openedOn ? openedOn : screenFor(Hyprland.focusedMonitor))
    readonly property real dpr: screen ? screen.devicePixelRatio : 1
    readonly property int menuWidth: 440
    readonly property int menuHeight: 420
    readonly property int iconSize: 36
    readonly property int avatarSize: 32
    readonly property int columns: 4
    // Where the bar puts its islands when there's no anchor (IPC / keybind): Bar.qml's top margin
    // plus its island height.
    readonly property int barBottom: Theme.gapOut + 36

    // --- state (all cleared by wipe()) ---
    property string query: ""
    // Why the last launch failed (failure.js), shown above the footer; "" when nothing failed.
    property string error: ""
    property string currentCategory: M.FAVOURITES
    property int placeX: Theme.gapOut
    property int placeY: barBottom + Theme.gapIn

    readonly property var appList: M.menuEntries(DesktopEntries.applications.values)
    readonly property var pinned: Settings.values.pinned || []
    readonly property var favList: M.favourites(pinned, appList)
    readonly property var categoryList: M.categories(appList, favList.length)
    readonly property bool searching: query.trim() !== ""
    readonly property var gridModel: searching
        ? Rank.searchApps(query, appList, Settings.launchCounts, 48)
        : M.appsIn(currentCategory, appList, pinned)

    // Header and footer: built only from $HOME, $USER, $HOSTNAME and /etc/hostname.
    readonly property string userName: String(Quickshell.env("USER") || "")
    readonly property var avatarSources: M.avatarSources(Quickshell.env("HOME"), userName)
    property int avatarIndex: 0
    property string hostFileText: ""
    readonly property string hostName: M.hostname(Quickshell.env("HOSTNAME"), hostFileText)

    visible: shown
    screen: target
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.namespace: "uq-appmenu"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
    exclusionMode: ExclusionMode.Ignore
    anchors.top: true
    anchors.left: true
    margins.top: placeY
    margins.left: placeX
    implicitWidth: menuWidth
    implicitHeight: menuHeight

    function screenFor(monitor) {
        if (!monitor)
            return null;
        var list = Quickshell.screens;
        for (var i = 0; i < list.length; i++)
            if (list[i].name === monitor.name)
                return list[i];
        return null;
    }

    // Under the anchor (the island's button), kept on screen. The anchor's scene position is in
    // its bar window, which sits at Bar.qml's margins.
    function place() {
        var a = Surfaces.anchor;
        var sw = screen ? screen.width : 1280;
        var x = Theme.gapOut, y = barBottom;
        if (a && typeof a.mapToItem === "function") {
            var p = a.mapToItem(null, 0, a.height);
            x = Theme.gapOut + Math.round(p.x) - Theme.gapIn;
            y = Theme.gapOut + Math.round(p.y);
        }
        placeX = Math.max(0, Math.min(x, sw - menuWidth));
        placeY = y + Theme.gapIn;
    }

    function wipe() {
        actionMenu.close();
        error = "";
        launchClose.stop();
        field.text = "";
        query = "";
        currentCategory = favList.length > 0 ? M.FAVOURITES : M.ALL;
        grid.currentIndex = 0;
        grid.positionViewAtBeginning();
    }

    function launch(item) {
        if (Bridge.locked)
            return;
        var a = L.activation("apps", item);
        if (a === null)
            return;
        // Launch {desktop_id} records the launch in the bridge. The menu closes once the bridge
        // has launched it (or after launchClose, if that takes long); a refusal keeps the menu
        // open and says why.
        error = "";
        var name = String(item.name || item.id || "the app");
        launchClose.restart();
        Bridge.call(a.call, a.args).then(function () {
            launchClose.stop();
            if (root.shown)
                Surfaces.close();
        }).catch(function (e) {
            launchClose.stop();
            console.warn("uq: launching " + String(item.id || "") + " failed: " + e);
            if (root.shown)
                root.error = F.failureText("Couldn't open " + name, e);
        });
    }

    Timer {
        id: launchClose
        interval: 1500
        onTriggered: if (root.shown) Surfaces.close()
    }

    // lock, logout and suspend run at once (lock through Session.lock(), like the power menu);
    // restart and shut down hand over to the power menu (on this menu's screen) at their
    // confirm step, which closes the app menu.
    function power(action) {
        if (Bridge.locked)
            return;
        switch (M.powerClick(action)) {
        case "lock":
            Surfaces.close();
            Session.lock();
            break;
        case "fire":
            // The menu closes at once (the session is ending or sleeping); a refusal is logged.
            Bridge.call("Power", [action]).catch(function (e) {
                console.warn("uq: Power " + action + " failed: " + e);
            });
            Surfaces.close();
            break;
        case "powermenu":
            Surfaces.toggle("powermenu", root.screen, null, action);
            break;
        }
    }

    function iconSource(icon) {
        var r = L.iconRef(icon);
        if (r.path !== undefined)
            return r.path;
        return Quickshell.iconPath(r.theme, true) || Quickshell.iconPath("application-x-executable", true);
    }

    function moveGrid(step) {
        var n = gridModel.length;
        if (n === 0)
            return;
        grid.currentIndex = Math.max(0, Math.min(n - 1, grid.currentIndex + step));
    }

    onShownChanged: {
        openedOn = shown ? screenFor(Hyprland.focusedMonitor) : null;
        wipe();
        if (shown) {
            place();
            Qt.callLater(field.forceActiveFocus);
        }
    }
    onGridModelChanged: grid.currentIndex = 0

    Connections {
        target: Bridge
        function onLockedChanged() {
            if (Bridge.locked) {
                root.wipe();
                Surfaces.lock();
            }
        }
    }
    Connections {
        target: Quickshell
        // Close if our monitor was unplugged.
        function onScreensChanged() {
            if (!root.shown || !Surfaces.screen)
                return;
            var list = Quickshell.screens;
            for (var i = 0; i < list.length; i++)
                if (list[i] === Surfaces.screen)
                    return;
            Surfaces.close();
        }
    }
    Connections {
        target: Surfaces
        function onWipe(surface) {
            if (surface === "appmenu")
                root.wipe();
        }
    }

    // $HOSTNAME is rarely exported to a systemd user service; fall back to /etc/hostname.
    FileView {
        path: Quickshell.env("HOSTNAME") ? "" : "/etc/hostname"
        printErrors: false
        onLoaded: root.hostFileText = text()
    }

    HyprlandFocusGrab {
        // The bar window is included so a click on the app-menu button toggles the menu closed
        // instead of clearing the grab and reopening it.
        windows: Surfaces.anchor && Surfaces.anchor.barWindow ? [root, Surfaces.anchor.barWindow] : [root]
        active: root.visible
        onCleared: Surfaces.close()
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.radius
        color: Theme.surface
        border.color: Theme.border
        border.width: Theme.borderWidth

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: Theme.gapOut
            spacing: Theme.gapIn * 2

            // --- header: avatar + search ---
            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.gapIn * 2

                ClippingRectangle {
                    Layout.preferredWidth: root.avatarSize
                    Layout.preferredHeight: root.avatarSize
                    radius: root.avatarSize / 2
                    color: Theme.accent
                    Accessible.role: Accessible.Graphic
                    Accessible.name: "Avatar for " + root.userName

                    UqText {
                        anchors.centerIn: parent
                        visible: avatar.status !== Image.Ready
                        text: M.initial(root.userName)
                        color: Theme.accentText
                        font.pixelSize: Theme.fontSize + 4
                        font.bold: true
                    }
                    Image {
                        id: avatar
                        anchors.fill: parent
                        asynchronous: true
                        fillMode: Image.PreserveAspectCrop
                        sourceSize.width: root.avatarSize * root.dpr
                        sourceSize.height: root.avatarSize * root.dpr
                        source: root.avatarIndex < root.avatarSources.length
                            ? root.avatarSources[root.avatarIndex] : ""
                        visible: status === Image.Ready
                        // A missing or unreadable file falls through to the next source.
                        onStatusChanged: if (status === Image.Error) root.avatarIndex += 1
                    }
                }

                UqInput {
                    id: field
                    Layout.fillWidth: true
                    implicitHeight: 32
                    maximumLength: 256
                    placeholder: "Search apps"
                    Accessible.name: "Search apps"
                    onEdited: function (text) { root.query = text; }
                    Keys.onPressed: function (event) {
                        if (event.key === Qt.Key_Escape) {
                            Surfaces.close();
                        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                            if (root.gridModel.length > 0)
                                root.launch(root.gridModel[Math.max(0, grid.currentIndex)]);
                        } else if (event.key === Qt.Key_Down) {
                            root.moveGrid(root.columns);
                        } else if (event.key === Qt.Key_Up) {
                            root.moveGrid(-root.columns);
                        } else if (event.key === Qt.Key_Tab) {
                            root.moveGrid(1);
                        } else if (event.key === Qt.Key_Backtab) {
                            root.moveGrid(-1);
                        } else {
                            return;
                        }
                        event.accepted = true;
                    }
                }
            }

            // --- body: categories | grid ---
            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: Theme.gapIn * 2

                ListView {
                    id: cats
                    Layout.preferredWidth: 112
                    Layout.fillHeight: true
                    clip: true
                    spacing: 2
                    boundsBehavior: Flickable.StopAtBounds
                    model: root.categoryList
                    delegate: Rectangle {
                        id: catRow
                        required property var modelData
                        readonly property bool current: !root.searching
                            && root.currentCategory === modelData.key
                        width: cats.width
                        height: 28
                        radius: Theme.radius - 2
                        color: current ? Theme.overlay
                            : (catMouse.containsMouse ? Theme.surfaceAlt : "transparent")
                        Accessible.role: Accessible.Button
                        Accessible.name: modelData.key

                        UqText {
                            anchors.fill: parent
                            anchors.leftMargin: Theme.gapIn * 2
                            anchors.rightMargin: Theme.gapIn
                            verticalAlignment: Text.AlignVCenter
                            text: catRow.modelData.key
                            color: catRow.current ? Theme.text : Theme.textMuted
                        }
                        MouseArea {
                            id: catMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: {
                                root.currentCategory = catRow.modelData.key;
                                field.text = "";
                                root.query = "";
                                field.forceActiveFocus();
                            }
                        }
                    }
                }

                Rectangle {
                    Layout.preferredWidth: 1
                    Layout.fillHeight: true
                    color: Theme.border
                }

                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    GridView {
                        id: grid
                        anchors.fill: parent
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        cellWidth: Math.floor(width / root.columns)
                        cellHeight: 76
                        model: root.gridModel
                        currentIndex: 0
                        delegate: Rectangle {
                            id: cell
                            required property var modelData
                            required property int index
                            readonly property bool selected: grid.currentIndex === index
                            width: grid.cellWidth - 2
                            height: grid.cellHeight - 2
                            radius: Theme.radius - 2
                            color: selected ? Theme.overlay
                                : (cellMouse.containsMouse ? Theme.surfaceAlt : "transparent")
                            Accessible.role: Accessible.Button
                            Accessible.name: String(modelData.name || "")

                            ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 4
                                spacing: 2

                                IconImage {
                                    Layout.alignment: Qt.AlignHCenter
                                    Layout.preferredWidth: root.iconSize
                                    Layout.preferredHeight: root.iconSize
                                    implicitSize: root.iconSize
                                    backer.sourceSize.width: root.iconSize * root.dpr
                                    backer.sourceSize.height: root.iconSize * root.dpr
                                    source: root.iconSource(cell.modelData.icon)
                                }
                                UqText {
                                    Layout.fillWidth: true
                                    Layout.fillHeight: true
                                    horizontalAlignment: Text.AlignHCenter
                                    wrapMode: Text.Wrap
                                    maximumLineCount: 2
                                    font.pixelSize: Theme.fontSize - 1
                                    text: String(cell.modelData.name || "")
                                }
                            }
                            MouseArea {
                                id: cellMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                acceptedButtons: Qt.LeftButton | Qt.RightButton
                                onClicked: function (mouse) {
                                    if (mouse.button === Qt.RightButton)
                                        actionMenu.openAt(cell, mouse.x, mouse.y,
                                                          String(cell.modelData.id || ""));
                                    else
                                        root.launch(cell.modelData);
                                }
                            }
                        }
                    }

                    UqText {
                        anchors.centerIn: parent
                        visible: root.gridModel.length === 0
                        text: root.searching ? "No matching apps"
                            : (root.currentCategory === M.FAVOURITES
                               ? "No favourites yet: pin apps from the dock" : "No apps")
                        color: Theme.textMuted
                    }
                }
            }

            UqText {
                objectName: "launchError"
                Layout.fillWidth: true
                visible: root.error !== ""
                text: root.error
                color: Theme.error
                wrapMode: Text.Wrap
                maximumLineCount: 2
                elide: Text.ElideRight
            }

            // --- footer: user@host + power ---
            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.gapIn

                UqText {
                    Layout.fillWidth: true
                    text: root.userName + "@" + root.hostName
                    color: Theme.textMuted
                }

                Repeater {
                    model: [
                        { action: "lock", glyph: "🔒", name: "Lock" },
                        { action: "logout", glyph: "⇥", name: "Log out" },
                        { action: "suspend", glyph: "⏾", name: "Suspend" },
                        { action: "reboot", glyph: "↻", name: "Restart" },
                        { action: "poweroff", glyph: "⏻", name: "Shut down" }
                    ]
                    delegate: Rectangle {
                        id: pbtn
                        required property var modelData
                        Layout.preferredWidth: 30
                        Layout.preferredHeight: 30
                        radius: Theme.radius - 2
                        color: pMouse.containsMouse ? Theme.surfaceAlt : "transparent"
                        Accessible.role: Accessible.Button
                        Accessible.name: modelData.name

                        UqText {
                            anchors.centerIn: parent
                            text: pbtn.modelData.glyph
                            color: Theme.text
                            font.pixelSize: Theme.fontSize + 3
                        }
                        MouseArea {
                            id: pMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: root.power(pbtn.modelData.action)
                        }
                    }
                }
            }
        }

        // Right-click on an app: app actions from other apps (Phase 4 spec §2.4.2).
        AppActionMenu {
            id: actionMenu
            onDone: Surfaces.close()
            onOpenChanged: if (!open && root.shown) field.forceActiveFocus()
        }
    }
}
