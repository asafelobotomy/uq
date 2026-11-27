// Network (spec §2.6, §3): Wi-Fi, wired, VPN, a hotspot and saved connections. Everything goes
// through NetworkManager, which asks for an administrator password itself when a change needs
// one; a cancelled prompt leaves things as they were. Passwords typed here go straight to
// NetworkManager: uQ never keeps them. While the page is open it can be asked for a password for
// a connection started from it (a wrong or missing one), and only then. New connections are only
// for you unless you tick "everyone". Network and device names come from the air and from the
// system and are shown through UqLabel (plain text) only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"
import "network"

SettingsPage {
    id: page

    // Closed when another page is shown, not kept alive hidden (PageCache): while open it
    // answers password questions, and none should wait on a page nobody can see.
    property bool keepAlive: false

    readonly property var snap: {
        try {
            return JSON.parse(nm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property bool loaded: snap.notice !== undefined
    readonly property bool ready: snap.available === true
    readonly property bool idle: page.ready && !nm.busy
    readonly property var wifi: snap.wifi || ({})
    readonly property var networks: wifi.networks || []
    readonly property var saved: snap.saved || []
    readonly property var vpns: page.saved.filter(c => c.kind === "vpn" || c.kind === "wireguard")
    readonly property var others: page.saved.filter(c => c.kind !== "vpn" && c.kind !== "wireguard" && c.kind !== "hotspot")
    readonly property var hotspot: snap.hotspot || ({})
    readonly property var secret: snap.secret || null
    readonly property var wired: snap.wired || []
    readonly property bool hasWifi: page.ready && (page.wifi.devices || []).length > 0
    /// The connection in use now (not a VPN or the hotspot), for the Saved tab's quick settings.
    readonly property var current: page.others.find(c => c.state === "on" && (c.kind === "wifi" || c.kind === "wired")) || page.others.find(c => c.state === "on") || null
    property int noticeSeen: 0

    function refresh() {
        nm.refresh();
    }
    function showNews() {
        const notice = page.snap.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.show(notice.text !== "" ? [notice.text] : []);
        }
        if (page.secret && page.secret.id !== secretDialog.askId) {
            secretDialog.ask(page.secret);
        } else if (!page.secret && secretDialog.visible) {
            // NetworkManager stopped asking (it gave up or got its answer elsewhere).
            secretDialog.askId = -1;
            secretDialog.close();
        }
    }
    function stateText(state) {
        switch (state) {
        case "on":
            return "Connected";
        case "starting":
            return "Connecting…";
        case "stopping":
            return "Disconnecting…";
        default:
            return "";
        }
    }
    function securityText(security) {
        switch (security) {
        case "open":
            return "Open";
        case "encrypted":
            return "Open, encrypted";
        case "old":
            return "Old, weak protection";
        case "password":
            return "Password";
        case "enterprise":
            return "Work or school sign-in";
        default:
            return "";
        }
    }
    function kindText(kind) {
        switch (kind) {
        case "wifi":
            return "Wi-Fi";
        case "wired":
            return "Wired";
        case "wireguard":
            return "WireGuard VPN";
        case "vpn":
            return "VPN";
        case "hotspot":
            return "Hotspot";
        default:
            return "Other";
        }
    }
    function kindIcon(kind) {
        switch (kind) {
        case "wifi":
            return "network-wireless";
        case "wired":
            return "network-wired";
        case "wireguard":
        case "vpn":
            return "network-vpn";
        case "hotspot":
            return "network-wireless-hotspot";
        default:
            return "network-connect";
        }
    }
    function securityIcon(security) {
        switch (security) {
        case "open":
            return "object-unlocked";
        case "encrypted":
            return "security-medium";
        case "old":
            return "security-low";
        case "enterprise":
            return "im-user";
        default:
            return "object-locked";
        }
    }
    function methodText(method) {
        switch (method) {
        case "manual":
            return "Set by hand";
        case "disabled":
            return "Off";
        default:
            return "Automatic";
        }
    }
    function join(n) {
        if (n.saved) {
            nm.activate(String(n.saved));
        } else if (n.security === "enterprise") {
            enterpriseDialog.network = n;
            enterpriseDialog.open();
        } else if (n.security === "open" || n.security === "encrypted") {
            nm.connectWifi(String(n.ap), String(n.device), "", false);
        } else {
            joinDialog.network = n;
            joinDialog.open();
        }
    }
    function details(c) {
        detailsDialog.uuid = String(c.uuid);
        detailsDialog.open();
    }


    NetworkModel {
        id: nm
        onStateJsonChanged: page.showNews()
    }
    // Look for networks when the page opens, and now and then while it is open.
    Timer {
        interval: 30000
        repeat: true
        triggeredOnStart: true
        running: page.visible && page.ready && page.wifi.enabled === true
        onTriggered: nm.scan()
    }

    // Join a network with a password.
    JoinDialog {
        id: joinDialog
        nm: nm
        page: page
    }

    // Join a work or school (802.1x) network.
    EnterpriseDialog {
        id: enterpriseDialog
        certDialog: certDialog
        nm: nm
        page: page
    }

    FileDialog {
        id: certDialog
        title: "Choose a certificate file"
        nameFilters: ["Certificates and keys (*.pem *.crt *.cer *.der *.key *.p12 *.pfx)", "All files (*)"]
        onAccepted: {
            const url = String(selectedFile);
            if (enterpriseDialog.picking === "ca")
                enterpriseDialog.caCert = url;
            else if (enterpriseDialog.picking === "client")
                enterpriseDialog.clientCert = url;
            else
                enterpriseDialog.privateKey = url;
        }
    }

    // Join a network that doesn't show its name.
    HiddenDialog {
        id: hiddenDialog
        nm: nm
        page: page
    }

    // NetworkManager needs a password for a connection started here.
    SecretDialog {
        id: secretDialog
        nm: nm
    }

    // A saved connection's settings.
    DetailsDialog {
        id: detailsDialog
        forgetDialog: forgetDialog
        nm: nm
        page: page
    }

    Kirigami.Dialog {
        id: forgetDialog
        property string uuid: ""
        property string name: ""
        title: "Forget " + forgetDialog.name + "?"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 22
        customFooterActions: [
            Kirigami.Action {
                text: "Forget"
                icon.name: "edit-delete"
                enabled: page.idle
                onTriggered: {
                    nm.forget(forgetDialog.uuid);
                    forgetDialog.close();
                }
            }
        ]
        UqLabel {
            text: "Its settings and saved password are removed. You can join it again later."
            wrapMode: Text.Wrap
        }
    }

    HotspotDialog {
        id: hotspotDialog
        nm: nm
        page: page
    }

    FileDialog {
        id: wireguardFile
        title: "Choose a WireGuard file"
        nameFilters: ["WireGuard files (*.conf)", "All files (*)"]
        onAccepted: nm.importWireguard(String(selectedFile), false)
    }

    // The main Wi-Fi switch, on the title row.
    headerItems: [
        IconButton {
            icon.name: "view-refresh"
            tip: "Look for networks again"
            visible: page.hasWifi && page.wifi.enabled === true
            enabled: page.idle
            onClicked: nm.scan()
        },
        SwitchRow {
            objectName: "wifiSwitch"
            settingId: "wifi"
            iconName: "network-wireless"
            label: "Wi-Fi"
            highlight: App.highlight
            visible: page.hasWifi
            value: page.wifi.enabled === true
            enabled: page.idle && page.wifi.hardware !== false
            onChosen: on => nm.setWifi(on)
        }
    ]

    banners: [
        ErrorCard {
            objectName: "notAvailable"
            visible: page.loaded && page.snap.available === false
            title: "Not available"
            detail: String(page.snap.text || "")
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "wifiHardware"
            closable: false
            lines: page.ready && page.wifi.hardware === false ? ["Wi-Fi is turned off by a switch or key on the computer."] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "networkError"
            negative: true
            closable: false
            lines: nm.error !== "" ? ["That wasn't done: " + nm.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }
    ]

    WifiTab {
        hiddenDialog: hiddenDialog
        nm: nm
        page: page
    }

    WiredTab {
        nm: nm
        page: page
    }

    VpnTab {
        nm: nm
        page: page
        wireguardFile: wireguardFile
    }

    HotspotTab {
        hotspotDialog: hotspotDialog
        nm: nm
        page: page
    }

    SavedTab {
        nm: nm
        page: page
    }
}
