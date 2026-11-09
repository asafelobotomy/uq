pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import "../../core"
import ".."

// The built-in server's notification centre: a 400 px panel at the top right of the focused
// monitor, at most ~70% of the screen tall, with "Clear all", a Do Not Disturb toggle and
// Builtin.notifications (newest first). It is the "notifcentre" surface in Surfaces, opened by
// NotifBackend.toggleCentre() when the built-in backend is in use; it closes on a click outside
// (HyprlandFocusGrab), Esc, lock, or its monitor going. Opening it marks everything read.
PanelWindow {
    id: root

    readonly property bool shown: Surfaces.open === "notifcentre" && !Bridge.locked
    // The focused monitor at the moment the centre opened, so it stays put when the cursor (and
    // Hyprland's focus) crosses to another output. Followed live only while hidden.
    property var openedOn: null
    readonly property var target: Surfaces.screen ? Surfaces.screen
        : (openedOn ? openedOn : screenFor(Hyprland.focusedMonitor))
    readonly property int panelWidth: 400
    readonly property int maxHeight: Math.round((screen ? screen.height : 1080) * 0.7)
    // Where the bar puts its islands: Bar.qml's top margin plus its island height.
    readonly property int barBottom: Theme.gapOut + 36
    readonly property real dpr: screen ? screen.devicePixelRatio : 1

    visible: shown
    screen: target
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.namespace: "uq-notif-centre"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
    exclusionMode: ExclusionMode.Ignore
    anchors.top: true
    anchors.right: true
    margins.top: barBottom + Theme.gapIn
    margins.right: Theme.gapOut
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

    function wipe() {
        flick.contentY = 0;
    }

    onShownChanged: {
        openedOn = shown ? screenFor(Hyprland.focusedMonitor) : null;
        wipe();
        if (shown) {
            Builtin.markRead();
            Qt.callLater(content.forceActiveFocus);
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
            if (surface === "notifcentre")
                root.wipe();
        }
    }

    HyprlandFocusGrab {
        windows: [root]
        active: root.visible
        onCleared: Surfaces.close()
    }

    Rectangle {
        id: content
        anchors.fill: parent
        radius: Theme.radius
        color: Theme.bg
        border.color: Theme.border
        border.width: Theme.borderWidth
        focus: true
        Accessible.role: Accessible.Pane
        Accessible.name: "Notifications"
        Keys.onEscapePressed: Surfaces.close()

        ColumnLayout {
            id: layout
            anchors.fill: parent
            anchors.margins: Theme.gapOut
            spacing: Theme.gapIn * 2

            // --- header ---
            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.gapIn * 2

                UqText {
                    Layout.fillWidth: true
                    text: "Notifications"
                    font.bold: true
                }

                // Do Not Disturb
                Rectangle {
                    id: dnd
                    readonly property bool on: NotifBackend.dnd
                    function toggle() {
                        NotifBackend.setDnd(!dnd.on);
                    }
                    implicitHeight: 24
                    implicitWidth: dndRow.implicitWidth + Theme.gapIn * 4
                    radius: Theme.radius - 4
                    color: dnd.on ? Theme.accent : (dndMouse.containsMouse ? Theme.overlay : Theme.surfaceAlt)
                    activeFocusOnTab: true
                    border.color: Theme.accent
                    border.width: activeFocus && !dnd.on ? 1 : 0
                    Accessible.role: Accessible.CheckBox
                    Accessible.checkable: true
                    Accessible.checked: dnd.on
                    Accessible.name: "Do Not Disturb"
                    Accessible.onPressAction: dnd.toggle()
                    Accessible.onToggleAction: dnd.toggle()
                    Keys.onSpacePressed: dnd.toggle()
                    Keys.onReturnPressed: dnd.toggle()

                    Row {
                        id: dndRow
                        anchors.centerIn: parent
                        spacing: Theme.gapIn
                        UqText {
                            text: "🔕"
                            color: dnd.on ? Theme.accentText : Theme.text
                        }
                        UqText {
                            text: "Do Not Disturb"
                            color: dnd.on ? Theme.accentText : Theme.text
                        }
                    }
                    MouseArea {
                        id: dndMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: dnd.toggle()
                    }
                }

                // Clear all
                Rectangle {
                    id: clear
                    implicitHeight: 24
                    implicitWidth: clearText.implicitWidth + Theme.gapIn * 4
                    radius: Theme.radius - 4
                    color: clearMouse.containsMouse ? Theme.overlay : Theme.surfaceAlt
                    activeFocusOnTab: true
                    enabled: Builtin.notifications.length > 0
                    opacity: enabled ? 1 : 0.5
                    border.color: Theme.accent
                    border.width: activeFocus ? 1 : 0
                    Accessible.role: Accessible.Button
                    Accessible.name: "Clear all notifications"
                    Accessible.onPressAction: Builtin.clearAll()
                    Keys.onSpacePressed: Builtin.clearAll()
                    Keys.onReturnPressed: Builtin.clearAll()

                    UqText {
                        id: clearText
                        anchors.centerIn: parent
                        text: "Clear all"
                    }
                    MouseArea {
                        id: clearMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: Builtin.clearAll()
                    }
                }
            }

            UqText {
                Layout.fillWidth: true
                visible: Builtin.notifications.length === 0
                horizontalAlignment: Text.AlignHCenter
                text: "No notifications"
                color: Theme.textMuted
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }

            // --- the list ---
            Flickable {
                id: flick
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredHeight: list.implicitHeight
                visible: Builtin.notifications.length > 0
                clip: true
                contentWidth: width
                contentHeight: list.implicitHeight
                boundsBehavior: Flickable.StopAtBounds

                Column {
                    id: list
                    width: flick.width
                    spacing: Theme.gapIn * 2

                    Repeater {
                        model: Builtin.notifications
                        delegate: NotifCard {
                            required property var modelData
                            notification: modelData
                            width: list.width
                            dpr: root.dpr
                        }
                    }
                }
            }
        }
    }
}
