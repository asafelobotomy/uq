pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import "../core"
import "../logic/quickui.js" as U

// Quick settings (the Control Center): a 380 px panel, at most ~70% of the screen tall, with a
// tab strip (Sound, Network, Display, System, Apps), the selected tab's groups in a scrolling
// column, and the Settings… button under every tab. The selected tab stays while the panel is
// open and is kept for the next open; Apps shows only while an app tile has loaded. Keys: the
// strip takes Left / Right / Home / End, and Ctrl+Tab / Ctrl+Shift+Tab (or Ctrl+Page Down /
// Page Up) switch tabs from anywhere in the panel. Hung under the Status island's button when opened from it (on that island's screen),
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

    // The tab the user picked; `tab` is what shows (the first tab while the picked one is hidden).
    property string selectedTab: "sound"
    readonly property var shownTabs: U.shownTabs({ apps: !apps.shown })
    readonly property string tab: U.currentTab(shownTabs, selectedTab)
    readonly property Item currentPage: tab === "network" ? networkPage
        : tab === "display" ? displayPage
        : tab === "system" ? systemPage
        : tab === "apps" ? appsPage
        : soundPage

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
    implicitHeight: Math.min(maxHeight, layout.implicitHeight + Theme.gapOut * 2)

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
        brightness.wipe();
        apps.wipe();
        flick.contentY = 0;
    }

    function selectTab(key) {
        if (key === "" || key === tab)
            return;
        selectedTab = key;
        flick.contentY = 0;
    }

    onShownChanged: {
        openedOn = shown ? screenFor(Hyprland.focusedMonitor) : null;
        wipe();
        if (shown) {
            place();
            Qt.callLater(tabBar.focusCurrent);
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
        // Switch tabs from anywhere in the panel; focus follows to the strip, since the item that
        // had it may be on the tab just hidden.
        Keys.onPressed: function (event) {
            if (!(event.modifiers & Qt.ControlModifier))
                return;
            var step = 0;
            if (event.key === Qt.Key_Backtab || (event.key === Qt.Key_Tab && (event.modifiers & Qt.ShiftModifier))
                    || event.key === Qt.Key_PageUp)
                step = -1;
            else if (event.key === Qt.Key_Tab || event.key === Qt.Key_PageDown)
                step = 1;
            if (step === 0)
                return;
            tabBar.go(step);
            event.accepted = true;
        }

        ColumnLayout {
            id: layout
            anchors.fill: parent
            anchors.margins: Theme.gapOut
            spacing: Theme.gapOut

            QuickTabs {
                id: tabBar
                Layout.fillWidth: true
                tabs: root.shownTabs
                current: root.tab
                onChosen: function (key) { root.selectTab(key); }
            }

            Flickable {
                id: flick
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.minimumHeight: 0
                Layout.preferredHeight: root.currentPage.implicitHeight
                clip: true
                contentWidth: width
                contentHeight: root.currentPage.implicitHeight
                boundsBehavior: Flickable.StopAtBounds

                // One column per tab; only the shown tab's column is visible (and in the Tab
                // chain). Every group stays loaded, so their state lives as before.
                Item {
                    width: flick.width
                    height: root.currentPage.implicitHeight

                    ColumnLayout {
                        id: soundPage
                        width: parent.width
                        visible: root.tab === "sound"
                        spacing: Theme.gapOut
                        Accessible.role: Accessible.Pane
                        Accessible.name: "Sound"

                        AudioGroup {
                            id: audio
                            Layout.fillWidth: true
                            active: root.shown
                        }
                    }

                    ColumnLayout {
                        id: networkPage
                        width: parent.width
                        visible: root.tab === "network"
                        spacing: Theme.gapOut
                        Accessible.role: Accessible.Pane
                        Accessible.name: "Network"

                        NetworkGroup {
                            id: network
                            Layout.fillWidth: true
                            active: root.shown
                        }

                        BluetoothGroup {
                            Layout.fillWidth: true
                        }
                    }

                    ColumnLayout {
                        id: displayPage
                        width: parent.width
                        visible: root.tab === "display"
                        spacing: Theme.gapOut
                        Accessible.role: Accessible.Pane
                        Accessible.name: "Display"

                        BrightnessGroup {
                            id: brightness
                            Layout.fillWidth: true
                            active: root.shown
                        }

                        ThemeGroup {
                            Layout.fillWidth: true
                            active: root.shown
                        }
                    }

                    ColumnLayout {
                        id: systemPage
                        width: parent.width
                        visible: root.tab === "system"
                        spacing: Theme.gapOut
                        Accessible.role: Accessible.Pane
                        Accessible.name: "System"

                        PowerGroup {
                            Layout.fillWidth: true
                        }

                        ProfileGroup {
                            Layout.fillWidth: true
                            active: root.shown
                        }
                    }

                    ColumnLayout {
                        id: appsPage
                        width: parent.width
                        visible: root.tab === "apps"
                        spacing: Theme.gapOut
                        Accessible.role: Accessible.Pane
                        Accessible.name: "Apps"

                        // Tiles from other apps; the tab hides while there are none.
                        AppsGroup {
                            id: apps
                            Layout.fillWidth: true
                        }
                    }
                }
            }

            SettingsEntry {
                Layout.fillWidth: true
            }
        }
    }
}
