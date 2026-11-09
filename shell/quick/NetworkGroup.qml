pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell.Networking
import "../core"
import "../logic/quickui.js" as U

// Quick settings: the Wi-Fi switch and network list, and the wired link state.
// SSIDs are untrusted: UqText only, one line, elided. A secured network that isn't known can't be
// joined from here (the shell has no password prompt); it says "Set up in Settings" instead.
ColumnLayout {
    id: root

    // Set by QuickSettings: the window is open. The Wi-Fi scanner runs only while it is.
    property bool active: false

    readonly property var devices: Networking.devices.values
    // Plain loops: `values` is a QList sequence, which Array.concat would not spread.
    readonly property var wifiDevices: {
        var ds = devices, out = [];
        for (var i = 0; i < ds.length; i++)
            if (ds[i] && ds[i].type === DeviceType.Wifi)
                out.push(ds[i]);
        return out;
    }
    // With several wired devices (a dock and a built-in port), the connected one, then one with a
    // link, then the first.
    readonly property var wiredDevice: {
        var ds = devices, first = null, linked = null;
        for (var i = 0; i < ds.length; i++) {
            var d = ds[i];
            if (!d || d.type !== DeviceType.Wired)
                continue;
            if (d.connected)
                return d;
            if (!linked && d.hasLink)
                linked = d;
            if (!first)
                first = d;
        }
        return linked || first;
    }
    // NetworkManager reports Wi-Fi hardware as enabled when there's no rfkill switch, even with no
    // Wi-Fi device, so the toggle also needs a device.
    readonly property bool wifiShown: Networking.wifiHardwareEnabled && wifiDevices.length > 0
    // Every WifiNetwork across the Wi-Fi devices; rows index into this.
    readonly property var wifiNets: {
        var out = [];
        wifiDevices.forEach(function (d) {
            var ns = d.networks.values;
            for (var i = 0; i < ns.length; i++)
                out.push(ns[i]);
        });
        return out;
    }
    readonly property var rows: Networking.wifiEnabled ? U.wifiList(wifiNets, WifiSecurityType.Open) : []

    spacing: Theme.gapIn * 2
    visible: wifiShown || wiredDevice !== null

    function setScanner(on) {
        wifiDevices.forEach(function (d) {
            if (d.scannerEnabled !== on)
                d.scannerEnabled = on;
        });
    }
    // Deferred so a wipe() in the same turn as opening (QuickSettings wipes on every show)
    // can't leave the scanner off while the panel is open.
    function syncScanner() {
        setScanner(active);
    }
    function wipe() {
        setScanner(false);
    }

    function tap(row) {
        var n = row.index >= 0 ? wifiNets[row.index] : null;
        if (!n)
            return;
        var a = U.wifiAction(row);
        if (a === "connect")
            n.connect();
        else if (a === "disconnect")
            n.disconnect();
    }

    onActiveChanged: Qt.callLater(root.syncScanner)
    onWifiDevicesChanged: Qt.callLater(root.syncScanner)
    Component.onDestruction: setScanner(false)

    UqText {
        text: "Network"
        color: Theme.textMuted
        font.bold: true
    }

    PowerGroup.ToggleRow {
        visible: root.wifiShown
        glyph: "📶"
        label: "Wi-Fi"
        on: Networking.wifiEnabled
        onToggled: Networking.wifiEnabled = !Networking.wifiEnabled
    }

    UqText {
        visible: root.wifiShown && Networking.wifiEnabled && root.rows.length === 0
        Layout.fillWidth: true
        text: root.wifiDevices.length === 0 ? "No Wi-Fi device" : "Searching for networks…"
        color: Theme.textMuted
    }

    Repeater {
        model: root.wifiShown ? root.rows : []
        delegate: Rectangle {
            id: net
            required property var modelData
            readonly property string action: U.wifiAction(modelData)
            readonly property string status: modelData.connected ? "Connected"
                : (action === "setup" ? "Set up in Settings" : "")
            Layout.fillWidth: true
            implicitHeight: 28
            radius: Theme.radius - 2
            color: modelData.connected ? Theme.overlay
                : (netMouse.containsMouse && action !== "setup" ? Theme.surfaceAlt : "transparent")
            activeFocusOnTab: true
            border.color: Theme.accent
            border.width: activeFocus ? 1 : 0
            Accessible.role: Accessible.Button
            Accessible.name: "Wi-Fi network " + modelData.name + ", " + modelData.signal + "%"
                + (modelData.secure ? ", secured" : ", open")
                + (action === "disconnect" ? ", connected; disconnect"
                    : (action === "connect" ? "; connect" : "; set up in Settings"))
            Accessible.onPressAction: root.tap(net.modelData)
            Keys.onSpacePressed: root.tap(net.modelData)
            Keys.onReturnPressed: root.tap(net.modelData)

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Theme.gapIn * 2
                anchors.rightMargin: Theme.gapIn * 2
                spacing: Theme.gapIn * 2
                UqText {
                    Layout.preferredWidth: 34
                    horizontalAlignment: Text.AlignRight
                    text: net.modelData.signal + "%"
                    color: Theme.textMuted
                }
                UqText {
                    Layout.preferredWidth: 14
                    text: net.modelData.secure ? "🔒" : ""
                    Accessible.ignored: true
                }
                UqText {
                    Layout.fillWidth: true
                    maximumLineCount: 1
                    text: net.modelData.name
                    color: net.action === "setup" ? Theme.textMuted : Theme.text
                }
                UqText {
                    visible: net.status !== ""
                    text: net.status
                    color: net.modelData.connected ? Theme.accent : Theme.textMuted
                }
            }
            MouseArea {
                id: netMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: root.tap(net.modelData)
            }
        }
    }

    // --- wired ---
    UqText {
        visible: root.wiredDevice !== null
        Layout.fillWidth: true
        text: U.wiredLabel(root.wiredDevice)
        color: root.wiredDevice && root.wiredDevice.connected ? Theme.text : Theme.textMuted
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
}
