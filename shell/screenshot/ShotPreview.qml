pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Hyprland
import "../core"
import "../logic/screenshot.js" as S

// The floating preview every screenshot ends in (never a new window): the capture, where it was
// saved, and Copy, Discard and a close button. `uq-screenshot` calls
//   qs ipc -p /usr/share/uq/shell call screenshot show <file.png> <copied|"">
// once the file is saved (and, normally, already on the clipboard). It stays for SHOW_MS, or
// while the pointer is over it, and never takes keyboard focus; the window's input mask covers
// only the card, so clicks beside it reach the windows below. Hidden while locked.
PanelWindow {
    id: root

    readonly property int cardWidth: 340
    readonly property int imageMax: 200
    property var shot: null            // S.show() state, or null when nothing is showing
    property bool copyFailed: false
    property bool discarded: false
    readonly property bool shown: shot !== null && !Bridge.locked
    readonly property real dpr: screen ? screen.devicePixelRatio : 1

    function screenFor(monitor) {
        if (!monitor)
            return null;
        var list = Quickshell.screens;
        for (var i = 0; i < list.length; i++)
            if (list[i].name === monitor.name)
                return list[i];
        return null;
    }

    function open(path, copied) {
        var s = S.show(path, copied);
        if (s === null)
            return false;
        root.copyFailed = false;
        root.discarded = false;
        root.shot = s;
        hideTimer.interval = S.SHOW_MS;
        hideTimer.restart();
        return true;
    }
    function close() {
        hideTimer.stop();
        root.shot = null;
    }
    function copy() {
        if (root.shot === null || root.discarded)
            return;
        copier.command = ["sh", "-c", "exec wl-copy --type image/png < \"$1\" >/dev/null 2>&1", "sh", root.shot.path];
        copier.running = true;
        hideTimer.restart();
    }
    function discard() {
        if (root.shot === null)
            return;
        remover.command = ["rm", "-f", "--", root.shot.path];
        remover.running = true;
        root.discarded = true;
        hideTimer.interval = 1500;
        hideTimer.restart();
    }

    visible: shown
    // The main display when one is chosen, else the focused screen.
    screen: Outputs.primaryScreen(Quickshell.screens) || screenFor(Hyprland.focusedMonitor)
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "uq-screenshot"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Ignore
    anchors.bottom: true
    anchors.right: true
    margins.bottom: WindowLook.edgeRight + Theme.gapOut
    margins.right: WindowLook.edgeRight + Theme.gapOut
    implicitWidth: cardWidth
    implicitHeight: Math.max(1, card.implicitHeight)
    mask: Region { item: card }

    IpcHandler {
        target: "screenshot"

        // "ok", or "ignored" for a path that isn't an absolute .png path or while locked.
        function show(path: string, copied: string): string {
            if (Bridge.locked)
                return "ignored";
            return root.open(path, copied) ? "ok" : "ignored";
        }
        function dismiss(): string {
            root.close();
            return "ok";
        }
    }

    Timer {
        id: hideTimer
        interval: S.SHOW_MS
        onTriggered: {
            if (hover.hovered && !root.discarded) {
                hideTimer.restart();
                return;
            }
            root.close();
        }
    }
    onShotChanged: if (shot === null) hideTimer.interval = S.SHOW_MS
    onShownChanged: if (!shown) hideTimer.stop()

    Process {
        id: copier
        onExited: function (code) {
            root.copyFailed = code !== 0;
            if (code === 0 && root.shot !== null)
                root.shot = { path: root.shot.path, copied: true };
        }
    }
    Process { id: remover }

    // A small button: the primary one is filled with the accent, the others are outlined.
    component ShotButton: Rectangle {
        id: b
        property string label: ""
        property bool primary: false
        signal activated()

        implicitHeight: 28
        implicitWidth: bText.implicitWidth + Theme.gapIn * 5
        radius: Theme.radius - 3
        color: primary ? (bMouse.containsMouse ? Qt.lighter(Theme.accent, 1.12) : Theme.accent)
                       : (bMouse.containsMouse ? Theme.surfaceAlt : "transparent")
        border.color: primary ? Theme.accent : (activeFocus ? Theme.accent : Theme.border)
        border.width: primary ? 0 : 1
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: b.label
        Accessible.onPressAction: b.activated()
        Keys.onSpacePressed: b.activated()
        Keys.onReturnPressed: b.activated()

        UqText {
            id: bText
            anchors.centerIn: parent
            text: b.label
            baseBold: b.primary
            color: b.primary ? Theme.accentText : Theme.text
        }
        MouseArea {
            id: bMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: b.activated()
        }
    }

    Rectangle {
        id: card
        width: root.cardWidth
        implicitHeight: content.implicitHeight + Theme.gapOut * 2
        radius: Theme.radius
        color: Theme.surface
        border.color: Theme.border
        border.width: Theme.borderWidth
        Accessible.role: Accessible.Pane
        Accessible.name: "Screenshot preview"

        HoverHandler { id: hover }

        ColumnLayout {
            id: content
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: Theme.gapOut
            spacing: Theme.gapIn * 2

            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.gapIn
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0
                    UqText {
                        Layout.fillWidth: true
                        text: root.shot ? S.status(root.shot, root.discarded, root.copyFailed) : ""
                        baseBold: true
                        color: root.copyFailed ? Theme.error : Theme.text
                    }
                    UqText {
                        Layout.fillWidth: true
                        text: root.shot ? S.baseName(root.shot.path) : ""
                        color: Theme.textMuted
                    }
                }
                ShotButton {
                    label: "✕"
                    Layout.alignment: Qt.AlignTop
                    onActivated: root.close()
                    Accessible.name: "Dismiss"
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.max(60, Math.min(root.imageMax, img.paintedHeight))
                radius: Theme.radius - 4
                color: Theme.bg
                clip: true
                opacity: root.discarded ? 0.35 : 1

                Image {
                    id: img
                    anchors.fill: parent
                    source: root.shot ? S.fileUrl(root.shot.path) : ""
                    asynchronous: true
                    cache: false
                    fillMode: Image.PreserveAspectFit
                    sourceSize.width: root.cardWidth * root.dpr
                    Accessible.role: Accessible.Graphic
                    Accessible.name: "The screenshot"
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.copy()
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.gapIn
                visible: !root.discarded
                ShotButton {
                    label: root.shot && root.shot.copied && !root.copyFailed ? "Copy again" : "Copy"
                    primary: true
                    onActivated: root.copy()
                }
                ShotButton {
                    label: "Discard"
                    onActivated: root.discard()
                }
                Item { Layout.fillWidth: true }
                UqText {
                    text: root.shot ? S.folderLabel(root.shot.path, Quickshell.env("HOME")) : ""
                    color: Theme.textMuted
                    Layout.maximumWidth: 150
                }
            }
        }
    }
}
