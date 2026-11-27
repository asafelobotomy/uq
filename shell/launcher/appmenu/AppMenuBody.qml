pragma ComponentBehavior: Bound
// The body: the categories, and the grid of the chosen one's apps.
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Widgets
import "../../core"
import "../../session"
import "../../logic/launcher.js" as L
import "../../logic/rank.js" as Rank
import "../../logic/appmenu.js" as M
import "../../logic/failure.js" as F

RowLayout {
    // Passed in from AppMenu.qml.
    property var actionMenu
    property var field
    property var root
    // The grid, for AppMenu.qml's keys and resets.
    property alias appGrid: grid

    Layout.fillWidth: true
    Layout.fillHeight: true
    spacing: Theme.gapIn * 2

    ListView {
        id: cats
        Layout.preferredWidth: 112
        Layout.fillHeight: true
        clip: true
        spacing: 2
        boundsBehavior: Flickable.StopAtBounds
        model: root.categoryList
        delegate: Rectangle {
            id: catRow
            required property var modelData
            readonly property bool current: !root.searching
                && root.currentCategory === modelData.key
            width: cats.width
            height: 28
            radius: Theme.radiusControl
            color: current ? Theme.overlay
                : (catMouse.containsMouse ? Theme.surfaceAlt : "transparent")
            Behavior on color { ColorAnimation { duration: Motion.fast } }
            Accessible.role: Accessible.Button
            Accessible.name: modelData.key

            UqText {
                anchors.fill: parent
                anchors.leftMargin: Theme.gapIn * 2
                anchors.rightMargin: Theme.gapIn
                verticalAlignment: Text.AlignVCenter
                text: catRow.modelData.key
                color: catRow.current ? Theme.text : Theme.textMuted
            }
            MouseArea {
                id: catMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: {
                    root.currentCategory = catRow.modelData.key;
                    field.text = "";
                    root.query = "";
                    field.forceActiveFocus();
                }
            }
        }
    }

    Rectangle {
        Layout.preferredWidth: 1
        Layout.fillHeight: true
        color: Theme.border
    }

    Item {
        Layout.fillWidth: true
        Layout.fillHeight: true

        GridView {
            id: grid
            anchors.fill: parent
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            cellWidth: Math.floor(width / root.columns)
            cellHeight: 76
            model: root.gridModel
            currentIndex: 0
            delegate: Rectangle {
                id: cell
                required property var modelData
                required property int index
                readonly property bool selected: grid.currentIndex === index
                width: grid.cellWidth - 2
                height: grid.cellHeight - 2
                radius: Theme.radiusControl
                color: selected ? Theme.overlay
                    : (cellMouse.containsMouse ? Theme.surfaceAlt : "transparent")
                Behavior on color { ColorAnimation { duration: Motion.fast } }
                Accessible.role: Accessible.Button
                Accessible.name: String(modelData.name || "")

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 4
                    spacing: 2

                    IconImage {
                        Layout.alignment: Qt.AlignHCenter
                        Layout.preferredWidth: root.iconSize
                        Layout.preferredHeight: root.iconSize
                        implicitSize: root.iconSize
                        backer.sourceSize.width: root.iconSize * root.dpr
                        backer.sourceSize.height: root.iconSize * root.dpr
                        // Icon files decode off the GUI thread, so the menu's first frame doesn't wait on them.
                        // Theme icons (image://icon) load on the GUI thread: QIcon is not thread-safe.
                        source: root.iconSource(cell.modelData.icon)
                        backer.asynchronous: L.asyncImageOk(source)
                    }
                    UqText {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        maximumLineCount: 2
                        font.pixelSize: Theme.fontSize - 1
                        text: String(cell.modelData.name || "")
                    }
                }
                MouseArea {
                    id: cellMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onClicked: function (mouse) {
                        if (mouse.button === Qt.RightButton)
                            actionMenu.openAt(cell, mouse.x, mouse.y,
                                              String(cell.modelData.id || ""));
                        else
                            root.launch(cell.modelData);
                    }
                }
            }
        }

        UqText {
            anchors.centerIn: parent
            visible: root.gridModel.length === 0
            text: root.searching ? "No matching apps"
                : (root.currentCategory === M.FAVOURITES
                   ? "No favourites yet: pin apps from the dock" : "No apps")
            color: Theme.textMuted
        }
    }
}
