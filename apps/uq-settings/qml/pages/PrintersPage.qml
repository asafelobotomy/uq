// Printers: the printers this computer knows, which one is the default, what each is printing
// and what is waiting; pause, resume, print a test page and cancel documents; look for printers
// on the network or plugged in and add them (driverless when the printer can); change a
// printer's name, location and default options; remove it. Printer names, messages and document
// titles come from printers and other people's apps, and are shown through UqLabel (plain text)
// only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"
import "printers"

SettingsPage {
    id: page

    // Shown again (back from another page, or the window came to the front): read again.
    function refresh() {
        pm.refresh();
    }

    readonly property var snap: {
        try {
            return JSON.parse(pm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property bool loaded: snap.notice !== undefined
    readonly property bool ready: snap.available === true
    readonly property bool idle: page.ready && !pm.busy
    readonly property var printers: snap.printers || []
    readonly property var found: snap.found === undefined ? null : snap.found
    readonly property bool looking: snap.looking === true
    property int noticeSeen: 0

    function showNews() {
        const notice = page.snap.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.show(notice.text !== "" ? [notice.text] : []);
        }
    }
    /// What the printer is doing and anything wrong with it, in one line.
    function statusLine(p) {
        const parts = [String(p.status)];
        if (p.accepting === false)
            parts.push("Not taking new documents");
        return parts.concat(p.problems || []).join(" · ");
    }
    function stateIcon(p) {
        if ((p.problems || []).length > 0 || p.state === "stopped")
            return "dialog-warning";
        if (p.state === "paused")
            return "media-playback-pause";
        if (p.state === "printing")
            return "document-print";
        return "checkmark";
    }
    function connectionIcon(c) {
        switch (c) {
        case "USB":
            return "drive-removable-media-usb";
        case "Network":
            return "network-wired";
        case "Saves PDF files":
        case "Saves to a file":
            return "document-save";
        default:
            return "printer";
        }
    }
    /// An address someone typed: a full printer address as it is, else a driverless one.
    function addressUri(text) {
        const t = String(text).trim();
        if (t === "")
            return "";
        return t.indexOf("://") >= 0 ? t : "ipp://" + t + "/ipp/print";
    }

    PrintersModel {
        id: pm
        onStateJsonChanged: page.showNews()
    }

    Kirigami.Dialog {
        id: removeDialog
        property string printer
        property string name
        title: "Remove " + removeDialog.name + "?"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 22
        padding: Kirigami.Units.largeSpacing
        customFooterActions: [
            Kirigami.Action {
                text: "Remove"
                icon.name: "edit-delete"
                enabled: page.idle
                onTriggered: {
                    pm.remove(removeDialog.printer);
                    removeDialog.close();
                }
            }
        ]
        UqLabel {
            text: "Documents waiting for it are cancelled. You can add it again later."
            wrapMode: Text.Wrap
        }
    }

    // Adding a printer: its name and place, and the driver (driverless when the printer can).
    Kirigami.Dialog {
        id: addDialog
        property string uri
        property string deviceId
        property string model
        readonly property var drivers: page.snap.drivers && page.snap.drivers.uri === addDialog.uri ? page.snap.drivers.drivers : null

        function ask(uri, info, model, deviceId, location) {
            addDialog.uri = uri;
            addDialog.model = model;
            addDialog.deviceId = deviceId;
            // The model reads better than "… (driverless)", which is how the search names it.
            addName.text = model !== "" ? model : info;
            addLocation.text = location;
            driverCombo.currentIndex = 0;
            pm.findDrivers(uri, deviceId, model);
            addDialog.open();
            addName.forceActiveFocus();
        }

        title: "Add a printer"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 26
        padding: Kirigami.Units.largeSpacing
        customFooterActions: [
            Kirigami.Action {
                text: "Add"
                icon.name: "list-add"
                enabled: page.idle && addDialog.drivers !== null && addDialog.drivers.length > 0
                onTriggered: {
                    const d = addDialog.drivers[Math.max(0, driverCombo.currentIndex)];
                    pm.add(addDialog.uri, addName.text, addLocation.text, String(d.name));
                    addDialog.close();
                }
            }
        ]
        ColumnLayout {
            UqLabel {
                text: "Name"
            }
            QQC2.TextField {
                id: addName
                objectName: "addName"
                maximumLength: 127
                Accessible.name: "Name"
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Where it is (optional)"
            }
            QQC2.TextField {
                id: addLocation
                maximumLength: 127
                placeholderText: "For example: Study"
                Accessible.name: "Where it is"
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Driver"
            }
            RowLayout {
                Layout.fillWidth: true
                QQC2.BusyIndicator {
                    visible: addDialog.drivers === null
                    running: visible
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: Kirigami.Units.iconSizes.small
                }
                UqComboBox {
                    id: driverCombo
                    objectName: "driverCombo"
                    visible: addDialog.drivers !== null && addDialog.drivers.length > 0
                    model: addDialog.drivers ? addDialog.drivers.map(d => String(d.label)) : []
                    Accessible.name: "Driver"
                    Layout.fillWidth: true
                    Layout.maximumWidth: Kirigami.Units.gridUnit * 24
                }
                UqLabel {
                    visible: addDialog.drivers !== null && addDialog.drivers.length === 0
                    text: "No driver matches this printer. Install its maker's driver package (for example gutenprint or hplip), then try again."
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
            }
        }
    }

    // A printer's name, place and default options.
    SettingsDialog {
        id: settingsDialog
        page: page
        pm: pm
    }

    banners: [
        ErrorCard {
            objectName: "notAvailable"
            visible: page.loaded && page.snap.available === false
            title: page.snap.off === true ? "Printing is switched off" : "Not available"
            detail: String(page.snap.text || "")
            Layout.fillWidth: true
        },
        QQC2.Button {
            objectName: "turnOn"
            visible: page.loaded && page.snap.off === true
            text: "Turn on printing"
            icon.name: "system-run"
            enabled: !pm.busy
            onClicked: pm.turnOn()
        },
        RuleMessage {
            objectName: "printersError"
            negative: true
            closable: false
            lines: pm.error !== "" ? ["That wasn't done: " + pm.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        id: listTab
        title: "Printers"
        iconName: "printer"

        Repeater {
            model: page.printers
            delegate: PrinterCard {
                page: page
                pm: pm
                removeDialog: removeDialog
                settingsDialog: settingsDialog
            }
        }
        SettingsCard {
            visible: page.ready && page.printers.length === 0
            wide: true
            SettingRow {
                settingId: "printers-list"
                highlight: App.highlight
                iconName: "printer"
                label: "No printers yet."
                opacity: 0.8
                Layout.fillWidth: true

                QQC2.Button {
                    text: "Add a printer"
                    icon.name: "list-add"
                    onClicked: page.show(addTab)
                }
            }
        }
    }

    SettingsTab {
        id: addTab
        title: "Add a printer"
        iconName: "list-add"

        SettingsCard {
            title: "Look for printers"
            iconName: "edit-find"
            help: "Turn the printer on and plug it in, or connect it to the same network as this computer."
            wide: true

            SettingRow {
                settingId: "printers-add"
                highlight: App.highlight
                iconName: "printer"
                label: page.looking ? "Looking…" : (page.found === null ? "Printers on your network and USB ports" : (page.found.length === 0 ? "No printers found" : (page.found.length === 1 ? "1 printer found" : page.found.length + " printers found")))
                Layout.fillWidth: true

                QQC2.BusyIndicator {
                    visible: page.looking
                    running: visible
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: Kirigami.Units.iconSizes.small
                }
                QQC2.Button {
                    objectName: "lookButton"
                    text: "Look for printers"
                    icon.name: "view-refresh"
                    enabled: page.idle && !page.looking
                    onClicked: pm.look()
                }
            }
            Repeater {
                model: page.found || []
                delegate: SettingRow {
                    id: foundRow
                    required property var modelData
                    readonly property var d: modelData
                    iconName: page.connectionIcon(String(foundRow.d.connection))
                    label: String(foundRow.d.info)
                    help: [String(foundRow.d.connection), foundRow.d.driverless === true ? "Works without a driver" : "", String(foundRow.d.uri)].filter(s => s !== "").join(" · ")
                    Layout.fillWidth: true

                    UqLabel {
                        visible: foundRow.d.added === true
                        text: "Added"
                        opacity: 0.8
                    }
                    QQC2.Button {
                        visible: foundRow.d.added !== true
                        text: "Add"
                        icon.name: "list-add"
                        enabled: page.idle
                        onClicked: addDialog.ask(String(foundRow.d.uri), String(foundRow.d.info), String(foundRow.d.model), String(foundRow.d.device_id), String(foundRow.d.location || ""))
                    }
                }
            }
        }
        SettingsCard {
            title: "Add by address"
            iconName: "network-wired"
            help: "For a network printer that wasn't found: type its name on the network or its address. Most printers made since 2010 work without a driver."
            wide: true

            SettingRow {
                label: "Printer address"
                iconName: "network-server"
                Layout.fillWidth: true

                QQC2.TextField {
                    id: addressField
                    objectName: "addressField"
                    placeholderText: "printer.local or 192.168.1.20"
                    Accessible.name: "Printer address"
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                    onAccepted: if (text.trim() !== "" && page.idle)
                        addDialog.ask(page.addressUri(text), "", "", "", "")
                }
                QQC2.Button {
                    text: "Add"
                    icon.name: "list-add"
                    enabled: page.idle && addressField.text.trim() !== ""
                    onClicked: addDialog.ask(page.addressUri(addressField.text), "", "", "", "")
                }
            }
        }
        SettingsCard {
            title: "Something else"
            iconName: "help-hint"
            help: "The printing service's own pages handle the rare printer this page can't set up: classes of printers, unusual drivers and sharing."
            wide: true

            SettingRow {
                label: "The printing service's web page"
                iconName: "internet-web-browser"
                Layout.fillWidth: true

                QQC2.Button {
                    text: "Open"
                    icon.name: "document-open"
                    onClicked: Qt.openUrlExternally("http://localhost:631/")
                }
            }
        }
    }
}
