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
import "bluetooth"

SettingsPage {
    id: page

    // Closed when another page is shown, not kept alive hidden (PageCache): while open it
    // answers pairing questions, and none should wait on a page nobody can see.
    property bool keepAlive: false

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
    QuestionDialog {
        id: questionDialog
        bm: bm
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
            delegate: BluetoothDeviceCard {
                bm: bm
                forgetDialog: forgetDialog
                page: page
            }
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
            delegate: BluetoothDeviceCard {
                bm: bm
                forgetDialog: forgetDialog
                page: page
            }
        }
    }
}
