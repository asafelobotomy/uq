// Choose the app for one file or link type: every installed app that says it opens it, uQ's own
// marked, and "Use the system's choice". Names are shown through UqLabel only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Kirigami.Dialog {
    id: picker

    property var entry: ({})
    property var apps: ({})
    property bool idle: true

    signal chosen(string mime, string app)
    signal resetRequested(string mime)

    function openFor(e) {
        picker.entry = e;
        picker.open();
    }

    title: "Open with"
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 26
    padding: Kirigami.Units.largeSpacing

    ColumnLayout {
        spacing: Kirigami.Units.smallSpacing

        UqLabel {
            text: String(picker.entry.label || picker.entry.mime || "")
            font.bold: true
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            text: String(picker.entry.mime || "")
            opacity: 0.7
            visible: !!picker.entry.label
            Layout.fillWidth: true
        }
        Kirigami.Separator {
            Layout.fillWidth: true
        }
        Repeater {
            model: picker.entry.apps || []
            delegate: QQC2.ItemDelegate {
                id: choice
                required property string modelData
                readonly property var app: picker.apps[choice.modelData] || ({})
                readonly property bool current: picker.entry.current === choice.modelData
                objectName: "pick_" + choice.modelData
                enabled: picker.idle
                highlighted: choice.current
                Accessible.name: String(choice.app.name || choice.modelData)
                Layout.fillWidth: true
                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing
                    UqAppIcon {
                        name: String(choice.app.icon || "")
                        label: String(choice.app.name || choice.modelData)
                        size: Kirigami.Units.iconSizes.smallMedium
                    }
                    UqLabel {
                        text: String(choice.app.name || choice.modelData)
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    UqLabel {
                        text: "uQ's own"
                        visible: choice.app.native === true
                        opacity: 0.7
                    }
                    Kirigami.Icon {
                        source: "emblem-checked"
                        visible: choice.current
                        implicitWidth: Kirigami.Units.iconSizes.small
                        implicitHeight: Kirigami.Units.iconSizes.small
                        Accessible.ignored: true
                    }
                }
                onClicked: {
                    if (!choice.current)
                        picker.chosen(String(picker.entry.mime), choice.modelData);
                    picker.close();
                }
            }
        }
        QQC2.Button {
            objectName: "pickReset"
            text: "Use the system's choice"
            icon.name: "edit-undo"
            visible: picker.entry.source === "you" || picker.entry.source === "override"
            enabled: picker.idle
            Layout.topMargin: Kirigami.Units.smallSpacing
            onClicked: {
                picker.resetRequested(String(picker.entry.mime));
                picker.close();
            }
        }
    }
}
