// The strip: the lead icon, the scroll arrows, the tabs and +N.
import QtQuick
import QtQuick.Effects
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import "../../core"
import "../../services"
import "../../notif"
import "../islands"
import "../../logic/minimise.js" as Min
import "../../logic/work.js" as Work
import "../../logic/dock.js" as Dock

RowLayout {
    id: row
    // Passed in from WorkIsland.qml.
    property var root
    // The tabs, for WorkIsland.qml's dragging (their left edges and widths).
    property alias tabsRep: tabsRepeater

    anchors.fill: parent
    spacing: root.metrics.gap

    // No windows here.
    UqText {
        visible: root.fit.mode === "empty" && !root.isIcon
        text: "Desktop"
        color: Theme.textMuted
        Layout.fillWidth: true
    }
    IconImage {
        visible: root.fit.mode === "empty" && root.isIcon
        Layout.alignment: Qt.AlignCenter
        implicitSize: root.iconSize
        source: Quickshell.iconPath("user-desktop", true)
    }

    // Not even one pill fits: the lead window's icon (and the count, when there's room).
    Rectangle {
        id: compact
        visible: root.fit.mode === "compact"
        Layout.fillWidth: true
        Layout.fillHeight: true
        radius: Theme.radius - 2
        color: compactMouse.containsMouse ? Theme.surfaceAlt : "transparent"
        Accessible.role: Accessible.Button
        Accessible.name: root.ordered.length + " windows open; list them"
        Row {
            anchors.centerIn: parent
            spacing: 2
            IconImage {
                anchors.verticalCenter: parent.verticalCenter
                implicitSize: root.iconSize
                source: root.lead ? root.iconFor(root.lead.cls) : ""
                AlertDot {
                    root: row.root
                    shown: root.alertHere
                }
            }
            UqText {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.room >= root.iconSize + Theme.fontSize * 2
                text: String(root.ordered.length)
                color: Theme.textMuted
            }
        }
        MouseArea {
            id: compactMouse
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: function (m) {
                if (m.button === Qt.RightButton && root.lead)
                    root.openMenu(compact, root.lead);
                else
                    root.showList(compact);
            }
        }
    }

    Arrow {
        root: row.root
        visible: root.fit.mode === "scroll"
        glyph: "‹"
        label: "Scroll the windows left"
        can: root.start > 0
        onPressed: root.offset = root.start - 1
    }

    Repeater {
        id: tabsRepeater
        // The windows' addresses: replaced only when windows come, go or change places, so
        // tabs aren't rebuilt (and their widths re-animated) every time a window moves.
        model: root.tabKeys
        delegate: WorkTab {
            id: pill
            root: row.root
        }
    }

    Arrow {
        root: row.root
        visible: root.fit.mode === "scroll"
        glyph: "›"
        label: "Scroll the windows right"
        can: root.start + root.fit.shown < root.ordered.length
        onPressed: root.offset = root.start + 1
    }
    // "+N": every window, in a list.
    Rectangle {
        id: moreChip
        visible: root.fit.mode === "scroll"
        Layout.preferredWidth: root.metrics.chip
        Layout.fillHeight: true
        radius: Theme.radius - 2
        color: moreMouse.containsMouse ? Theme.surfaceAlt : "transparent"
        Accessible.role: Accessible.Button
        Accessible.name: (root.ordered.length - root.fit.shown) + " more windows"
            + (root.alertHidden ? ", one wants attention" : "") + "; list them all"
        UqText {
            anchors.centerIn: parent
            text: "+" + (root.ordered.length - root.fit.shown)
            color: Theme.textMuted
            elide: Text.ElideNone
        }
        AlertDot {
            root: row.root
            shown: root.alertHidden
            anchors.rightMargin: 1
            anchors.topMargin: 1
        }
        MouseArea {
            id: moreMouse
            anchors.fill: parent
            hoverEnabled: true
            onClicked: root.showList(moreChip)
        }
    }
    // Pills that don't fill the island leave the rest empty.
    Item {
        visible: root.fit.mode === "pills"
        Layout.fillWidth: true
    }
}
