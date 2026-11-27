pragma ComponentBehavior: Bound
// The default terminal, dropped down from the pill with the command being typed. Enter opens the real one in its place.
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Widgets
import "../../core"
import "../../logic/launcher.js" as L
import "../../logic/rank.js" as Rank
import "../../logic/commands.js" as Commands
import "../../logic/quickui.js" as QuickUi
import "../../logic/shellprefs.js" as ShellPrefs
import "../../logic/failure.js" as F
import "../../logic/lookhome.js" as H

Rectangle {
    id: termDrop
    // Passed in from LookSeeDo.qml.
    property var root

    objectName: "lookseedoTerminal"
    Layout.fillWidth: true
    readonly property bool open: root.runMode && root.termState !== "sent" && !root.docked
    Layout.preferredHeight: open ? root.termHeight : 0
    visible: Layout.preferredHeight > 0
    clip: true
    radius: Theme.radiusControl
    color: Theme.bg
    border.color: Theme.border
    border.width: Theme.borderWidth - 1
    Behavior on Layout.preferredHeight {
        NumberAnimation { duration: Motion.base; easing.type: Motion.easing }
    }

    Rectangle {
        id: termBar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: 28
        color: Theme.surfaceAlt
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Theme.gapOut
            anchors.rightMargin: Theme.gapOut
            spacing: Theme.gapIn * 2
            IconImage {
                Layout.preferredWidth: 16
                Layout.preferredHeight: 16
                implicitSize: 16
                source: root.defaultTerminal && root.defaultTerminal.icon
                    ? root.iconSource(root.defaultTerminal.icon)
                    : root.iconSource("utilities-terminal")
            }
            UqText {
                Layout.fillWidth: true
                text: root.termName
                color: Theme.textMuted
                font.pixelSize: Theme.fontSize - 1
            }
        }
    }
    UqText {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: termBar.bottom
        anchors.margins: Theme.gapOut
        baseFamily: Theme.fontMono
        wrapMode: Text.WrapAnywhere
        maximumLineCount: 7
        // The same "prompt, command, cursor" the real terminal opens with.
        text: "$ " + root.command + "\u2588"
        color: Theme.text
    }
    UqText {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: Theme.gapOut
        visible: root.termState === "sending"
        text: "Opening…"
        color: Theme.textMuted
        font.pixelSize: Theme.fontSize - 1
    }
}
