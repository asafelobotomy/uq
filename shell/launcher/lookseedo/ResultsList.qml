pragma ComponentBehavior: Bound
// The results: each section's heading and rows, scrolled past maxListHeight.
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

Flickable {
    id: flick
    // Passed in from LookSeeDo.qml.
    property var actionMenu
    property var root

    Layout.fillWidth: true
    Layout.preferredHeight: Math.min(results.implicitHeight, root.maxListHeight)
    // Recents and the picker show with an empty field; once there is text the
    // list is the search's, and a `>` command has the terminal instead.
    visible: root.sections.length > 0 && !root.runMode && !root.dockStrip
             && !(root.query.trim() !== "" && root.searched === "" && root.pinSlot < 0)
    contentHeight: results.implicitHeight
    clip: true
    boundsBehavior: Flickable.StopAtBounds

    function ensureVisible(item) {
        var p = item.mapToItem(results, 0, 0);
        if (p.y < contentY)
            contentY = p.y;
        else if (p.y + item.height > contentY + height)
            contentY = p.y + item.height - height;
    }

    Column {
        id: results
        width: flick.width
        spacing: 2

        Repeater {
            model: root.sections
            delegate: Column {
                id: sec
                required property var modelData
                required property int index
                width: results.width
                spacing: 2

                UqText {
                    width: parent.width
                    visible: text !== ""
                    leftPadding: Theme.gapIn
                    topPadding: sec.index > 0 ? Theme.gapIn : 0
                    text: sec.modelData.top ? "Top hit" : sec.modelData.providerName
                    color: Theme.textMuted
                    font.pixelSize: Theme.fontSize - 1
                }

                Repeater {
                    model: sec.modelData.items
                    delegate: Rectangle {
                        id: row
                        required property var modelData
                        required property int index
                        readonly property bool isApp: sec.modelData.provider === "apps"
                        readonly property bool selected: root.selection.group === sec.index
                            && root.selection.index === index
                        width: results.width
                        height: root.rowHeight
                        radius: Theme.radiusControl
                        color: selected ? Theme.overlay
                            : (rowMouse.containsMouse ? Theme.surfaceAlt : "transparent")
                        Behavior on color { ColorAnimation { duration: Motion.fast } }
                        Accessible.role: Accessible.ListItem
                        Accessible.name: isApp ? String(modelData.name || "")
                            : String(modelData.title || "")
                        onSelectedChanged: if (selected) flick.ensureVisible(row)

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: Theme.gapIn
                            anchors.rightMargin: Theme.gapIn
                            spacing: Theme.gapIn * 2

                            IconImage {
                                Layout.preferredWidth: root.iconSize
                                Layout.preferredHeight: root.iconSize
                                implicitSize: root.iconSize
                                backer.sourceSize.width: root.iconSize * root.dpr
                                backer.sourceSize.height: root.iconSize * root.dpr
                                source: root.iconSource(row.modelData.icon)
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 0
                                UqText {
                                    Layout.fillWidth: true
                                    text: String((row.isApp ? row.modelData.name
                                                            : row.modelData.title) || "")
                                }
                                UqText {
                                    Layout.fillWidth: true
                                    visible: text !== ""
                                    text: String((row.isApp ? row.modelData.comment
                                                            : row.modelData.subtitle) || "")
                                    color: Theme.textMuted
                                    font.pixelSize: Theme.fontSize - 1
                                }
                            }
                            // The top hit names where it came from; other rows sit
                            // under their group's heading.
                            UqText {
                                visible: sec.modelData.top === true
                                Layout.maximumWidth: root.width * 0.3
                                text: String(sec.modelData.providerName || "")
                                color: Theme.textMuted
                                font.pixelSize: Theme.fontSize - 1
                            }
                        }
                        MouseArea {
                            id: rowMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            onClicked: function (mouse) {
                                if (mouse.button === Qt.RightButton) {
                                    // App actions from other apps (Phase 4 §2.4.2).
                                    if (row.isApp && sec.modelData.pick !== true && row.modelData.id)
                                        actionMenu.openAt(row, mouse.x, mouse.y,
                                                          String(row.modelData.id || ""));
                                } else {
                                    root.activateClicked(sec.index, row.index);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
