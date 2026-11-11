pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import "../core"

// Quick settings: a 380 px panel of groups in a scrolling column, at most ~70% of the screen
// tall. Hung under the Status island's button when opened from it (on that island's screen),
// else top-right of the focused monitor (SUPER+A, IPC). Which surface is open lives in
// Surfaces; this closes on a click outside (HyprlandFocusGrab), Esc, lock, or its monitor going.
PanelWindow {
    id: root

    readonly property bool shown: Surfaces.open === "quicksettings" && !Bridge.locked
    // The focused monitor at the moment the surface opened, so an open surface stays put when
    // the cursor (and Hyprland's focus) crosses to another output. Followed live only while hidden.
    property var openedOn: null
    readonly property var target: Surfaces.screen ? Surfaces.screen
        : (openedOn ? openedOn : screenFor(Hyprland.focusedMonitor))
    readonly property int panelWidth: 380
    readonly property int maxHeight: Math.round((screen ? screen.height : 1080) * 0.7)
    // Where the bar puts its islands: Bar.qml's top margin plus its island height.
    readonly property int barBottom: Theme.gapOut + 36

    property int placeX: Theme.gapOut
    property int placeY: barBottom + Theme.gapIn

    visible: shown
    screen: target
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.namespace: "uq-quick"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
    exclusionMode: ExclusionMode.Ignore
    anchors.top: true
    anchors.left: true
    margins.top: placeY
    margins.left: placeX
    implicitWidth: panelWidth
    implicitHeight: Math.min(maxHeight, column.implicitHeight + Theme.gapOut * 2)

    function screenFor(monitor) {
        if (!monitor)
            return null;
        var list = Quickshell.screens;
        for (var i = 0; i < list.length; i++)
            if (list[i].name === monitor.name)
                return list[i];
        return null;
    }

    // Right edge under the anchor (the Status island's button), kept on screen; with no anchor,
    // top-right of the screen. The anchor's scene position is in its bar window, which sits at
    // Bar.qml's margins.
    function place() {
        var a = Surfaces.anchor;
        var sw = screen ? screen.width : 1280;
        var x = sw - panelWidth - Theme.gapOut, y = barBottom;
        if (a && typeof a.mapToItem === "function") {
            var p = a.mapToItem(null, a.width, a.height);
            x = Theme.gapOut + Math.round(p.x) + Theme.gapIn - panelWidth;
            y = Theme.gapOut + Math.round(p.y);
        }
        placeX = Math.max(0, Math.min(x, sw - panelWidth));
        placeY = y + Theme.gapIn;
    }

    function wipe() {
        audio.wipe();
        network.wipe();
        apps.wipe();
        flick.contentY = 0;
    }

    onShownChanged: {
        openedOn = shown ? screenFor(Hyprland.focusedMonitor) : null;
        wipe();
        if (shown) {
            place();
            Qt.callLater(content.forceActiveFocus);
        }
    }
    onScreenChanged: if (shown) place()

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
            if (!root.shown)
                return;
            var mine = Surfaces.screen ? Surfaces.screen : root.openedOn;
            if (!mine)
                return;
            var list = Quickshell.screens;
            for (var i = 0; i < list.length; i++)
                if (list[i] === mine)
                    return;
            Surfaces.close();
        }
    }
    Connections {
        target: Surfaces
        function onWipe(surface) {
            if (surface === "quicksettings")
                root.wipe();
        }
    }

    HyprlandFocusGrab {
        // The bar window is included so a click on the Status button toggles the panel closed
        // instead of clearing the grab and reopening it.
        windows: Surfaces.anchor && Surfaces.anchor.barWindow ? [root, Surfaces.anchor.barWindow] : [root]
        active: root.visible
        onCleared: Surfaces.close()
    }

    Rectangle {
        id: content
        anchors.fill: parent
        radius: Theme.radius
        color: Theme.surface
        border.color: Theme.border
        border.width: Theme.borderWidth
        focus: true
        Accessible.role: Accessible.Pane
        Accessible.name: "Quick settings"
        Keys.onEscapePressed: Surfaces.close()

        Flickable {
            id: flick
            anchors.fill: parent
            anchors.margins: Theme.gapOut
            clip: true
            contentWidth: width
            contentHeight: column.implicitHeight
            boundsBehavior: Flickable.StopAtBounds

            ColumnLayout {
                id: column
                width: flick.width
                spacing: Theme.gapOut

                AudioGroup {
                    id: audio
                    Layout.fillWidth: true
                    active: root.shown
                }

                NetworkGroup {
                    id: network
                    Layout.fillWidth: true
                    active: root.shown
                }

                BluetoothGroup {
                    Layout.fillWidth: true
                }

                ThemeGroup {
                    Layout.fillWidth: true
                    active: root.shown
                }

                ProfileGroup {
                    Layout.fillWidth: true
                    active: root.shown
                }

                PowerGroup {
                    Layout.fillWidth: true
                }

                // Tiles from other apps; hidden while there are none.
                AppsGroup {
                    id: apps
                    Layout.fillWidth: true
                }

                SettingsEntry {
                    Layout.fillWidth: true
                }
            }
        }
    }
}
