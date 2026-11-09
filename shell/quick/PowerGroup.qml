pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Widgets
import Quickshell.Services.UPower
import "../core"
import "../notif"
import "../logic/quick.js" as Q
import "../logic/quickui.js" as U

// Quick settings: power profile, battery, Do Not Disturb and Stay awake. Each row shows only
// when its backend is there: power-profiles-daemon, a present laptop battery, noti-fi.
ColumnLayout {
    id: root

    // PowerProfiles exposes no "daemon present" property in 0.3.1, and its profile is an enum
    // that reads PowerSaver (0) when the daemon is absent. So the selector shows only when the
    // profile is defined and the daemon's D-Bus service file is installed (either bus name).
    property bool ppdInstalled: ppdNew.loaded || ppdOld.loaded
    readonly property bool hasProfiles: ppdInstalled && PowerProfiles.profile !== undefined
    readonly property var battery: U.batteryShown(UPower.displayDevice) ? UPower.displayDevice : null
    readonly property string batteryText: {
        var b = battery;
        if (!b)
            return "";
        var charging = b.state === UPowerDeviceState.Charging;
        return Q.batteryLabel(b.percentage * 100, charging,
            b.state === UPowerDeviceState.Discharging ? b.timeToEmpty : 0);
    }

    spacing: Theme.gapIn * 2

    FileView {
        id: ppdNew
        path: "/usr/share/dbus-1/system-services/org.freedesktop.UPower.PowerProfiles.service"
        printErrors: false
    }
    FileView {
        id: ppdOld
        path: "/usr/share/dbus-1/system-services/net.hadess.PowerProfiles.service"
        printErrors: false
    }

    // A labelled on/off row.
    component ToggleRow: Rectangle {
        id: t
        property string label: ""
        property string glyph: ""
        property bool on: false
        signal toggled()

        Layout.fillWidth: true
        implicitHeight: 32
        radius: Theme.radius - 2
        color: tMouse.containsMouse ? Theme.overlay : Theme.surfaceAlt
        activeFocusOnTab: true
        border.color: Theme.accent
        border.width: activeFocus ? 1 : 0
        Accessible.role: Accessible.CheckBox
        Accessible.checkable: true
        Accessible.checked: on
        Accessible.name: label
        Accessible.onPressAction: t.toggled()
        Accessible.onToggleAction: t.toggled()
        Keys.onSpacePressed: t.toggled()
        Keys.onReturnPressed: t.toggled()

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Theme.gapIn * 2
            anchors.rightMargin: Theme.gapIn * 2
            spacing: Theme.gapIn * 2
            UqText {
                Layout.preferredWidth: 18
                horizontalAlignment: Text.AlignHCenter
                text: t.glyph
            }
            UqText {
                Layout.fillWidth: true
                text: t.label
            }
            // switch
            Rectangle {
                Layout.preferredWidth: 30
                Layout.preferredHeight: 16
                radius: 8
                color: t.on ? Theme.accent : Theme.overlay
                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    x: t.on ? parent.width - width - 2 : 2
                    width: 12
                    height: 12
                    radius: 6
                    color: t.on ? Theme.accentText : Theme.textMuted
                }
            }
        }
        MouseArea {
            id: tMouse
            anchors.fill: parent
            hoverEnabled: true
            onClicked: t.toggled()
        }
    }

    UqText {
        text: "Power"
        color: Theme.textMuted
        font.bold: true
    }

    // --- battery ---
    RowLayout {
        visible: root.battery !== null
        Layout.fillWidth: true
        spacing: Theme.gapIn * 2
        Accessible.role: Accessible.StaticText
        Accessible.name: "Battery " + root.batteryText

        IconImage {
            implicitSize: 18
            Layout.preferredWidth: 18
            Layout.preferredHeight: 18
            source: root.battery ? Quickshell.iconPath(root.battery.iconName, true) : ""
        }
        UqText {
            Layout.fillWidth: true
            text: root.batteryText
        }
    }

    // --- power profile ---
    RowLayout {
        visible: root.hasProfiles
        Layout.fillWidth: true
        spacing: Theme.gapIn

        Repeater {
            model: U.profileOptions(PowerProfiles.hasPerformanceProfile)
            delegate: Rectangle {
                id: seg
                required property int modelData
                readonly property bool current: PowerProfiles.profile === modelData
                readonly property string label: Q.profileLabel(modelData)
                Layout.fillWidth: true
                implicitHeight: 28
                radius: Theme.radius - 2
                color: current ? Theme.accent : (segMouse.containsMouse ? Theme.overlay : Theme.surfaceAlt)
                activeFocusOnTab: true
                border.color: Theme.accent
                border.width: activeFocus && !current ? 1 : 0
                Accessible.role: Accessible.RadioButton
                Accessible.checkable: true
                Accessible.checked: current
                Accessible.name: "Power profile " + label
                Accessible.onPressAction: seg.choose()
                Keys.onSpacePressed: seg.choose()
                Keys.onReturnPressed: seg.choose()

                function choose() {
                    if (!seg.current)
                        PowerProfiles.profile = seg.modelData;
                }

                UqText {
                    anchors.fill: parent
                    anchors.leftMargin: Theme.gapIn
                    anchors.rightMargin: Theme.gapIn
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    text: seg.label
                    color: seg.current ? Theme.accentText : Theme.text
                }
                MouseArea {
                    id: segMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: seg.choose()
                }
            }
        }
    }

    // --- Do Not Disturb (noti-fi only in this slice) ---
    ToggleRow {
        visible: Notifi.available
        glyph: "🔕"
        label: "Do Not Disturb"
        on: Notifi.dnd
        onToggled: Notifi.setDnd(!Notifi.dnd).catch(function () {})
    }

    // --- Stay awake ---
    ToggleRow {
        glyph: "☕"
        label: "Stay awake"
        on: StayAwake.enabled
        onToggled: StayAwake.enabled = !StayAwake.enabled
    }
}
