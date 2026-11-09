pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Widgets
import "../core"
import "../logic/quick.js" as Q

// Fallback OSD (spec §2.6), used when noti-fi is unavailable or its request fails. Binds to
// OsdRouter's fallback state and hides 1500 ms after the last show. It never takes keyboard
// focus or grabs input, and its empty input mask lets pointer clicks pass through to whatever
// is underneath.
PanelWindow {
    id: root

    readonly property int hideMs: 1500
    readonly property int iconSize: 24
    property bool shown: false
    // The focused monitor when the OSD appeared; re-pinned only on a show while hidden, so
    // key-repeat doesn't make it jump between outputs.
    property var openedOn: null
    readonly property real dpr: screen ? screen.devicePixelRatio : 1
    readonly property string iconName: Q.osdIcon(OsdRouter.kind, OsdRouter.value, OsdRouter.max,
                                                 OsdRouter.muted)
    readonly property real fraction: Q.osdFraction(OsdRouter.value, OsdRouter.max)

    visible: shown
    screen: openedOn ? openedOn : screenFor(Hyprland.focusedMonitor)
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "uq-osd"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Ignore
    // An empty region: no input at all, so the OSD never eats clicks while shown.
    mask: Region {}
    anchors.bottom: true
    margins.bottom: Math.round((screen ? screen.height : 1080) * 0.1)
    implicitWidth: 240
    implicitHeight: 48

    function screenFor(monitor) {
        if (!monitor)
            return null;
        var list = Quickshell.screens;
        for (var i = 0; i < list.length; i++)
            if (list[i].name === monitor.name)
                return list[i];
        return null;
    }

    Connections {
        target: OsdRouter
        function onSerialChanged() {
            if (!root.shown)
                root.openedOn = root.screenFor(Hyprland.focusedMonitor);
            root.shown = true;
            hideTimer.restart();
        }
    }
    Connections {
        target: Quickshell
        // Drop the pin if its monitor was unplugged.
        function onScreensChanged() {
            if (!root.openedOn)
                return;
            var list = Quickshell.screens;
            for (var i = 0; i < list.length; i++)
                if (list[i] === root.openedOn)
                    return;
            root.openedOn = null;
            root.shown = false;
        }
    }

    Timer {
        id: hideTimer
        interval: root.hideMs
        onTriggered: {
            root.shown = false;
            root.openedOn = null;
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.radius
        color: Theme.surface
        border.color: Theme.border
        border.width: Theme.borderWidth

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Theme.gapOut
            anchors.rightMargin: Theme.gapOut
            spacing: Theme.gapIn * 2

            IconImage {
                Layout.preferredWidth: root.iconSize
                Layout.preferredHeight: root.iconSize
                implicitSize: root.iconSize
                backer.sourceSize.width: root.iconSize * root.dpr
                backer.sourceSize.height: root.iconSize * root.dpr
                visible: root.iconName !== ""
                source: root.iconName !== "" ? Quickshell.iconPath(root.iconName, true) : ""
            }
            Rectangle {
                id: track
                Layout.fillWidth: true
                Layout.preferredHeight: 6
                radius: 3
                color: Theme.overlay
                Accessible.role: Accessible.ProgressBar
                Accessible.name: OsdRouter.kind + " level"

                Rectangle {
                    width: Math.round(track.width * root.fraction)
                    height: parent.height
                    radius: parent.radius
                    color: OsdRouter.muted ? Theme.textMuted : Theme.accent
                }
            }
            UqText {
                Layout.preferredWidth: 32
                horizontalAlignment: Text.AlignRight
                text: String(Q.osdPercent(OsdRouter.value, OsdRouter.max))
            }
        }
    }
}
