pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import "../core"
import "../logic/pick.js" as P

// The window picker (spec §5, §10): while uqd has a pick pending, a translucent full-screen layer
// on every screen says "Click a window to choose it." A click answers PickDone "click" (uqd then
// reads the window under the cursor itself), Esc answers "cancel". The shell sends no coordinates
// and no window data. All decisions live in logic/pick.js. uqd never says when a pick ended, so
// the overlay also closes itself after PICK_TIMEOUT_MS, on lock and when uqd goes away; a late
// PickDone for a finished token is ignored by uqd.
Scope {
    id: root

    property var pick: P.idle()
    readonly property bool active: P.isActive(pick)

    // Sends the reply a state change produced; errors are ignored (the pick may be over already).
    function apply(r) {
        root.pick = r.state;
        if (r.reply === null || r.token === null)
            return;
        Bridge.call("PickDone", [r.token].concat(r.reply)).catch(function () {});
    }

    Connections {
        target: Bridge
        function onPickRequested(token) {
            root.apply(P.start(root.pick, token, Bridge.locked));
            // A new token while the overlay is already up must restart the 20 s clock.
            if (root.active)
                pickTimer.restart();
        }
        function onLockedChanged() {
            if (Bridge.locked)
                root.apply(P.escape(root.pick));
        }
        function onUqdUpChanged() {
            // uqd is gone, so there is nobody to tell: just close.
            if (!Bridge.uqdUp)
                root.pick = P.idle();
        }
    }

    // A new token restarts the clock; going idle stops it.
    Timer {
        id: pickTimer
        interval: P.PICK_TIMEOUT_MS
        running: root.active
        repeat: false
        onTriggered: root.apply(P.timeout(root.pick))
    }

    Variants {
        model: Quickshell.screens

        delegate: PanelWindow {
            id: win

            required property var modelData

            screen: modelData
            visible: root.active
            color: "transparent"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "uq-pick"
            WlrLayershell.keyboardFocus: root.active ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
            exclusionMode: ExclusionMode.Ignore
            anchors { top: true; bottom: true; left: true; right: true }

            onVisibleChanged: if (visible) Qt.callLater(scrim.forceActiveFocus)

            Rectangle {
                id: scrim
                anchors.fill: parent
                color: Qt.rgba(0, 0, 0, 0.35)
                focus: true
                Accessible.role: Accessible.Pane
                Accessible.name: "Choose a window"
                Keys.onEscapePressed: root.apply(P.escape(root.pick))

                UqText {
                    anchors.centerIn: parent
                    text: "Click a window to choose it. Press Esc to cancel."
                    color: Theme.text
                    font.bold: true
                    font.pixelSize: Theme.fontSize + 4
                }

                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.CrossCursor
                    onClicked: root.apply(P.click(root.pick))
                }
            }
        }
    }
}
