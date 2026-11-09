pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell.Bluetooth
import "../core"
import "../logic/quickui.js" as U

// Quick settings: the Bluetooth adapter switch and paired devices (connect/disconnect, battery).
// Hidden without an adapter. Device names are untrusted: UqText only, one line, elided.
ColumnLayout {
    id: root

    readonly property var adapter: Bluetooth.defaultAdapter
    readonly property bool enabledNow: adapter ? adapter.enabled : false
    readonly property var paired: adapter && enabledNow ? U.btDevices(adapter.devices.values) : []

    visible: adapter !== null
    spacing: Theme.gapIn * 2

    UqText {
        text: "Bluetooth"
        color: Theme.textMuted
        font.bold: true
    }

    ToggleRow {
        glyph: "ᛒ"
        label: "Bluetooth"
        on: root.enabledNow
        onToggled: if (root.adapter) root.adapter.enabled = !root.adapter.enabled
    }

    UqText {
        visible: root.enabledNow && root.paired.length === 0
        Layout.fillWidth: true
        text: "No paired devices"
        color: Theme.textMuted
    }

    Repeater {
        model: root.paired
        delegate: Rectangle {
            id: dev
            required property var modelData
            readonly property string label: U.btLabel(modelData)
            readonly property string battery: U.btBattery(modelData)
            readonly property bool busy: modelData.state === BluetoothDeviceState.Connecting
                || modelData.state === BluetoothDeviceState.Disconnecting
            readonly property string status: busy
                ? (modelData.state === BluetoothDeviceState.Connecting ? "Connecting…" : "Disconnecting…")
                : (modelData.connected ? "Connected" : "")

            function toggle() {
                if (dev.busy)
                    return;
                if (dev.modelData.connected)
                    dev.modelData.disconnect();
                else
                    dev.modelData.connect();
            }

            Layout.fillWidth: true
            implicitHeight: 28
            radius: Theme.radius - 2
            color: modelData.connected ? Theme.overlay : (devMouse.containsMouse ? Theme.surfaceAlt : "transparent")
            activeFocusOnTab: true
            border.color: Theme.accent
            border.width: activeFocus ? 1 : 0
            Accessible.role: Accessible.Button
            Accessible.name: "Bluetooth device " + label
                + (battery !== "" ? ", battery " + battery : "")
                + (modelData.connected ? ", connected; disconnect" : "; connect")
            Accessible.onPressAction: dev.toggle()
            Keys.onSpacePressed: dev.toggle()
            Keys.onReturnPressed: dev.toggle()

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Theme.gapIn * 2
                anchors.rightMargin: Theme.gapIn * 2
                spacing: Theme.gapIn * 2
                UqText {
                    Layout.fillWidth: true
                    maximumLineCount: 1
                    text: dev.label
                }
                UqText {
                    visible: dev.battery !== ""
                    text: "🔋 " + dev.battery
                    color: Theme.textMuted
                }
                UqText {
                    visible: dev.status !== ""
                    text: dev.status
                    color: dev.modelData.connected ? Theme.accent : Theme.textMuted
                }
            }
            MouseArea {
                id: devMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: dev.toggle()
            }
        }
    }
}
