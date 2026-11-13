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

SettingsPage {
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
    function status(d) {
        if (d.working === "pairing")
            return "Pairing…";
        if (d.working === "connecting")
            return "Connecting…";
        return d.connected ? "Connected" : "";
    }
    function hasBattery(d) {
        return d.battery !== null && d.battery !== undefined;
    }
    /// The battery icon nearest the level: battery-000 to battery-100 in steps of 10.
    function batteryIcon(level) {
        const step = Math.max(0, Math.min(10, Math.round(level / 10))) * 10;
        return "battery-" + String(step).padStart(3, "0");
    }

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

    // One device: its picture and name, battery, connect or pair, and (once paired) whether it
    // may connect on its own and Forget.
    component DeviceCard: SettingsCard {
        id: card
        required property var modelData
        required property int index
        readonly property var d: modelData

        title: String(card.d.name)
        iconName: String(card.d.icon || "") === "" || card.d.icon === "bluetooth" ? "network-bluetooth" : String(card.d.icon)
        border.width: card.d.connected === true ? 2 : 1
        border.color: card.d.connected === true ? Kirigami.Theme.highlightColor : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.08)
        Accessible.description: String(card.d.kind)

        actions: [
            Kirigami.Icon {
                visible: page.hasBattery(card.d)
                source: page.hasBattery(card.d) ? page.batteryIcon(card.d.battery) : ""
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
                Accessible.ignored: true
            },
            UqLabel {
                visible: page.hasBattery(card.d)
                text: page.hasBattery(card.d) ? card.d.battery + "%" : ""
                Accessible.name: "Battery " + text
            },
            QQC2.ToolButton {
                icon.name: "edit-delete"
                visible: card.d.paired === true
                enabled: page.idle
                display: QQC2.AbstractButton.IconOnly
                Accessible.name: "Forget " + String(card.d.name)
                QQC2.ToolTip.text: "Forget"
                QQC2.ToolTip.visible: hovered
                onClicked: {
                    forgetDialog.path = String(card.d.path);
                    forgetDialog.name = String(card.d.name);
                    forgetDialog.open();
                }
            }
        ]

        SettingRow {
            settingId: card.d.paired && card.index === 0 ? "bluetooth-devices" : ""
            highlight: App.highlight
            iconName: card.d.connected === true ? "network-connect" : (card.d.working ? "chronometer" : "network-disconnect")
            label: page.status(card.d) !== "" ? page.status(card.d) : (card.d.paired ? "Not connected" : String(card.d.kind))
            Layout.fillWidth: true

            QQC2.BusyIndicator {
                visible: !!card.d.working
                running: visible
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
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
        SwitchRow {
            visible: card.d.paired === true
            iconName: "security-high"
            label: "Connects without asking"
            help: "It can connect to this computer on its own, without a question each time."
            value: card.d.trusted === true
            enabled: page.idle
            Layout.fillWidth: true
            onChosen: on => bm.setTrusted(String(card.d.path), on)
        }
    }

    // The main switch, on the title row.
    headerItems: [
        SwitchRow {
            objectName: "powerSwitch"
            settingId: "bluetooth-power"
            label: page.powered ? "On" : "Off"
            highlight: App.highlight
            visible: page.ready
            value: page.powered
            enabled: page.idle
            Accessible.name: "Bluetooth"
            onChosen: on => bm.setPowered(on)
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
        ErrorCard {
            objectName: "noAdapter"
            visible: page.loaded && page.snap.available === true && page.adapter === null
            title: "No Bluetooth adapter"
            detail: "This computer has none, or it is switched off in the computer's firmware settings."
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "bluetoothError"
            negative: true
            closable: false
            lines: bm.error !== "" ? ["That wasn't done: " + bm.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        },
        // A code to type on the other device.
        Kirigami.AbstractCard {
            objectName: "displayCode"
            visible: page.display !== null
            Layout.fillWidth: true
            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing * 2
                Kirigami.Icon {
                    source: "input-keyboard"
                    implicitWidth: Kirigami.Units.iconSizes.large
                    implicitHeight: Kirigami.Units.iconSizes.large
                    Accessible.ignored: true
                }
                UqLabel {
                    text: "On " + String(page.display ? page.display.device : "") + ", type this code and press Enter:"
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                UqLabel {
                    text: page.display ? String(page.display.code) : ""
                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 2.5
                    font.bold: true
                    font.letterSpacing: Kirigami.Units.smallSpacing
                    Accessible.name: "Code " + (page.display ? String(page.display.code).split("").join(" ") : "")
                }
            }
        }
    ]

    SettingsTab {
        id: mineTab
        title: "My devices"
        iconName: "network-bluetooth"

        Repeater {
            model: page.powered ? page.paired : []
            delegate: DeviceCard {}
        }
        SettingsCard {
            visible: page.ready && (!page.powered || page.paired.length === 0)
            wide: true
            SettingRow {
                settingId: page.powered && page.paired.length > 0 ? "" : "bluetooth-devices"
                highlight: App.highlight
                iconName: "network-bluetooth"
                label: page.powered ? "No devices paired yet." : "Bluetooth is off."
                opacity: 0.8
                Layout.fillWidth: true

                QQC2.Button {
                    text: "Add a device"
                    icon.name: "list-add"
                    visible: page.powered
                    onClicked: page.show(addTab)
                }
                QQC2.Button {
                    text: "Turn on"
                    icon.name: "network-bluetooth"
                    visible: !page.powered
                    enabled: page.idle
                    onClicked: bm.setPowered(true)
                }
            }
        }
    }

    SettingsTab {
        id: addTab
        title: "Add a device"
        iconName: "list-add"

        SettingsCard {
            title: "Look for devices"
            iconName: "edit-find"
            help: "Put the device in pairing mode (see its manual), then choose Pair."

            SettingRow {
                settingId: "bluetooth-pair"
                label: page.adapter && page.adapter.discovering ? "Looking…" : (page.powered ? "Ready" : "Bluetooth is off")
                iconName: "network-bluetooth"
                highlight: App.highlight
                Layout.fillWidth: true

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
                    enabled: page.idle && page.powered
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
        }
        SettingsCard {
            title: "This computer"
            iconName: "computer"

            SwitchRow {
                settingId: "bluetooth-visible"
                iconName: "visibility"
                label: "Visible to other devices"
                help: page.ready && page.adapter.name !== "" ? "Other devices see it as " + String(page.adapter.name) + "." : ""
                highlight: App.highlight
                value: page.ready && page.adapter.discoverable === true
                enabled: page.idle && page.powered
                onChosen: on => bm.setDiscoverable(on)
                Layout.fillWidth: true
            }
        }
        Repeater {
            model: page.powered ? page.nearby : []
            delegate: DeviceCard {}
        }
    }
}
