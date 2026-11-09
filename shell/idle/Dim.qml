pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland

// The idle dim (spec §2.7): a 60% black overlay layer on every screen while the dim stage is
// idle. Any input ends idle in the compositor, which clears `shown`; no input handling here.
Scope {
    id: root

    property bool shown: false
    // ui.reduce_motion: no fade. Theme has no such token yet, so shell.qml leaves this false.
    property bool reduceMotion: false
    readonly property int fadeMs: 1000

    Variants {
        model: Quickshell.screens

        delegate: PanelWindow {
            id: win

            required property var modelData

            screen: modelData
            visible: root.shown
            color: "transparent"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "uq-dim"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            exclusionMode: ExclusionMode.Ignore
            anchors { top: true; bottom: true; left: true; right: true }

            Rectangle {
                id: shade

                anchors.fill: parent
                color: Qt.rgba(0, 0, 0, 0.6)
                // Fades in only: when idle ends the window goes at once, so the user's first
                // input after coming back isn't spent on a fading overlay.
                opacity: root.reduceMotion ? 1 : 0

                NumberAnimation {
                    id: fadeIn

                    target: shade
                    property: "opacity"
                    from: 0
                    to: 1
                    duration: root.fadeMs
                    easing.type: Easing.InOutQuad
                }
            }

            onVisibleChanged: {
                fadeIn.stop();
                if (!visible)
                    return;
                if (root.reduceMotion) {
                    shade.opacity = 1;
                } else {
                    shade.opacity = 0;
                    fadeIn.start();
                }
            }
        }
    }
}
