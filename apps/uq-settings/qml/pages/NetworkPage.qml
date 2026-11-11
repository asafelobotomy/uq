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

QQC2.ScrollView {
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

    Accessible.role: Accessible.Pane
    Accessible.name: "Network"
    contentWidth: availableWidth

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
            UqLabel {
                text: "A metered connection tells apps to hold back big downloads and updates."
                wrapMode: Text.Wrap
                opacity: 0.7
                Layout.fillWidth: true
            }

            Kirigami.Separator {
                Layout.fillWidth: true
            }
            UqLabel {
                text: "IPv4 addresses and DNS"
                font.bold: true
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
            UqLabel {
                text: "DNS servers you list here are used instead of the network's own."
                wrapMode: Text.Wrap
                opacity: 0.7
                Layout.fillWidth: true
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

    ColumnLayout {
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 44)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Network"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        ErrorCard {
            objectName: "notAvailable"
            visible: page.loaded && page.snap.available === false
            title: "Not available"
            detail: String(page.snap.text || "")
            Layout.fillWidth: true
        }
        RuleMessage {
            objectName: "networkError"
            negative: true
            closable: false
            lines: nm.error !== "" ? ["That wasn't done: " + nm.error] : []
            Layout.fillWidth: true
        }
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }

        // Wi-Fi.
        SwitchRow {
            settingId: "wifi"
            label: "Wi-Fi"
            highlight: App.highlight
            visible: page.ready && (page.wifi.devices || []).length > 0
            value: page.wifi.enabled === true
            enabled: page.idle && page.wifi.hardware !== false
            onChosen: on => nm.setWifi(on)
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            QQC2.Button {
                text: "Look again"
                icon.name: "view-refresh"
                visible: page.wifi.enabled === true
                enabled: page.idle
                onClicked: nm.scan()
            }
        }
        UqLabel {
            visible: page.ready && page.wifi.hardware === false
            text: "Wi-Fi is turned off by a switch or key on the computer."
            wrapMode: Text.Wrap
            color: Kirigami.Theme.neutralTextColor
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            visible: page.ready && page.wifi.enabled === true && page.networks.length === 0
            text: page.wifi.scanned ? "No networks found." : "Looking for networks…"
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: page.wifi.enabled === true ? page.networks : []
            delegate: Kirigami.AbstractCard {
                id: netCard
                required property var modelData
                readonly property var n: modelData
                Layout.fillWidth: true

                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing

                    Kirigami.Icon {
                        source: ["network-wireless-signal-none", "network-wireless-signal-weak", "network-wireless-signal-ok", "network-wireless-signal-good", "network-wireless-signal-excellent"][netCard.n.bars] || "network-wireless"
                        implicitWidth: Kirigami.Units.iconSizes.medium
                        implicitHeight: Kirigami.Units.iconSizes.medium
                        Accessible.name: "Signal " + netCard.n.strength + "%"
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        UqLabel {
                            text: String(netCard.n.ssid)
                            font.bold: netCard.n.state === "on"
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        UqLabel {
                            text: [page.stateText(netCard.n.state), netCard.n.saved ? "Saved" : "", page.securityText(netCard.n.security), String(netCard.n.band)].filter(t => t !== "").join(" · ")
                            opacity: 0.7
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    QQC2.Button {
                        text: "Connect"
                        icon.name: "network-connect"
                        visible: netCard.n.state === "off"
                        enabled: page.idle
                        onClicked: page.join(netCard.n)
                    }
                    QQC2.Button {
                        text: "Disconnect"
                        icon.name: "network-disconnect"
                        visible: netCard.n.state !== "off" && !!netCard.n.saved
                        enabled: page.idle
                        onClicked: nm.deactivate(String(netCard.n.saved))
                    }
                    QQC2.Button {
                        text: "Details…"
                        icon.name: "configure"
                        visible: !!netCard.n.saved
                        enabled: page.idle
                        onClicked: page.details({
                            uuid: netCard.n.saved
                        })
                    }
                }
            }
        }
        SettingRow {
            settingId: "hidden-network"
            label: "A network that doesn't show its name"
            highlight: App.highlight
            visible: page.ready && page.wifi.enabled === true
            Layout.fillWidth: true

            QQC2.Button {
                text: "Join a hidden network…"
                icon.name: "list-add"
                enabled: page.idle
                onClicked: hiddenDialog.open()
            }
        }

        // Hotspot.
        SwitchRow {
            settingId: "hotspot"
            label: page.hotspot.state === "on" ? "Hotspot: sharing as " + String(page.hotspot.name || "") : "Hotspot: share this computer's connection over Wi-Fi"
            highlight: App.highlight
            visible: page.ready && page.hotspot.can === true
            value: page.hotspot.state === "on" || page.hotspot.state === "starting"
            enabled: page.idle && page.wifi.enabled === true
            onChosen: on => {
                if (on)
                    hotspotDialog.open();
                else
                    nm.hotspot(false, "", "");
            }
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            visible: page.ready && page.hotspot.can === true
            text: "While the hotspot is on, this computer's Wi-Fi isn't used to join other networks."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }

        // Wired.
        Repeater {
            model: page.ready ? (page.snap.wired || []) : []
            delegate: SettingRow {
                id: wiredRow
                required property var modelData
                required property int index
                settingId: index === 0 ? "wired" : ""
                label: "Wired (" + String(modelData.name) + "): " + String(modelData.state) + (modelData.speed !== "" ? ", " + String(modelData.speed) : "")
                highlight: App.highlight
                Layout.fillWidth: true
                Layout.topMargin: index === 0 ? Kirigami.Units.largeSpacing : 0

                QQC2.Button {
                    text: "Disconnect"
                    icon.name: "network-disconnect"
                    visible: !!wiredRow.modelData.uuid && wiredRow.modelData.connected
                    enabled: page.idle
                    onClicked: nm.deactivate(String(wiredRow.modelData.uuid))
                }
                QQC2.Button {
                    text: "Details…"
                    icon.name: "configure"
                    visible: !!wiredRow.modelData.uuid
                    enabled: page.idle
                    onClicked: page.details({
                        uuid: wiredRow.modelData.uuid
                    })
                }
            }
        }

        // VPN.
        SettingRow {
            settingId: "vpn"
            label: "VPN"
            highlight: App.highlight
            visible: page.ready
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            QQC2.Button {
                objectName: "importWireguard"
                text: "Add a WireGuard file…"
                icon.name: "document-import"
                enabled: page.idle
                onClicked: wireguardFile.open()
            }
        }
        UqLabel {
            visible: page.ready && page.vpns.length === 0
            text: "No VPNs yet. Your VPN provider gives you a WireGuard file to add here."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            visible: page.ready && page.snap.openvpn === false
            text: "For OpenVPN connections, install the networkmanager-openvpn package."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: page.vpns
            delegate: SwitchRow {
                id: vpnRow
                required property var modelData
                label: String(modelData.name) + " · " + page.kindText(modelData.kind) + (modelData.state === "starting" ? " · Connecting…" : "")
                value: modelData.state === "on" || modelData.state === "starting"
                enabled: page.idle
                onChosen: on => on ? nm.activate(String(vpnRow.modelData.uuid)) : nm.deactivate(String(vpnRow.modelData.uuid))
                Layout.fillWidth: true

                QQC2.Button {
                    text: "Details…"
                    icon.name: "configure"
                    enabled: page.idle
                    onClicked: page.details(vpnRow.modelData)
                }
            }
        }

        // Saved connections.
        SettingRow {
            settingId: "saved-connections"
            label: "Saved connections"
            highlight: App.highlight
            visible: page.ready
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SettingRow {
            settingId: "ip-settings"
            label: "Addresses, DNS and metered data: choose Details on a connection."
            highlight: App.highlight
            visible: page.ready
            opacity: 0.7
            Layout.fillWidth: true
        }
        Repeater {
            model: page.others
            delegate: Kirigami.AbstractCard {
                id: savedCard
                required property var modelData
                readonly property var c: modelData
                Layout.fillWidth: true

                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        UqLabel {
                            text: String(savedCard.c.name)
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        UqLabel {
                            text: [page.kindText(savedCard.c.kind), page.stateText(savedCard.c.state), savedCard.c.autoconnect ? "" : "Not automatic", savedCard.c.everyone ? "Everyone" : "Only you"].filter(t => t !== "").join(" · ")
                            opacity: 0.7
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    QQC2.Button {
                        text: "Details…"
                        icon.name: "configure"
                        enabled: page.idle
                        onClicked: page.details(savedCard.c)
                    }
                }
            }
        }
        UqLabel {
            visible: page.ready
            text: "Passwords you type here go straight to the network service, which keeps them for the connection. Changes may ask for an administrator password."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
