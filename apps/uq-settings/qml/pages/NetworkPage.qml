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

SettingsPage {
    id: page

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
    Kirigami.Dialog {
        id: joinDialog
        property var network: ({})
        title: "Join " + String(joinDialog.network.ssid || "")
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 24
        onOpened: {
            joinEveryone.checked = false;
            joinPassword.forceActiveFocus();
        }
        onClosed: joinPassword.wipe()
        customFooterActions: [
            Kirigami.Action {
                text: "Connect"
                icon.name: "network-connect"
                enabled: page.idle && joinPassword.text !== ""
                onTriggered: {
                    nm.connectWifi(String(joinDialog.network.ap), String(joinDialog.network.device), joinPassword.text, joinEveryone.checked);
                    joinDialog.close();
                }
            }
        ]
        ColumnLayout {
            UqLabel {
                text: joinDialog.network.security === "old" ? "This network's key" : "This network's password"
            }
            SecretField {
                id: joinPassword
                objectName: "joinPassword"
                Accessible.name: "Network password"
                Layout.fillWidth: true
                onAccepted: if (text !== "")
                    joinDialog.customFooterActions[0].trigger()
            }
            QQC2.CheckBox {
                id: joinEveryone
                text: "Everyone who uses this computer can join it too"
            }
        }
    }

    // Join a work or school (802.1x) network.
    Kirigami.Dialog {
        id: enterpriseDialog
        property var network: ({})
        readonly property var methods: ["peap", "ttls", "tls"]
        readonly property var phase2: ["mschapv2", "pap", "gtc", "mschap", "chap"]
        readonly property string method: methods[eapMethod.currentIndex] || "peap"
        property string caCert: ""
        property string clientCert: ""
        property string privateKey: ""
        property string picking: ""
        title: "Join " + String(enterpriseDialog.network.ssid || "")
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 28
        onOpened: {
            eapIdentity.clear();
            eapAnonymous.clear();
            eapDomain.clear();
            enterpriseDialog.caCert = "";
            enterpriseDialog.clientCert = "";
            enterpriseDialog.privateKey = "";
            eapEveryone.checked = false;
            eapIdentity.forceActiveFocus();
        }
        onClosed: {
            eapPassword.wipe();
            eapKeyPassword.wipe();
        }
        customFooterActions: [
            Kirigami.Action {
                text: "Connect"
                icon.name: "network-connect"
                enabled: page.idle && eapIdentity.text.trim() !== "" && (enterpriseDialog.method === "tls" ? enterpriseDialog.clientCert !== "" && enterpriseDialog.privateKey !== "" : eapPassword.text !== "")
                onTriggered: {
                    nm.connectEnterprise(String(enterpriseDialog.network.ap), String(enterpriseDialog.network.device), JSON.stringify({
                        method: enterpriseDialog.method,
                        identity: eapIdentity.text,
                        anonymous_identity: eapAnonymous.text,
                        password: eapPassword.text,
                        phase2: enterpriseDialog.phase2[eapPhase2.currentIndex] || "mschapv2",
                        ca_cert: enterpriseDialog.caCert,
                        domain: eapDomain.text,
                        client_cert: enterpriseDialog.clientCert,
                        private_key: enterpriseDialog.privateKey,
                        key_password: eapKeyPassword.text
                    }), eapEveryone.checked);
                    enterpriseDialog.close();
                }
            }
        ]
        GridLayout {
            columns: 2
            UqLabel {
                text: "Sign-in method"
            }
            UqComboBox {
                id: eapMethod
                model: ["Password, protected (PEAP)", "Password, tunnelled (TTLS)", "Certificate (TLS)"]
                Accessible.name: "Sign-in method"
                Layout.fillWidth: true
            }
            UqLabel {
                text: "User name"
            }
            QQC2.TextField {
                id: eapIdentity
                Accessible.name: "User name"
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Outer name (optional)"
            }
            QQC2.TextField {
                id: eapAnonymous
                Accessible.name: "Outer name, sent before the protected sign-in"
                Layout.fillWidth: true
            }
            UqLabel {
                visible: enterpriseDialog.method !== "tls"
                text: "Password"
            }
            SecretField {
                id: eapPassword
                visible: enterpriseDialog.method !== "tls"
                Accessible.name: "Password"
                Layout.fillWidth: true
            }
            UqLabel {
                visible: enterpriseDialog.method !== "tls"
                text: "Inner method"
            }
            UqComboBox {
                id: eapPhase2
                visible: enterpriseDialog.method !== "tls"
                model: ["MSCHAPv2", "PAP", "GTC", "MSCHAP", "CHAP"]
                Accessible.name: "Inner sign-in method"
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Network's certificate"
            }
            RowLayout {
                UqLabel {
                    text: enterpriseDialog.caCert === "" ? "The system's certificates" : decodeURIComponent(enterpriseDialog.caCert.replace(/^.*\//, ""))
                    elide: Text.ElideMiddle
                    Layout.fillWidth: true
                }
                QQC2.Button {
                    text: "Choose…"
                    onClicked: {
                        enterpriseDialog.picking = "ca";
                        certDialog.open();
                    }
                }
            }
            UqLabel {
                text: "Server name ends with"
            }
            QQC2.TextField {
                id: eapDomain
                placeholderText: "e.g. radius.example.org"
                Accessible.name: "Only trust a server whose name ends with this"
                Layout.fillWidth: true
            }
            UqLabel {
                visible: enterpriseDialog.method === "tls"
                text: "Your certificate"
            }
            RowLayout {
                visible: enterpriseDialog.method === "tls"
                UqLabel {
                    text: enterpriseDialog.clientCert === "" ? "None chosen" : decodeURIComponent(enterpriseDialog.clientCert.replace(/^.*\//, ""))
                    elide: Text.ElideMiddle
                    Layout.fillWidth: true
                }
                QQC2.Button {
                    text: "Choose…"
                    onClicked: {
                        enterpriseDialog.picking = "client";
                        certDialog.open();
                    }
                }
            }
            UqLabel {
                visible: enterpriseDialog.method === "tls"
                text: "Your private key"
            }
            RowLayout {
                visible: enterpriseDialog.method === "tls"
                UqLabel {
                    text: enterpriseDialog.privateKey === "" ? "None chosen" : decodeURIComponent(enterpriseDialog.privateKey.replace(/^.*\//, ""))
                    elide: Text.ElideMiddle
                    Layout.fillWidth: true
                }
                QQC2.Button {
                    text: "Choose…"
                    onClicked: {
                        enterpriseDialog.picking = "key";
                        certDialog.open();
                    }
                }
            }
            UqLabel {
                visible: enterpriseDialog.method === "tls"
                text: "Key password"
            }
            SecretField {
                id: eapKeyPassword
                visible: enterpriseDialog.method === "tls"
                Accessible.name: "Private key password"
                Layout.fillWidth: true
            }
            Item {
                implicitWidth: 1
            }
            QQC2.CheckBox {
                id: eapEveryone
                text: "Everyone who uses this computer can join it too"
            }
        }
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
    Kirigami.Dialog {
        id: hiddenDialog
        title: "Join a hidden network"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 24
        onOpened: {
            hiddenName.clear();
            hiddenSecurity.currentIndex = 1;
            hiddenEveryone.checked = false;
            hiddenName.forceActiveFocus();
        }
        onClosed: hiddenPassword.wipe()
        customFooterActions: [
            Kirigami.Action {
                text: "Connect"
                icon.name: "network-connect"
                enabled: page.idle && hiddenName.text.trim() !== "" && (hiddenSecurity.currentIndex === 0 || hiddenPassword.text !== "")
                onTriggered: {
                    nm.connectHidden(hiddenName.text, hiddenSecurity.currentIndex === 0 ? "open" : "password", hiddenPassword.text, hiddenEveryone.checked);
                    hiddenDialog.close();
                }
            }
        ]
        GridLayout {
            columns: 2
            UqLabel {
                text: "Network name"
            }
            QQC2.TextField {
                id: hiddenName
                maximumLength: 32
                Accessible.name: "Network name"
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Protection"
            }
            UqComboBox {
                id: hiddenSecurity
                model: ["None", "Password"]
                Accessible.name: "Protection"
                Layout.fillWidth: true
            }
            UqLabel {
                visible: hiddenSecurity.currentIndex === 1
                text: "Password"
            }
            SecretField {
                id: hiddenPassword
                visible: hiddenSecurity.currentIndex === 1
                Accessible.name: "Network password"
                Layout.fillWidth: true
            }
            Item {
                implicitWidth: 1
            }
            QQC2.CheckBox {
                id: hiddenEveryone
                text: "Everyone who uses this computer can join it too"
            }
        }
    }

    // NetworkManager needs a password for a connection started here.
    Kirigami.Dialog {
        id: secretDialog
        property int askId: -1
        property var question: ({})
        property bool answered: false

        function ask(q) {
            secretDialog.question = q;
            secretDialog.askId = q.id;
            secretDialog.answered = false;
            secretDialog.open();
        }
        function send() {
            const values = {};
            for (let i = 0; i < secretFields.count; i++) {
                const f = secretFields.itemAt(i);
                values[f.key] = f.field.text;
            }
            secretDialog.answered = true;
            nm.answerSecret(secretDialog.askId, JSON.stringify(values));
            secretDialog.close();
        }

        title: "Password needed"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 24
        onClosed: {
            for (let i = 0; i < secretFields.count; i++)
                secretFields.itemAt(i).field.wipe();
            if (!secretDialog.answered && secretDialog.askId >= 0)
                nm.cancelSecret(secretDialog.askId);
        }
        customFooterActions: [
            Kirigami.Action {
                text: "Connect"
                icon.name: "network-connect"
                onTriggered: secretDialog.send()
            }
        ]
        ColumnLayout {
            UqLabel {
                text: (secretDialog.question.retry ? "That didn't work. " : "") + "Type the password for the " + String(secretDialog.question.what || "network") + " " + String(secretDialog.question.name || "") + "."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            Repeater {
                id: secretFields
                model: secretDialog.question.fields || []
                delegate: ColumnLayout {
                    required property var modelData
                    readonly property string key: String(modelData.key)
                    property alias field: secretField
                    Layout.fillWidth: true
                    UqLabel {
                        text: String(modelData.label)
                    }
                    SecretField {
                        id: secretField
                        Accessible.name: String(modelData.label)
                        Layout.fillWidth: true
                        onAccepted: secretDialog.send()
                    }
                }
            }
        }
    }

    // A saved connection's settings.
    Kirigami.Dialog {
        id: detailsDialog
        property string uuid: ""
        readonly property var c: page.saved.find(x => x.uuid === detailsDialog.uuid) || null
        readonly property var methods: ["auto", "manual", "disabled"]
        readonly property var metered: ["auto", "yes", "no"]
        title: detailsDialog.c ? String(detailsDialog.c.name) : ""
        standardButtons: Kirigami.Dialog.Close
        preferredWidth: Kirigami.Units.gridUnit * 30
        onOpened: {
            if (!detailsDialog.c)
                return;
            ipv4Method.currentIndex = Math.max(0, detailsDialog.methods.indexOf(detailsDialog.c.ipv4.method));
            ipv4Addresses.text = String(detailsDialog.c.ipv4.addresses);
            ipv4Gateway.text = String(detailsDialog.c.ipv4.gateway);
            ipv4Dns.text = String(detailsDialog.c.ipv4.dns);
            ipv6Method.currentIndex = Math.max(0, detailsDialog.methods.indexOf(detailsDialog.c.ipv6.method));
            ipv6Addresses.text = String(detailsDialog.c.ipv6.addresses);
            ipv6Gateway.text = String(detailsDialog.c.ipv6.gateway);
            ipv6Dns.text = String(detailsDialog.c.ipv6.dns);
            if (detailsDialog.c.wireguard) {
                wgEndpoint.text = String(detailsDialog.c.wireguard.endpoint);
                wgAllowed.text = String(detailsDialog.c.wireguard.allowed_ips);
                wgKeepalive.value = detailsDialog.c.wireguard.keepalive;
            }
        }
        customFooterActions: [
            Kirigami.Action {
                text: "Forget…"
                icon.name: "edit-delete"
                enabled: page.idle && detailsDialog.c !== null
                onTriggered: {
                    forgetDialog.uuid = detailsDialog.uuid;
                    forgetDialog.name = detailsDialog.c ? String(detailsDialog.c.name) : "";
                    detailsDialog.close();
                    forgetDialog.open();
                }
            }
        ]
        ColumnLayout {
            visible: detailsDialog.c !== null
            spacing: Kirigami.Units.smallSpacing

            UqLabel {
                text: detailsDialog.c ? page.kindText(detailsDialog.c.kind) + (detailsDialog.c.state !== "off" ? " · " + page.stateText(detailsDialog.c.state) : "") : ""
                opacity: 0.7
            }
            QQC2.Switch {
                id: autoSwitch
                text: "Connect automatically"
                checked: detailsDialog.c ? !!detailsDialog.c.autoconnect : false
                enabled: page.idle
                onToggled: {
                    nm.setAutoconnect(detailsDialog.uuid, autoSwitch.checked);
                    autoSwitch.checked = Qt.binding(() => detailsDialog.c ? !!detailsDialog.c.autoconnect : false);
                }
            }
            QQC2.Switch {
                id: everyoneSwitch
                text: "Everyone who uses this computer can use it"
                checked: detailsDialog.c ? !!detailsDialog.c.everyone : false
                enabled: page.idle
                onToggled: {
                    nm.setEveryone(detailsDialog.uuid, everyoneSwitch.checked);
                    everyoneSwitch.checked = Qt.binding(() => detailsDialog.c ? !!detailsDialog.c.everyone : false);
                }
            }
            RowLayout {
                UqLabel {
                    text: "Metered (limit data use)"
                }
                HelpTip {
                    tip: "A metered connection tells apps to hold back big downloads and updates."
                }
                Item {
                    Layout.fillWidth: true
                }
                UqComboBox {
                    id: meteredCombo
                    model: ["Decide automatically", "Yes", "No"]
                    currentIndex: detailsDialog.c ? Math.max(0, detailsDialog.metered.indexOf(detailsDialog.c.metered)) : 0
                    enabled: page.idle
                    Accessible.name: "Metered connection"
                    onActivated: index => {
                        nm.setMetered(detailsDialog.uuid, detailsDialog.metered[index]);
                        meteredCombo.currentIndex = Qt.binding(() => detailsDialog.c ? Math.max(0, detailsDialog.metered.indexOf(detailsDialog.c.metered)) : 0);
                    }
                }
            }

            Kirigami.Separator {
                Layout.fillWidth: true
            }
            RowLayout {
                UqLabel {
                    text: "IPv4 addresses and DNS"
                    font.bold: true
                }
                HelpTip {
                    tip: "DNS servers you list here are used instead of the network's own."
                }
            }
            GridLayout {
                columns: 2
                UqLabel {
                    text: "Addresses"
                }
                UqComboBox {
                    id: ipv4Method
                    model: ["Automatic", "Manual", "Off"]
                    Accessible.name: "IPv4 addresses"
                    Layout.fillWidth: true
                }
                UqLabel {
                    visible: ipv4Method.currentIndex === 1
                    text: "Address/size"
                }
                QQC2.TextField {
                    id: ipv4Addresses
                    visible: ipv4Method.currentIndex === 1
                    placeholderText: "192.168.1.20/24"
                    Accessible.name: "IPv4 addresses, comma-separated"
                    Layout.fillWidth: true
                }
                UqLabel {
                    visible: ipv4Method.currentIndex === 1
                    text: "Gateway"
                }
                QQC2.TextField {
                    id: ipv4Gateway
                    visible: ipv4Method.currentIndex === 1
                    Accessible.name: "IPv4 gateway"
                    Layout.fillWidth: true
                }
                UqLabel {
                    text: "DNS servers"
                }
                QQC2.TextField {
                    id: ipv4Dns
                    placeholderText: "Automatic"
                    Accessible.name: "IPv4 DNS servers, comma-separated"
                    Layout.fillWidth: true
                }
            }
            QQC2.Button {
                text: "Save IPv4 settings"
                icon.name: "document-save"
                enabled: page.idle
                onClicked: nm.setIp(detailsDialog.uuid, 4, detailsDialog.methods[ipv4Method.currentIndex], ipv4Addresses.text, ipv4Gateway.text, ipv4Dns.text)
            }
            UqLabel {
                text: "IPv6 addresses and DNS"
                font.bold: true
            }
            GridLayout {
                columns: 2
                UqLabel {
                    text: "Addresses"
                }
                UqComboBox {
                    id: ipv6Method
                    model: ["Automatic", "Manual", "Off"]
                    Accessible.name: "IPv6 addresses"
                    Layout.fillWidth: true
                }
                UqLabel {
                    visible: ipv6Method.currentIndex === 1
                    text: "Address/size"
                }
                QQC2.TextField {
                    id: ipv6Addresses
                    visible: ipv6Method.currentIndex === 1
                    placeholderText: "fd00::20/64"
                    Accessible.name: "IPv6 addresses, comma-separated"
                    Layout.fillWidth: true
                }
                UqLabel {
                    visible: ipv6Method.currentIndex === 1
                    text: "Gateway"
                }
                QQC2.TextField {
                    id: ipv6Gateway
                    visible: ipv6Method.currentIndex === 1
                    Accessible.name: "IPv6 gateway"
                    Layout.fillWidth: true
                }
                UqLabel {
                    text: "DNS servers"
                }
                QQC2.TextField {
                    id: ipv6Dns
                    placeholderText: "Automatic"
                    Accessible.name: "IPv6 DNS servers, comma-separated"
                    Layout.fillWidth: true
                }
            }
            QQC2.Button {
                text: "Save IPv6 settings"
                icon.name: "document-save"
                enabled: page.idle
                onClicked: nm.setIp(detailsDialog.uuid, 6, detailsDialog.methods[ipv6Method.currentIndex], ipv6Addresses.text, ipv6Gateway.text, ipv6Dns.text)
            }

            ColumnLayout {
                visible: detailsDialog.c !== null && detailsDialog.c.kind === "wireguard"
                Layout.fillWidth: true
                Kirigami.Separator {
                    Layout.fillWidth: true
                }
                UqLabel {
                    text: "VPN server"
                    font.bold: true
                }
                GridLayout {
                    columns: 2
                    UqLabel {
                        text: "Server address"
                    }
                    QQC2.TextField {
                        id: wgEndpoint
                        placeholderText: "vpn.example.com:51820"
                        Accessible.name: "VPN server address and port"
                        Layout.fillWidth: true
                    }
                    UqLabel {
                        text: "Send through the VPN"
                    }
                    QQC2.TextField {
                        id: wgAllowed
                        placeholderText: "0.0.0.0/0, ::/0"
                        Accessible.name: "Address ranges sent through the VPN, comma-separated"
                        Layout.fillWidth: true
                    }
                    UqLabel {
                        text: "Keep alive every (seconds, 0 for off)"
                    }
                    QQC2.SpinBox {
                        id: wgKeepalive
                        from: 0
                        to: 65535
                        Accessible.name: "Keep-alive interval in seconds"
                    }
                }
                QQC2.Button {
                    text: "Save VPN server"
                    icon.name: "document-save"
                    enabled: page.idle
                    onClicked: nm.editWireguard(detailsDialog.uuid, wgEndpoint.text, wgAllowed.text, wgKeepalive.value)
                }
            }
        }
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

    Kirigami.Dialog {
        id: hotspotDialog
        title: "Share this computer's connection"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 24
        onOpened: {
            hotspotName.text = String(page.hotspot.name || "");
            hotspotName.forceActiveFocus();
        }
        onClosed: hotspotPassword.wipe()
        customFooterActions: [
            Kirigami.Action {
                text: "Turn on"
                icon.name: "network-wireless-hotspot"
                enabled: page.idle && (page.hotspot.uuid ? true : hotspotName.text.trim() !== "" && hotspotPassword.text.length >= 8)
                onTriggered: {
                    nm.hotspot(true, hotspotName.text, hotspotPassword.text);
                    hotspotDialog.close();
                }
            }
        ]
        GridLayout {
            columns: 2
            UqLabel {
                text: "Name"
            }
            QQC2.TextField {
                id: hotspotName
                maximumLength: 32
                Accessible.name: "Hotspot name"
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Password"
            }
            SecretField {
                id: hotspotPassword
                Accessible.name: "Hotspot password, at least 8 characters"
                Layout.fillWidth: true
            }
            Item {
                implicitWidth: 1
            }
            UqLabel {
                text: page.hotspot.uuid ? "Leave the password empty to keep the saved one." : "At least 8 characters."
                wrapMode: Text.Wrap
                opacity: 0.7
                Layout.fillWidth: true
            }
        }
    }

    FileDialog {
        id: wireguardFile
        title: "Choose a WireGuard file"
        nameFilters: ["WireGuard files (*.conf)", "All files (*)"]
        onAccepted: nm.importWireguard(String(selectedFile), false)
    }

    // A small icon that names itself in a tooltip on hover.
    component TipIcon: Kirigami.Icon {
        id: tipIcon
        property string tip: ""
        implicitWidth: Kirigami.Units.iconSizes.small
        implicitHeight: Kirigami.Units.iconSizes.small
        Accessible.name: tipIcon.tip
        HoverHandler {
            id: tipHover
        }
        QQC2.ToolTip.visible: tipHover.hovered && tipIcon.tip !== ""
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        QQC2.ToolTip.text: tipIcon.tip
    }
    // A small rounded tag: a band ("5 GHz") or a speed.
    component Chip: Rectangle {
        id: chip
        property string label: ""
        implicitWidth: chipText.implicitWidth + Kirigami.Units.smallSpacing * 2
        implicitHeight: chipText.implicitHeight + 2
        radius: Kirigami.Units.cornerRadius
        color: "transparent"
        border.width: 1
        border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.3)
        UqLabel {
            id: chipText
            anchors.centerIn: parent
            text: chip.label
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            opacity: 0.8
        }
    }
    // A button showing only its icon, named in a tooltip.
    component IconButton: QQC2.ToolButton {
        id: ib
        property string tip: ""
        display: QQC2.AbstractButton.IconOnly
        Accessible.name: ib.tip
        QQC2.ToolTip.text: ib.tip
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
    }

    // One Wi-Fi network: signal, lock, name, band, and what can be done with it.
    component NetworkTile: Rectangle {
        id: tile
        required property var modelData
        readonly property var n: modelData
        readonly property bool on: tile.n.state === "on"

        implicitHeight: tileRow.implicitHeight + Kirigami.Units.smallSpacing * 2
        implicitWidth: tileRow.implicitWidth + Kirigami.Units.largeSpacing * 2
        Layout.fillWidth: true
        radius: Kirigami.Units.cornerRadius
        color: tile.on ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15) : "transparent"
        border.width: tile.on ? 1 : 0
        border.color: Kirigami.Theme.highlightColor
        Accessible.role: Accessible.ListItem
        Accessible.name: String(tile.n.ssid) + ", " + page.securityText(tile.n.security) + ", signal " + tile.n.strength + "%" + (tile.on ? ", connected" : "")

        RowLayout {
            id: tileRow
            anchors.fill: parent
            anchors.leftMargin: Kirigami.Units.largeSpacing
            anchors.rightMargin: Kirigami.Units.smallSpacing
            spacing: Kirigami.Units.smallSpacing

            TipIcon {
                source: ["network-wireless-signal-none", "network-wireless-signal-weak", "network-wireless-signal-ok", "network-wireless-signal-good", "network-wireless-signal-excellent"][tile.n.bars] || "network-wireless"
                tip: "Signal " + tile.n.strength + "%"
                implicitWidth: Kirigami.Units.iconSizes.smallMedium
                implicitHeight: Kirigami.Units.iconSizes.smallMedium
            }
            TipIcon {
                source: page.securityIcon(tile.n.security)
                tip: page.securityText(tile.n.security)
                opacity: tile.n.security === "open" ? 0.5 : 1
            }
            UqLabel {
                text: String(tile.n.ssid)
                font.bold: tile.on
                elide: Text.ElideRight
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.smallSpacing
            }
            UqLabel {
                visible: tile.n.state === "starting" || tile.n.state === "stopping"
                text: page.stateText(tile.n.state)
                opacity: 0.7
            }
            TipIcon {
                visible: !!tile.n.saved
                source: "bookmarks"
                tip: "Saved"
                opacity: 0.7
            }
            Chip {
                visible: String(tile.n.band) !== ""
                label: String(tile.n.band)
            }
            QQC2.Button {
                text: "Connect"
                icon.name: "network-connect"
                visible: tile.n.state === "off"
                enabled: page.idle
                onClicked: page.join(tile.n)
            }
            QQC2.Button {
                text: "Disconnect"
                icon.name: "network-disconnect"
                visible: tile.n.state !== "off" && !!tile.n.saved
                enabled: page.idle
                onClicked: nm.deactivate(String(tile.n.saved))
            }
            IconButton {
                icon.name: "configure"
                tip: "Details"
                enabled: page.idle && !!tile.n.saved
                opacity: tile.n.saved ? 1 : 0
                onClicked: page.details({
                    uuid: tile.n.saved
                })
            }
        }
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

    SettingsTab {
        title: "Wi-Fi"
        iconName: "network-wireless"
        available: page.hasWifi

        SettingsCard {
            title: "Networks"
            iconName: "network-wireless"
            wide: true

            SettingRow {
                visible: page.wifi.enabled !== true || page.networks.length === 0
                iconName: page.wifi.enabled === true ? "network-wireless-acquiring" : "network-wireless-disconnected"
                label: page.wifi.enabled !== true ? "Wi-Fi is off." : (page.wifi.scanned ? "No networks found." : "Looking for networks…")
                opacity: 0.8
                Layout.fillWidth: true

                QQC2.Button {
                    text: "Turn on"
                    icon.name: "network-wireless"
                    visible: page.wifi.enabled !== true
                    enabled: page.idle && page.wifi.hardware !== false
                    onClicked: nm.setWifi(true)
                }
            }
            GridLayout {
                columns: page.width >= Kirigami.Units.gridUnit * 70 ? 2 : 1
                columnSpacing: Kirigami.Units.largeSpacing
                rowSpacing: Kirigami.Units.smallSpacing
                uniformCellWidths: true
                visible: page.wifi.enabled === true
                Layout.fillWidth: true

                Repeater {
                    model: page.wifi.enabled === true ? page.networks : []
                    delegate: NetworkTile {}
                }
            }
            Kirigami.Separator {
                visible: page.wifi.enabled === true
                Layout.fillWidth: true
            }
            SettingRow {
                settingId: "hidden-network"
                iconName: "view-hidden"
                label: "Hidden network"
                help: "A network that doesn't show its name. You need its exact name to join."
                highlight: App.highlight
                visible: page.wifi.enabled === true
                Layout.fillWidth: true

                QQC2.Button {
                    text: "Join…"
                    icon.name: "list-add"
                    enabled: page.idle
                    Accessible.name: "Join a hidden network"
                    onClicked: hiddenDialog.open()
                }
            }
        }
    }

    SettingsTab {
        title: "Wired"
        iconName: "network-wired"
        available: page.ready && page.wired.length > 0

        Repeater {
            model: page.ready ? page.wired : []
            delegate: SettingsCard {
                id: wiredCard
                required property var modelData
                required property int index
                title: String(modelData.name)
                iconName: "network-wired"
                border.width: modelData.connected ? 2 : 1
                border.color: modelData.connected ? Kirigami.Theme.highlightColor : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.08)

                actions: [
                    IconButton {
                        icon.name: "configure"
                        tip: "Details"
                        visible: !!wiredCard.modelData.uuid
                        enabled: page.idle
                        onClicked: page.details({
                            uuid: wiredCard.modelData.uuid
                        })
                    }
                ]

                SettingRow {
                    settingId: wiredCard.index === 0 ? "wired" : ""
                    highlight: App.highlight
                    iconName: wiredCard.modelData.connected ? "network-connect" : "network-disconnect"
                    label: String(wiredCard.modelData.state)
                    Layout.fillWidth: true

                    TipIcon {
                        visible: String(wiredCard.modelData.speed) !== ""
                        source: "speedometer"
                        tip: "Speed"
                    }
                    UqLabel {
                        visible: String(wiredCard.modelData.speed) !== ""
                        text: String(wiredCard.modelData.speed)
                    }
                    QQC2.Button {
                        text: "Disconnect"
                        icon.name: "network-disconnect"
                        visible: !!wiredCard.modelData.uuid && wiredCard.modelData.connected
                        enabled: page.idle
                        onClicked: nm.deactivate(String(wiredCard.modelData.uuid))
                    }
                }
            }
        }
    }

    SettingsTab {
        title: "VPN"
        iconName: "network-vpn"
        available: page.ready

        SettingsCard {
            title: "VPN"
            iconName: "network-vpn"
            help: page.snap.openvpn === false ? "For OpenVPN connections, install the networkmanager-openvpn package." : ""
            wide: true
            actions: [
                QQC2.Button {
                    objectName: "importWireguard"
                    text: "Add a WireGuard file…"
                    icon.name: "document-import"
                    enabled: page.idle
                    onClicked: wireguardFile.open()
                }
            ]

            SettingRow {
                visible: page.vpns.length === 0
                settingId: page.vpns.length === 0 ? "vpn" : ""
                iconName: "network-vpn"
                label: "No VPNs yet."
                help: "Your VPN provider gives you a WireGuard file to add here."
                highlight: App.highlight
                opacity: 0.8
                Layout.fillWidth: true
            }
            Repeater {
                model: page.vpns
                delegate: SwitchRow {
                    id: vpnRow
                    required property var modelData
                    required property int index
                    settingId: index === 0 ? "vpn" : ""
                    highlight: App.highlight
                    iconName: "network-vpn"
                    label: String(modelData.name) + (modelData.state === "starting" ? " · Connecting…" : "")
                    value: modelData.state === "on" || modelData.state === "starting"
                    enabled: page.idle
                    onChosen: on => on ? nm.activate(String(vpnRow.modelData.uuid)) : nm.deactivate(String(vpnRow.modelData.uuid))
                    Layout.fillWidth: true

                    IconButton {
                        icon.name: "configure"
                        tip: "Details"
                        enabled: page.idle
                        onClicked: page.details(vpnRow.modelData)
                    }
                }
            }
        }
    }

    SettingsTab {
        title: "Hotspot"
        iconName: "network-wireless-hotspot"
        available: page.ready && page.hotspot.can === true

        SettingsCard {
            title: "Hotspot"
            iconName: "network-wireless-hotspot"
            help: "While the hotspot is on, this computer's Wi-Fi isn't used to join other networks."
            wide: true

            SwitchRow {
                settingId: "hotspot"
                iconName: "preferences-system-network-sharing"
                label: page.hotspot.state === "on" ? "Sharing as " + String(page.hotspot.name || "") : "Share this computer's connection over Wi-Fi"
                highlight: App.highlight
                value: page.hotspot.state === "on" || page.hotspot.state === "starting"
                enabled: page.idle && page.wifi.enabled === true
                onChosen: on => {
                    if (on)
                        hotspotDialog.open();
                    else
                        nm.hotspot(false, "", "");
                }
                Layout.fillWidth: true
            }
        }
    }

    SettingsTab {
        title: "Saved"
        iconName: "bookmarks"
        available: page.ready

        // The connection in use now: its data use, and the way to its addresses (search for
        // "metered" or "DNS" lands here).
        SettingsCard {
            title: page.current ? String(page.current.name) : "Not connected"
            iconName: page.current ? page.kindIcon(page.current.kind) : "network-disconnect"
            help: "Each saved connection has its own metered and address settings, under Details."
            actions: [
                IconButton {
                    icon.name: "configure"
                    tip: "Details"
                    visible: page.current !== null
                    enabled: page.idle
                    onClicked: page.details(page.current)
                }
            ]

            SettingRow {
                objectName: "meteredRow"
                settingId: "metered"
                iconName: "network-limited"
                label: "Metered"
                help: "Tells apps to hold back big downloads and updates on this connection. Details holds it for every connection."
                highlight: App.highlight
                enabled: page.idle && page.current !== null
                Layout.fillWidth: true

                UqComboBox {
                    id: currentMetered
                    readonly property var values: ["auto", "yes", "no"]
                    readonly property int shown: page.current ? Math.max(0, values.indexOf(page.current.metered)) : 0
                    model: ["Decide automatically", "Yes", "No"]
                    currentIndex: currentMetered.shown
                    Accessible.name: "Metered connection"
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 11
                    onActivated: index => {
                        nm.setMetered(String(page.current.uuid), currentMetered.values[index]);
                        currentMetered.currentIndex = Qt.binding(() => currentMetered.shown);
                    }
                }
            }
            SettingRow {
                objectName: "ipSettingsRow"
                settingId: "ip-settings"
                iconName: "network-server"
                label: "Addresses and DNS"
                help: "Details holds a connection's addresses, gateway and DNS servers."
                highlight: App.highlight
                Layout.fillWidth: true

                UqLabel {
                    visible: page.current !== null
                    text: page.current && page.current.ipv4 ? page.methodText(page.current.ipv4.method) : ""
                    opacity: 0.8
                }
                QQC2.Button {
                    text: "Details…"
                    icon.name: "configure"
                    enabled: page.idle && page.current !== null
                    onClicked: page.details(page.current)
                }
            }
        }

        SettingsCard {
            title: "Saved connections"
            iconName: "bookmarks"
            help: "Passwords you type here go straight to the network service, which keeps them for the connection. Changes may ask for an administrator password."

            SettingRow {
                visible: page.others.length === 0
                settingId: page.others.length === 0 ? "saved-connections" : ""
                highlight: App.highlight
                iconName: "bookmarks"
                label: "None yet."
                opacity: 0.8
                Layout.fillWidth: true
            }
            Repeater {
                model: page.others
                delegate: SettingRow {
                    id: savedRow
                    required property var modelData
                    required property int index
                    readonly property var c: modelData
                    settingId: index === 0 ? "saved-connections" : ""
                    highlight: App.highlight
                    iconName: page.kindIcon(savedRow.c.kind)
                    label: String(savedRow.c.name)
                    Layout.fillWidth: true

                    UqLabel {
                        visible: savedRow.c.state !== "off"
                        text: page.stateText(savedRow.c.state)
                        opacity: 0.7
                    }
                    TipIcon {
                        source: "chronometer"
                        tip: savedRow.c.autoconnect ? "Connects automatically" : "Not automatic"
                        opacity: savedRow.c.autoconnect ? 0.8 : 0.25
                    }
                    TipIcon {
                        source: savedRow.c.everyone ? "system-users" : "user"
                        tip: savedRow.c.everyone ? "Everyone who uses this computer" : "Only you"
                        opacity: 0.8
                    }
                    IconButton {
                        icon.name: "configure"
                        tip: "Details"
                        enabled: page.idle
                        onClicked: page.details(savedRow.c)
                    }
                }
            }
        }
    }
}
