// Rules' dialog for starting a rule from a template.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

Kirigami.Dialog {
    id: templateDialog
    // Passed in from RulesPage.qml.
    property var page
    property var rm

    property var template: ({})
    property alias app: appBox.text
    readonly property bool needsWorkspace: template.needs === "app_and_workspace"
    readonly property bool complete: appBox.text.trim() !== "" && (!needsWorkspace || workspaceBox.text.trim() !== "")

    function start(t) {
        templateDialog.template = t;
        appBox.clear();
        workspaceBox.clear();
        quietBox.checked = false;
        templateDialog.open();
    }
    function add() {
        if (!page.idle || !templateDialog.complete)
            return;
        rm.fromTemplate(templateDialog.template.id, appBox.text.trim(), templateDialog.needsWorkspace ? workspaceBox.text.trim() : "", templateDialog.needsWorkspace && quietBox.checked);
        templateDialog.close();
    }

    title: "New rule from a template"
    standardButtons: Kirigami.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 26
    onOpened: appBox.forceActiveFocus()
    customFooterActions: [
        Kirigami.Action {
            text: "Add"
            icon.name: "list-add"
            enabled: page.idle && templateDialog.complete
            onTriggered: templateDialog.add()
        }
    ]

    ColumnLayout {
        spacing: Kirigami.Units.smallSpacing

        UqLabel {
            text: String(templateDialog.template.name || "")
            font.bold: true
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            text: "App ID"
        }
        RowLayout {
            QQC2.TextField {
                id: appBox
                maximumLength: 256
                placeholderText: "Such as org.pulseaudio.pavucontrol"
                Accessible.name: "App ID"
                Layout.fillWidth: true
                onAccepted: templateDialog.add()
            }
            QQC2.Button {
                text: "Pick a window"
                icon.name: "edit-select"
                enabled: page.idle
                onClicked: page.startPick("template")
            }
        }
        UqLabel {
            visible: templateDialog.needsWorkspace
            text: "Workspace"
        }
        RowLayout {
            visible: templateDialog.needsWorkspace
            QQC2.TextField {
                id: workspaceBox
                maximumLength: 64
                placeholderText: "A number, or name:web"
                Accessible.name: "Workspace"
                Layout.fillWidth: true
                onAccepted: templateDialog.add()
            }
            QQC2.CheckBox {
                id: quietBox
                text: "Quietly"
                Accessible.description: "Open it there without switching to that workspace"
            }
        }
    }
}
