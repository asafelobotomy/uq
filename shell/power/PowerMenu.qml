pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import "../core"
import "../session"
import "../logic/power.js" as P

// The power menu (spec §2.13): a panel centred on the focused monitor with Lock, Log out,
// Suspend, Restart and Shut down. It is the "powermenu" surface in Surfaces, opened by
// SUPER+Escape, `qs ipc ... call uq toggle powermenu`, and the app menu's restart and shut-down
// buttons (which open it at that action's confirm step, Surfaces.powerConfirm).
// Restart and shut down start a 30 s countdown that runs the action at 0 unless it's cancelled:
// Cancel, Esc, a click outside (HyprlandFocusGrab), lock, or the menu closing for any reason.
// A cancelled countdown is null, the 1 s Timer runs only while one is pending, and `due` is
// checked against the live state, so a cancelled countdown never fires later. Clicks only ever
// make bridge calls (Power) or Session.lock().
PanelWindow {
    id: root

    readonly property bool shown: Surfaces.open === "powermenu" && !Bridge.locked
    // The focused monitor at the moment the menu opened, so it stays put when the cursor (and
    // Hyprland's focus) crosses to another output. Followed live only while hidden.
    property var openedOn: null
    readonly property var target: Surfaces.screen ? Surfaces.screen
        : (openedOn ? openedOn : screenFor(Hyprland.focusedMonitor))

    // --- state (cleared whenever the menu hides) ---
    property var confirm: null      // power.js countdown, or null
    property double now: 0          // ms, refreshed by the tick timer for the label

    readonly property var actions: [
        { action: "lock", glyph: "🔒", name: "Lock" },
        { action: "logout", glyph: "⇥", name: "Log out" },
        { action: "suspend", glyph: "⏾", name: "Suspend" },
        { action: "reboot", glyph: "↻", name: "Restart" },
        { action: "poweroff", glyph: "⏻", name: "Shut down" }
    ]

    visible: shown
    screen: target
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.namespace: "uq-power"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
    exclusionMode: ExclusionMode.Ignore
    // No anchors: the compositor centres the layer surface on its output.
    implicitWidth: layout.implicitWidth + Theme.gapOut * 2
    implicitHeight: layout.implicitHeight + Theme.gapOut * 2

    function screenFor(monitor) {
        if (!monitor)
            return null;
        var list = Quickshell.screens;
        for (var i = 0; i < list.length; i++)
            if (list[i].name === monitor.name)
                return list[i];
        return null;
    }

    function cancelConfirm() {
        confirm = P.cancel();
    }

    // Cancel: drop the countdown and go back to the five buttons, keeping keyboard focus.
    function back() {
        cancelConfirm();
        content.forceActiveFocus();
    }

    // A button: lock, logout and suspend run now; reboot and poweroff start the countdown.
    function choose(action) {
        if (Bridge.locked || !root.shown)
            return;
        if (P.needsConfirm(action)) {
            now = Date.now();
            confirm = P.startConfirm(action, now);
            Qt.callLater(cancelButton.forceActiveFocus);
        } else if (P.ACTIONS.indexOf(action) >= 0) {
            run(action);
        }
    }

    // Closes the menu (which also cancels any countdown), then runs the action.
    function run(action) {
        if (Bridge.locked || P.ACTIONS.indexOf(action) < 0)
            return;
        cancelConfirm();
        Surfaces.close();
        if (action === "lock")
            Session.lock();
        else
            Bridge.call("Power", [action]).catch(function (e) {
                console.log("uq: Power " + action + " failed: " + e);
            });
    }

    // "Restart now" / "Shut down now": runs the pending action, if one is still pending.
    function runNow() {
        if (root.confirm !== null && root.shown)
            run(root.confirm.action);
    }

    function tick() {
        now = Date.now();
        // Evaluated against the live state: a cancelled countdown is null and never due.
        if (root.shown && P.due(root.confirm, now))
            run(root.confirm.action);
    }

    onShownChanged: {
        openedOn = shown ? screenFor(Hyprland.focusedMonitor) : null;
        cancelConfirm();
        if (shown) {
            now = Date.now();
            confirm = P.startConfirm(Surfaces.powerConfirm, now);
            Qt.callLater(function () {
                if (root.confirm !== null)
                    cancelButton.forceActiveFocus();
                else
                    content.forceActiveFocus();
            });
        }
    }

    Timer {
        id: tickTimer
        interval: 1000
        repeat: true
        running: root.confirm !== null && root.shown
        onTriggered: root.tick()
    }

    Connections {
        target: Bridge
        function onLockedChanged() {
            if (Bridge.locked)
                root.cancelConfirm();
        }
    }
    Connections {
        target: Surfaces
        function onWipe(surface) {
            if (surface === "powermenu")
                root.cancelConfirm();
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
        Accessible.name: "Power"
        Keys.onEscapePressed: Surfaces.close()

        ColumnLayout {
            id: layout
            anchors.centerIn: parent
            spacing: Theme.gapIn * 2

            // --- the five actions ---
            RowLayout {
                visible: root.confirm === null
                spacing: Theme.gapIn * 2

                Repeater {
                    model: root.actions
                    delegate: Rectangle {
                        id: btn
                        required property var modelData
                        required property int index
                        Layout.preferredWidth: 88
                        Layout.preferredHeight: 80
                        radius: Theme.radius - 2
                        color: btnMouse.containsMouse || activeFocus ? Theme.surfaceAlt : "transparent"
                        border.color: Theme.accent
                        border.width: activeFocus ? 1 : 0
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: modelData.name
                        Accessible.onPressAction: root.choose(btn.modelData.action)
                        Keys.onReturnPressed: root.choose(btn.modelData.action)
                        Keys.onEnterPressed: root.choose(btn.modelData.action)
                        Keys.onSpacePressed: root.choose(btn.modelData.action)
                        Keys.onEscapePressed: Surfaces.close()
                        Keys.onLeftPressed: btn.nextItemInFocusChain(false).forceActiveFocus()
                        Keys.onRightPressed: btn.nextItemInFocusChain(true).forceActiveFocus()

                        Column {
                            anchors.centerIn: parent
                            spacing: Theme.gapIn
                            UqText {
                                anchors.horizontalCenter: parent.horizontalCenter
                                text: btn.modelData.glyph
                                font.pixelSize: Theme.fontSize + 12
                            }
                            UqText {
                                anchors.horizontalCenter: parent.horizontalCenter
                                text: btn.modelData.name
                            }
                        }
                        MouseArea {
                            id: btnMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: root.choose(btn.modelData.action)
                        }
                    }
                }
            }

            // --- the confirm step: countdown, run now, cancel ---
            ColumnLayout {
                visible: root.confirm !== null
                Layout.alignment: Qt.AlignHCenter
                spacing: Theme.gapIn * 2

                UqText {
                    Layout.alignment: Qt.AlignHCenter
                    text: P.countdownLabel(root.confirm, root.now)
                    color: Theme.warning
                    font.bold: true
                    font.pixelSize: Theme.fontSize + 4
                }

                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: Theme.gapIn * 2

                    Rectangle {
                        id: nowButton
                        Layout.preferredWidth: nowText.implicitWidth + Theme.gapOut * 2
                        Layout.preferredHeight: 32
                        radius: Theme.radius - 2
                        color: Theme.warning
                        border.color: Theme.text
                        border.width: activeFocus ? 1 : 0
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: nowText.text
                        Accessible.onPressAction: root.runNow()
                        Keys.onReturnPressed: root.runNow()
                        Keys.onEnterPressed: root.runNow()
                        Keys.onSpacePressed: root.runNow()
                        Keys.onEscapePressed: Surfaces.close()

                        UqText {
                            id: nowText
                            anchors.centerIn: parent
                            text: P.nowLabel(root.confirm ? root.confirm.action : "")
                            color: Theme.accentText
                        }
                        MouseArea {
                            anchors.fill: parent
                            onClicked: root.runNow()
                        }
                    }

                    Rectangle {
                        id: cancelButton
                        Layout.preferredWidth: cancelText.implicitWidth + Theme.gapOut * 2
                        Layout.preferredHeight: 32
                        radius: Theme.radius - 2
                        color: cancelMouse.containsMouse || activeFocus ? Theme.overlay : Theme.surfaceAlt
                        border.color: Theme.accent
                        border.width: activeFocus ? 1 : 0
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: "Cancel"
                        Accessible.onPressAction: root.back()
                        Keys.onReturnPressed: root.back()
                        Keys.onEnterPressed: root.back()
                        Keys.onSpacePressed: root.back()
                        Keys.onEscapePressed: Surfaces.close()

                        UqText {
                            id: cancelText
                            anchors.centerIn: parent
                            text: "Cancel"
                        }
                        MouseArea {
                            id: cancelMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: root.back()
                        }
                    }
                }
            }
        }
    }
}
