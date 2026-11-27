pragma ComponentBehavior: Bound
// The pinned squares: open an app, or fill an empty square.
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

ColumnLayout {
    id: pinned
    // Passed in from LookSeeDo.qml.
    property var root

    Layout.fillWidth: true
    spacing: Theme.gapIn
    visible: root.homeShown

    RowLayout {
        Layout.fillWidth: true
        UqText {
            Layout.fillWidth: true
            leftPadding: Theme.gapIn
            text: root.editPins ? "Pinned: choose a square to change" : "Pinned"
            color: Theme.textMuted
            font.pixelSize: Theme.fontSize - 1
        }
        Rectangle {
            id: editButton
            objectName: "lookseedoEditPins"
            implicitWidth: editLabel.implicitWidth + Theme.gapOut
            implicitHeight: 22
            radius: height / 2
            color: editMouse.containsMouse ? Theme.surfaceAlt : "transparent"
            Behavior on color { ColorAnimation { duration: Motion.fast } }
            Accessible.role: Accessible.Button
            Accessible.name: root.editPins ? "Done editing pinned apps" : "Edit pinned apps"
            UqText {
                id: editLabel
                anchors.centerIn: parent
                text: root.editPins ? "Done" : "Edit"
                color: Theme.accent
                font.pixelSize: Theme.fontSize - 1
            }
            MouseArea {
                id: editMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: root.editPins = !root.editPins
            }
        }
    }

    Row {
        id: squareRow
        Layout.fillWidth: true
        readonly property int side: 76
        spacing: Math.max(Theme.gapIn, (width - 5 * side) / 4)
        Repeater {
            model: root.squares
            delegate: Rectangle {
                id: sq
                required property var modelData
                readonly property bool filled: modelData.entry !== null
                objectName: "lookseedoSquare" + modelData.slot
                width: squareRow.side
                height: squareRow.side
                radius: Theme.radiusControl
                color: sqMouse.containsMouse ? Theme.overlay : Theme.surfaceAlt
                Behavior on color { ColorAnimation { duration: Motion.fast } }
                border.color: root.editPins ? Theme.accent : Theme.border
                border.width: Theme.borderWidth - 1
                Accessible.role: Accessible.Button
                Accessible.name: filled ? String(modelData.entry.name)
                    : "Empty square " + (modelData.slot + 1) + ": pin an app"

                IconImage {
                    visible: sq.filled
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: Theme.gapIn * 2
                    implicitSize: 32
                    backer.sourceSize.width: 32 * root.dpr
                    backer.sourceSize.height: 32 * root.dpr
                    source: sq.filled ? root.iconSource(sq.modelData.entry.icon) : ""
                }
                UqText {
                    visible: sq.filled
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.margins: Theme.gapIn
                    horizontalAlignment: Text.AlignHCenter
                    text: sq.filled ? String(sq.modelData.entry.name) : ""
                    font.pixelSize: Theme.fontSize - 1
                }
                UqText {
                    visible: !sq.filled
                    anchors.centerIn: parent
                    text: "+"
                    color: Theme.textMuted
                    font.pixelSize: 24
                }
                // Edit mode: a small mark on a filled square that it can be changed.
                UqText {
                    visible: sq.filled && root.editPins
                    anchors.top: parent.top
                    anchors.right: parent.right
                    anchors.margins: Theme.gapIn
                    text: "…"
                    color: Theme.accent
                    font.pixelSize: Theme.fontSize
                }
                MouseArea {
                    id: sqMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onClicked: function (mouse) {
                        // Empty squares, edit mode and a right click all choose the
                        // app for the square; a plain click opens the app.
                        if (!sq.filled || root.editPins || mouse.button === Qt.RightButton)
                            root.startPick(sq.modelData.slot);
                        else
                            root.openPinned(sq.modelData.slot);
                    }
                }
            }
        }
    }
}
