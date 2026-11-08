import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import "../core"

// One bar per screen: five floating islands (System, Work, Time/weather, Notifications, Status).
// Time and Status are empty placeholders until their own tasks land.
PanelWindow {
    id: bar
    required property ShellScreen modelData
    screen: modelData

    readonly property int islandHeight: 36

    WlrLayershell.namespace: "uq-bar"
    anchors { top: true; left: true; right: true }
    margins { top: Theme.gapOut; left: Theme.gapOut; right: Theme.gapOut }
    implicitHeight: islandHeight
    exclusiveZone: islandHeight + Theme.gapOut
    color: "transparent"

    RowLayout {
        anchors.fill: parent
        spacing: Theme.gapIn

        SystemIsland {
            Layout.fillHeight: true
            screen: bar.modelData
        }
        WorkIsland {
            Layout.fillHeight: true
            screen: bar.modelData
            barWidth: bar.width
        }
        Item { Layout.fillWidth: true }
        TimeIsland { Layout.fillHeight: true }
        NotifIsland { Layout.fillHeight: true }
        Island { Layout.fillHeight: true; Layout.preferredWidth: 160 }   // Status
    }
}
