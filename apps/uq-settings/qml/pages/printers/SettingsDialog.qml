// Printers' settings dialog: a printer's name, location, driver options and sharing.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Kirigami.Dialog {
    id: settingsDialog
    // Passed in from PrintersPage.qml.
    property var page
    property var pm

    property string printer
    property var chosen: ({})
    property bool more: false
    readonly property var loadedOptions: page.snap.options && page.snap.options.printer === settingsDialog.printer ? page.snap.options.options : null
    readonly property var common: settingsDialog.loadedOptions ? settingsDialog.loadedOptions.filter(o => o.common) : []
    readonly property var others: settingsDialog.loadedOptions ? settingsDialog.loadedOptions.filter(o => !o.common) : []

    function ask(p) {
        settingsDialog.printer = String(p.name);
        settingsDialog.chosen = {};
        settingsDialog.more = false;
        nameField.text = String(p.info);
        locationField.text = String(p.location || "");
        pm.loadOptions(settingsDialog.printer);
        settingsDialog.open();
    }
    function choose(key, value) {
        const c = Object.assign({}, settingsDialog.chosen);
        c[key] = value;
        settingsDialog.chosen = c;
    }

    title: "Printer settings"
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 28
    padding: Kirigami.Units.largeSpacing
    customFooterActions: [
        Kirigami.Action {
            text: "Save"
            icon.name: "document-save"
            enabled: page.idle
            onTriggered: {
                pm.save(settingsDialog.printer, nameField.text, locationField.text, JSON.stringify(settingsDialog.chosen));
                settingsDialog.close();
            }
        }
    ]

    component OptionRow: RowLayout {
        id: optionRow
        required property var modelData
        readonly property var o: modelData
        readonly property string current: settingsDialog.chosen[optionRow.o.key] !== undefined ? settingsDialog.chosen[optionRow.o.key] : String(optionRow.o["default"])
        Layout.fillWidth: true
        UqLabel {
            text: String(optionRow.o.label)
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
        UqComboBox {
            model: optionRow.o.choices.map(c => String(c.label))
            currentIndex: optionRow.o.choices.findIndex(c => c.key === optionRow.current)
            Accessible.name: String(optionRow.o.label)
            Layout.preferredWidth: Kirigami.Units.gridUnit * 12
            onActivated: index => settingsDialog.choose(String(optionRow.o.key), String(optionRow.o.choices[index].key))
        }
    }

    ColumnLayout {
        UqLabel {
            text: "Name"
        }
        QQC2.TextField {
            id: nameField
            objectName: "printerName"
            maximumLength: 127
            Accessible.name: "Name"
            Layout.fillWidth: true
        }
        UqLabel {
            text: "Where it is"
        }
        QQC2.TextField {
            id: locationField
            maximumLength: 127
            placeholderText: "For example: Study"
            Accessible.name: "Where it is"
            Layout.fillWidth: true
        }
        Kirigami.Separator {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Printing defaults"
            font.bold: true
        }
        QQC2.BusyIndicator {
            visible: settingsDialog.loadedOptions === null
            running: visible
            Layout.alignment: Qt.AlignHCenter
        }
        UqLabel {
            visible: settingsDialog.loadedOptions !== null && settingsDialog.loadedOptions.length === 0
            text: "This printer has no options to set here."
            opacity: 0.8
        }
        Repeater {
            model: settingsDialog.common
            delegate: OptionRow {}
        }
        QQC2.Button {
            visible: settingsDialog.others.length > 0
            text: "More options"
            icon.name: settingsDialog.more ? "go-up" : "go-down"
            flat: true
            onClicked: settingsDialog.more = !settingsDialog.more
        }
        Repeater {
            model: settingsDialog.more ? settingsDialog.others : []
            delegate: OptionRow {}
        }
    }
}
