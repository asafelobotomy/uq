import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import "../core"
import "../logic/shellprefs.js" as ShellPrefs

// One bar per screen: five floating islands (System, Work, Time/weather, Notifications, Status).
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

    // Stay awake (quick settings): blocks every idle stage while on. One per bar; any one held
    // inhibits idle for the session.
    IdleInhibitor {
        window: bar
        enabled: StayAwake.enabled
    }

    RowLayout {
        anchors.fill: parent
        spacing: Theme.gapIn

        SystemIsland {
            visible: ShellPrefs.islandOn(Settings.values.shell, "system")
            Layout.fillHeight: true
            screen: bar.modelData
        }
        WorkIsland {
            visible: ShellPrefs.islandOn(Settings.values.shell, "work")
            Layout.fillHeight: true
            screen: bar.modelData
            barWidth: bar.width
        }
        Item { Layout.fillWidth: true }
        TimeIsland {
            Layout.fillHeight: true
            visible: ShellPrefs.islandOn(Settings.values.shell, "time")
        }
        NotifIsland {
            visible: ShellPrefs.islandOn(Settings.values.shell, "notif")
            Layout.fillHeight: true
            screen: bar.modelData
        }
        StatusIsland {
            visible: ShellPrefs.islandOn(Settings.values.shell, "status")
            Layout.fillHeight: true
            screen: bar.modelData
        }
    }
}
