import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import Quickshell.Networking
import Quickshell.Services.Mpris
import Quickshell.Services.Pipewire
import Quickshell.Services.SystemTray
import Quickshell.Services.UPower
import "../core"
import "../services"
import "../widgets"
import "../logic/net.js" as Net
import "../logic/tray.js" as Tray
import "../notif"
import "islands"

// Widgets from other apps, tray, media, network/VPN, audio, battery and the quick-settings
// button. While a VPN is up a lock sits beside the network icon; a click on it shows the VPN's
// name, type, server, addresses and where the internet sees the computer. Other apps' tray icons move to the Tray island while it is on the bar (as media
// moves to Now playing).
// Tray tooltips, track titles, SSIDs and VPN names are untrusted: they only reach UqText.
Island {
    id: root
    property var screen: null
    readonly property int iconSize: 18
    readonly property int cell: 26

    // Sizes: Icon is the quick settings button; Small adds the battery (else the network);
    // Medium shows network, sound, battery (with its level) and the button; Large adds the
    // volume level, then, in what is left, widgets from other apps, media and tray icons.
    readonly property bool wideTier: size === "medium" || size === "large"
    // The network counts as its icon here, and the VPN lock (beside it while a VPN is up) as one
    // more icon.
    readonly property bool showVpn: netItem.visible && vpnActive
    readonly property real fixedWidth: {
        var w = 0, n = 0;
        var parts = [audioItem, batteryItem, quickButton];
        for (var i = 0; i < parts.length; i++)
            if (parts[i].visible) {
                w += parts[i].implicitWidth > 0 ? parts[i].implicitWidth : parts[i].Layout.preferredWidth;
                n++;
            }
        if (netItem.visible) {
            w += iconSize + (showVpn ? 4 + iconSize : 0);
            n++;
        }
        return w + Math.max(0, n - 1) * Theme.gapIn;
    }
    readonly property real spare: size === "large"
        ? Math.max(0, room - fixedWidth - Theme.gapIn) : 0
    readonly property bool showWidgets: size === "large" && widgetHost.wanted && widgetHost.implicitWidth + Theme.gapIn <= spare
    readonly property real afterWidgets: spare - (showWidgets ? widgetHost.implicitWidth + Theme.gapIn : 0)
    readonly property bool showMedia: size === "large" && player !== null && !BarLayout.placed("media")
        && afterWidgets >= cell + Theme.gapIn
    readonly property int trayRoom: Math.max(0, Math.floor((afterWidgets - (showMedia ? cell + Theme.gapIn : 0)) / (cell + Theme.gapIn)))
    readonly property var trayShown: {
        if (size !== "large" || BarLayout.placed("tray"))
            return [];
        var all = SystemTray.items.values;
        var bell = Tray.notifiBell(NotifBackend.backend, Tray.find(all, Tray.NOTIFI));
        return Tray.shownItems(all, { notifiBell: bell, securityIcon: BarLayout.placed("security") }).slice(0, trayRoom);
    }

    // --- media ---
    readonly property var player: {
        var ps = Mpris.players.values;
        for (var i = 0; i < ps.length; i++)
            if (ps[i].isPlaying)
                return ps[i];
        return null;
    }

    // --- network ---
    readonly property var netDevice: {
        var ds = Networking.devices.values, wired = null;
        for (var i = 0; i < ds.length; i++) {
            if (!ds[i].connected)
                continue;
            if (ds[i].type === DeviceType.Wifi)
                return ds[i];
            if (ds[i].type === DeviceType.Wired && wired === null)
                wired = ds[i];
        }
        return wired;
    }
    readonly property real wifiStrength: {
        if (!netDevice || netDevice.type !== DeviceType.Wifi)
            return 0;
        var ns = netDevice.networks.values;
        for (var i = 0; i < ns.length; i++)
            if (ns[i].connected)
                return ns[i].signalStrength;
        return 0;
    }
    readonly property string netIcon: !netDevice ? "network-offline-symbolic"
        : (netDevice.type === DeviceType.Wifi ? Net.wifiIcon(wifiStrength) : "network-wired-symbolic")

    // The VPN, read once for every screen (services/VpnStatus.qml).
    readonly property bool vpnActive: VpnStatus.vpnActive
    readonly property string vpnLabel: VpnStatus.vpnLabel
    readonly property var ip: Bridge.publicIp
    readonly property string country: ip && ip.country_code && !Bridge.sharing ? String(ip.country_code) : ""

    // The lock: the icon theme's VPN icon (a padlock in Papirus, Breeze and Adwaita), or its
    // plain padlock.
    readonly property string vpnIcon: Quickshell.iconPath("network-vpn-symbolic", true) !== ""
        ? "network-vpn-symbolic" : "changes-prevent-symbolic"
    readonly property var vpnRows: Net.vpnRows(vpnLabel, VpnStatus.vpnConn ? VpnStatus.nmDetails : null,
        VpnStatus.tsDetails, ip, Bridge.sharing)
    readonly property bool ipLookupOn: !(Settings.values.privacy && Settings.values.privacy.public_ip_lookup === false)
    function toggleVpn() {
        if (vpnPopup.open && vpnPopup.anchorItem === vpnButton) {
            vpnPopup.hide();
        } else {
            VpnStatus.refreshDetails();
            vpnPopup.show(vpnButton, vpnDetails);
        }
    }
    LazyPopup {
        id: vpnPopup
    }
    Component {
        id: vpnDetails
        ColumnLayout {
            spacing: Theme.gapIn
            width: Theme.fontSize * 24
            RowLayout {
                spacing: Theme.gapIn
                IconImage {
                    implicitSize: root.iconSize
                    Layout.preferredWidth: root.iconSize
                    Layout.preferredHeight: root.iconSize
                    source: Quickshell.iconPath(root.vpnIcon, true)
                }
                UqText {
                    text: root.vpnActive ? "VPN connected" : "No VPN"
                    baseBold: true
                    Layout.fillWidth: true
                }
            }
            GridLayout {
                columns: 2
                columnSpacing: Theme.gapIn * 2
                rowSpacing: 2
                Layout.fillWidth: true
                Repeater {
                    model: root.vpnRows
                    delegate: UqText {
                        required property var modelData
                        required property int index
                        Layout.row: index
                        Layout.column: 0
                        text: modelData.label
                        color: Theme.textMuted
                    }
                }
                Repeater {
                    model: root.vpnRows
                    delegate: UqText {
                        required property var modelData
                        required property int index
                        Layout.row: index
                        Layout.column: 1
                        Layout.fillWidth: true
                        text: modelData.value
                        baseFamily: modelData.label.indexOf("address") >= 0 ? Theme.fontMono : Theme.fontUi
                    }
                }
            }
            UqText {
                visible: Bridge.sharing
                text: "Where you connect from is hidden while the screen is shared."
                color: Theme.textMuted
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            UqText {
                visible: !Bridge.sharing && !root.ipLookupOn
                text: "Turn on \"Look up my public IP address online\" in Settings to see where you appear to be."
                color: Theme.textMuted
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
        }
    }

    // --- audio ---
    readonly property var sink: Pipewire.defaultAudioSink
    readonly property var sinkAudio: sink ? sink.audio : null
    PwObjectTracker {
        objects: root.sink ? [root.sink] : []
    }

    // --- battery ---
    readonly property var battery: {
        var ds = UPower.devices.values;
        for (var i = 0; i < ds.length; i++)
            if (ds[i].isLaptopBattery)
                return ds[i];
        var d = UPower.displayDevice;
        return d && d.isLaptopBattery ? d : null;
    }

    // A widget's popup, or the widgets that didn't fit, under their icon.
    WidgetPopup {
        id: widgetPopup
    }
    Component {
        id: overflowList
        Column {
            spacing: Theme.gapIn
            Repeater {
                model: widgetHost.overflow
                delegate: Row {
                    id: orow
                    required property var modelData
                    spacing: Theme.gapIn * 2
                    // A widget that hid itself has no row.
                    visible: !oslot.hiddenByWidget
                    Slot {
                        id: oslot
                        kind: "bar"
                        cell: root.cell
                        widget: orow.modelData
                    }
                    UqText {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.min(implicitWidth, 220)
                        text: orow.modelData.name
                    }
                }
            }
        }
    }

    RowLayout {
        id: row
        anchors.centerIn: parent
        spacing: Theme.gapIn

        // Widgets from other apps (Phase 4 spec §2.2): up to 4 icons, the rest under "…".
        WidgetHost {
            id: widgetHost
            kind: "bar"
            cell: root.cell
            readonly property bool wanted: loadedCount > 0 || overflow.length > 0
            visible: root.showWidgets
            Layout.preferredWidth: implicitWidth
            Layout.preferredHeight: root.cell
            onPopupRequested: function (slot, open) {
                if (open && slot.popup)
                    widgetPopup.show(slot, slot.popup, slot.ctx);
                else if (widgetPopup.anchorItem === slot)
                    widgetPopup.hide();
            }
            onOverflowClicked: function (anchor) {
                if (widgetPopup.open && widgetPopup.anchorItem === anchor)
                    widgetPopup.hide();
                else
                    widgetPopup.show(anchor, overflowList);
            }
        }

        // tray: as many as fit at Large (icons uQ already shows elsewhere stay out, logic/tray.js),
        // unless the Tray island is on the bar
        Repeater {
            model: root.trayShown
            delegate: TrayIcon {
                cell: root.cell
                iconSize: root.iconSize
                Layout.preferredWidth: root.cell
                Layout.preferredHeight: root.cell
            }
        }

        // media (left to the Now playing island while that is on the bar)
        Item {
            visible: root.showMedia
            Layout.preferredWidth: root.cell
            Layout.preferredHeight: root.cell
            Accessible.role: Accessible.Button
            Accessible.name: root.player ? "Play or pause " + String(root.player.trackTitle) : "Media"
            RowLayout {
                id: mediaRow
                anchors.verticalCenter: parent.verticalCenter
                spacing: 4
            IconImage {
                implicitSize: root.iconSize
                Layout.preferredWidth: root.iconSize
                Layout.preferredHeight: root.iconSize
                source: Quickshell.iconPath("media-playback-start-symbolic", true)
            }
            }
            MouseArea {
                anchors.fill: parent
                onClicked: if (root.player) root.player.togglePlaying()
                onWheel: function (w) {
                    if (root.player && root.player.canGoNext)
                        root.player.next();
                }
            }
        }

        // network
        RowLayout {
            id: netItem
            visible: !root.isIcon && (root.size !== "small" || root.battery === null)
            spacing: 4
            IconImage {
                implicitSize: root.iconSize
                Layout.preferredWidth: root.iconSize
                Layout.preferredHeight: root.iconSize
                source: Quickshell.iconPath(root.netIcon, true)
            }
            // The VPN: a lock, the same size and kind as the icons beside it.
            Item {
                id: vpnButton
                objectName: "vpnLock"
                visible: root.showVpn
                readonly property bool open: vpnPopup.open && vpnPopup.anchorItem === vpnButton
                Layout.preferredWidth: root.iconSize
                Layout.preferredHeight: root.cell
                Accessible.role: Accessible.Button
                Accessible.name: "VPN: " + root.vpnLabel
                Accessible.onPressAction: root.toggleVpn()
                Rectangle {
                    anchors.centerIn: parent
                    width: root.iconSize + 6
                    height: root.iconSize + 6
                    radius: Theme.radius - 2
                    color: Theme.overlay
                    visible: vpnButton.open || vpnMouse.containsMouse
                }
                IconImage {
                    anchors.centerIn: parent
                    implicitSize: root.iconSize
                    source: Quickshell.iconPath(root.vpnIcon, true)
                }
                MouseArea {
                    id: vpnMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: root.toggleVpn()
                }
            }
        }

        // audio
        Item {
            id: audioItem
            visible: root.wideTier && root.sinkAudio !== null
            Layout.preferredWidth: audioRow.implicitWidth
            Layout.preferredHeight: root.cell
            RowLayout {
                id: audioRow
                anchors.verticalCenter: parent.verticalCenter
                spacing: 4
            IconImage {
                implicitSize: root.iconSize
                Layout.preferredWidth: root.iconSize
                Layout.preferredHeight: root.iconSize
                opacity: root.sinkAudio && root.sinkAudio.muted ? 0.5 : 1
                source: Quickshell.iconPath(root.sinkAudio
                    ? Net.volumeIcon(root.sinkAudio.volume, root.sinkAudio.muted)
                    : "audio-volume-muted-symbolic", true)
            }
            UqText {
                visible: root.size === "large"
                text: root.sinkAudio ? Math.round(root.sinkAudio.volume * 100) + "%" : ""
                color: root.sinkAudio && root.sinkAudio.muted ? Theme.textMuted : Theme.text
            }
            }
            MouseArea {
                anchors.fill: parent
                onWheel: function (w) {
                    if (root.sinkAudio && w.angleDelta.y !== 0)
                        root.sinkAudio.volume = Net.stepVolume(root.sinkAudio.volume, w.angleDelta.y);
                }
            }
        }

        // battery
        RowLayout {
            id: batteryItem
            visible: !root.isIcon && root.battery !== null
            spacing: 4
            IconImage {
                implicitSize: root.iconSize
                Layout.preferredWidth: root.iconSize
                Layout.preferredHeight: root.iconSize
                source: root.battery ? Quickshell.iconPath(root.battery.iconName, true) : ""
            }
            UqText {
                visible: root.wideTier
                text: root.battery ? Net.batteryPct(root.battery.percentage) + "%" : ""
            }
        }

        // Quick settings: opens QuickSettings under this button, on this island's screen.
        Rectangle {
            id: quickButton
            // Read by QuickSettings: its focus grab includes this bar so a second click toggles it shut.
            readonly property var barWindow: root.QsWindow.window
            readonly property bool open: Surfaces.open === "quicksettings" && Surfaces.anchor === quickButton
            Layout.preferredWidth: root.cell
            Layout.preferredHeight: root.cell
            radius: Theme.radius - 2
            color: open ? Theme.accent : (quickMouse.containsMouse ? Theme.overlay : Theme.surfaceAlt)
            Accessible.role: Accessible.Button
            Accessible.name: "Quick settings"
            Accessible.onPressAction: Surfaces.toggle("quicksettings", root.screen, quickButton)
            UqText {
                anchors.centerIn: parent
                text: "⚙"
                color: quickButton.open ? Theme.accentText : Theme.textMuted
            }
            MouseArea {
                id: quickMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: Surfaces.toggle("quicksettings", root.screen, quickButton)
            }
        }
    }
}
