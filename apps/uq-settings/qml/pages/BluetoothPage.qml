// Bluetooth (spec §2.6, §3): turn Bluetooth on or off, let other devices find this computer,
// look for devices, pair, connect, trust and forget them, and see their battery. Pairing works
// only while this page is open: a code to compare or type is shown in plain text and needs an
// explicit Confirm; nothing is accepted on its own. Device names come from the devices
// themselves and are shown through UqLabel (plain text) only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"

QQC2.ScrollView {
    id: page

    readonly property var snap: {
        try {
            return JSON.parse(bm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property bool loaded: snap.notice !== undefined
    readonly property var adapter: snap.adapter || null
    readonly property bool ready: snap.available === true && page.adapter !== null
    readonly property bool powered: page.ready && page.adapter.powered === true
    readonly property bool idle: page.ready && !bm.busy
    readonly property var devices: snap.devices || []
    readonly property var paired: page.devices.filter(d => d.paired)
    readonly property var nearby: page.devices.filter(d => !d.paired)
    readonly property var question: snap.question || null
    readonly property var display: snap.display || null
    property int noticeSeen: 0

    function refresh() {
        bm.refresh();
    }
    function showNews() {
        const notice = page.snap.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.show(notice.text !== "" ? [notice.text] : []);
        }
        if (page.question && page.question.id !== questionDialog.askId) {
            questionDialog.ask(page.question);
        } else if (!page.question && questionDialog.visible) {
            // The device or Bluetooth gave up on the question.
            questionDialog.askId = -1;
            questionDialog.close();
        }
    }
    function subtitle(d) {
        const parts = [String(d.kind)];
        if (d.working === "pairing")
            parts.push("Pairing…");
        else if (d.working === "connecting")
            parts.push("Connecting…");
        else if (d.connected)
            parts.push("Connected");
        if (d.battery !== null && d.battery !== undefined)
            parts.push("Battery " + d.battery + "%");
        return parts.join(" · ");
    }

    Accessible.role: Accessible.Pane
    Accessible.name: "Bluetooth"
    contentWidth: availableWidth

    BluetoothModel {
        id: bm
        onStateJsonChanged: page.showNews()
    }
    // Looking for devices costs power: stop after two minutes.
    Timer {
        id: scanStop
        interval: 120000
        running: page.powered && page.adapter.discovering === true
        onTriggered: bm.scan(false)
    }

    // A pairing question. Nothing is accepted unless Confirm is pressed.
    Kirigami.Dialog {
        id: questionDialog
        property int askId: -1
        property var q: ({})
        property bool answered: false
        readonly property bool typed: q.kind === "pin" || q.kind === "passkey"

        function ask(question) {
            questionDialog.q = question;
            questionDialog.askId = question.id;
            questionDialog.answered = false;
            typedCode.clear();
            questionDialog.open();
            if (questionDialog.typed)
                typedCode.forceActiveFocus();
        }
        function reply(yes) {
            questionDialog.answered = true;
            bm.answer(questionDialog.askId, yes, yes ? typedCode.text : "");
            questionDialog.close();
        }

        title: questionDialog.q.kind === "service" ? "Allow this device?" : "Pair with " + String(questionDialog.q.device || "") + "?"
        standardButtons: QQC2.Dialog.NoButton
        preferredWidth: Kirigami.Units.gridUnit * 24
        closePolicy: QQC2.Popup.CloseOnEscape
        onClosed: {
            if (!questionDialog.answered && questionDialog.askId >= 0)
                bm.answer(questionDialog.askId, false, "");
            typedCode.clear();
        }
        customFooterActions: [
            Kirigami.Action {
                text: "Confirm"
                icon.name: "dialog-ok"
                enabled: !questionDialog.typed || typedCode.text.trim() !== ""
                onTriggered: questionDialog.reply(true)
            },
            Kirigami.Action {
                text: "Reject"
                icon.name: "dialog-cancel"
                onTriggered: questionDialog.reply(false)
            }
        ]
        ColumnLayout {
            UqLabel {
                objectName: "questionText"
                text: {
                    const dev = String(questionDialog.q.device || "the device");
                    switch (questionDialog.q.kind) {
                    case "confirm":
                        return "Check that " + dev + " shows this code, then press Confirm. If it shows a different code, press Reject.";
                    case "authorize":
                        return dev + " wants to pair with this computer. Pair only with devices you know.";
                    case "service":
                        return dev + " wants to " + String(questionDialog.q.service || "use a service") + ".";
                    case "pin":
                        return "Type the PIN for " + dev + ". It's often in the device's manual (for example 0000).";
                    case "passkey":
                        return "Type the 6-digit code " + dev + " shows.";
                    default:
                        return "";
                    }
                }
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            UqLabel {
                objectName: "questionCode"
                visible: questionDialog.q.kind === "confirm"
                text: String(questionDialog.q.code || "")
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 2.5
                font.bold: true
                font.letterSpacing: Kirigami.Units.smallSpacing
                Layout.alignment: Qt.AlignHCenter
                Accessible.name: "Code " + String(questionDialog.q.code || "").split("").join(" ")
            }
            QQC2.TextField {
                id: typedCode
                visible: questionDialog.typed
                maximumLength: questionDialog.q.kind === "passkey" ? 6 : 16
                inputMethodHints: questionDialog.q.kind === "passkey" ? Qt.ImhDigitsOnly : Qt.ImhNone
                Accessible.name: questionDialog.q.kind === "passkey" ? "Code" : "PIN"
                Layout.fillWidth: true
                onAccepted: if (text.trim() !== "")
                    questionDialog.reply(true)
            }
        }
    }

    Kirigami.Dialog {
        id: forgetDialog
        property string path: ""
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
                    bm.forget(forgetDialog.path);
                    forgetDialog.close();
                }
            }
        ]
        UqLabel {
            text: "To use it again you'll need to pair it again."
            wrapMode: Text.Wrap
        }
    }

    component DeviceCard: Kirigami.AbstractCard {
        id: card
        required property var modelData
        readonly property var d: modelData
        Layout.fillWidth: true

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing
            RowLayout {
                spacing: Kirigami.Units.largeSpacing
                Layout.fillWidth: true

                Kirigami.Icon {
                    source: String(card.d.icon)
                    fallback: "bluetooth"
                    implicitWidth: Kirigami.Units.iconSizes.medium
                    implicitHeight: Kirigami.Units.iconSizes.medium
                    Accessible.ignored: true
                }
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    UqLabel {
                        text: String(card.d.name)
                        font.bold: card.d.connected === true
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    UqLabel {
                        text: page.subtitle(card.d)
                        opacity: 0.7
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }
                QQC2.Button {
                    text: "Pair"
                    icon.name: "network-connect"
                    visible: !card.d.paired && !card.d.working
                    enabled: page.idle && page.powered
                    onClicked: bm.pair(String(card.d.path))
                }
                QQC2.Button {
                    text: "Stop"
                    icon.name: "process-stop"
                    visible: card.d.working === "pairing"
                    onClicked: bm.cancelPairing(String(card.d.path))
                }
                QQC2.Button {
                    text: "Connect"
                    icon.name: "network-connect"
                    visible: card.d.paired && !card.d.connected && !card.d.working
                    enabled: page.idle && page.powered
                    onClicked: bm.connectDevice(String(card.d.path))
                }
                QQC2.Button {
                    text: "Disconnect"
                    icon.name: "network-disconnect"
                    visible: card.d.paired && card.d.connected === true
                    enabled: page.idle
                    onClicked: bm.disconnectDevice(String(card.d.path))
                }
            }
            RowLayout {
                visible: card.d.paired === true
                Layout.fillWidth: true
                QQC2.Switch {
                    id: trustSwitch
                    text: "Let it connect without asking"
                    checked: card.d.trusted === true
                    enabled: page.idle
                    onToggled: {
                        bm.setTrusted(String(card.d.path), trustSwitch.checked);
                        trustSwitch.checked = Qt.binding(() => card.d.trusted === true);
                    }
                }
                Item {
                    Layout.fillWidth: true
                }
                QQC2.Button {
                    text: "Forget…"
                    icon.name: "edit-delete"
                    enabled: page.idle
                    onClicked: {
                        forgetDialog.path = String(card.d.path);
                        forgetDialog.name = String(card.d.name);
                        forgetDialog.open();
                    }
                }
            }
        }
    }

    ColumnLayout {
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 44)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Bluetooth"
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
        UqLabel {
            objectName: "noAdapter"
            visible: page.loaded && page.snap.available === true && page.adapter === null
            text: "This computer has no Bluetooth adapter, or it is switched off in the computer's firmware settings."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        RuleMessage {
            objectName: "bluetoothError"
            negative: true
            closable: false
            lines: bm.error !== "" ? ["That wasn't done: " + bm.error] : []
            Layout.fillWidth: true
        }
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }
        // A code to type on the other device.
        Kirigami.AbstractCard {
            objectName: "displayCode"
            visible: page.display !== null
            Layout.fillWidth: true
            contentItem: ColumnLayout {
                UqLabel {
                    text: "On " + String(page.display ? page.display.device : "") + ", type this code and press Enter:"
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                UqLabel {
                    text: page.display ? String(page.display.code) : ""
                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 2.5
                    font.bold: true
                    Layout.alignment: Qt.AlignHCenter
                    Accessible.name: "Code " + (page.display ? String(page.display.code).split("").join(" ") : "")
                }
            }
        }

        SwitchRow {
            settingId: "bluetooth-power"
            label: "Bluetooth"
            highlight: App.highlight
            visible: page.ready
            value: page.powered
            enabled: page.idle
            onChosen: on => bm.setPowered(on)
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SwitchRow {
            settingId: "bluetooth-visible"
            label: "Let other devices find this computer" + (page.ready && page.adapter.name !== "" ? " (as " + String(page.adapter.name) + ")" : "")
            highlight: App.highlight
            visible: page.powered
            value: page.ready && page.adapter.discoverable === true
            enabled: page.idle
            onChosen: on => bm.setDiscoverable(on)
            Layout.fillWidth: true
        }

        SettingRow {
            settingId: "bluetooth-devices"
            label: page.paired.length > 0 ? "Your devices" : "Your devices: none paired yet"
            highlight: App.highlight
            visible: page.powered
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: page.powered ? page.paired : []
            delegate: DeviceCard {}
        }

        SettingRow {
            settingId: "bluetooth-pair"
            label: page.adapter && page.adapter.discovering ? "Looking for devices…" : "Pair a new device"
            highlight: App.highlight
            visible: page.powered
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            QQC2.BusyIndicator {
                visible: page.adapter !== null && page.adapter.discovering === true
                running: visible
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
            }
            QQC2.Button {
                objectName: "lookButton"
                text: "Look for devices"
                icon.name: "view-refresh"
                visible: !(page.adapter && page.adapter.discovering)
                enabled: page.idle
                onClicked: bm.scan(true)
            }
            QQC2.Button {
                text: "Stop looking"
                icon.name: "process-stop"
                visible: !!(page.adapter && page.adapter.discovering)
                enabled: page.idle
                onClicked: bm.scan(false)
            }
        }
        UqLabel {
            visible: page.powered
            text: "Put the device in pairing mode (see its manual), then choose Pair."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: page.powered ? page.nearby : []
            delegate: DeviceCard {}
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
