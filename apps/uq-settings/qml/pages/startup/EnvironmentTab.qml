// Startup › Environment: the session's environment variables, and the switches of Apps and
// scaling that set variables of their own (read-only here). Variable names and values are shown
// through UqLabel (plain text) only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    id: tab

    property var snap: ({})
    property var model: null
    property bool idle: false
    property bool ready: false
    readonly property var limits: tab.snap.limits || ({})
    readonly property var vars: tab.snap.vars || []

    Kirigami.Dialog {
        id: varDialog

        title: "Add a variable"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 28
        onOpened: newVar.nameField.forceActiveFocus()
        customFooterActions: [
            Kirigami.Action {
                text: "Add"
                icon.name: "list-add"
                enabled: tab.idle && newVar.nameField.text.trim() !== ""
                onTriggered: {
                    tab.model.setVar(-1, newVar.nameField.text.trim(), newVar.valueField.text);
                    varDialog.close();
                }
            }
        ]

        ColumnLayout {
            EnvVarRow {
                id: newVar
                showSave: false
                removable: false
                valueLimit: tab.limits.value_bytes || 4096
                Layout.preferredWidth: Kirigami.Units.gridUnit * 26
            }
            UqLabel {
                text: "Capital letters, digits and _."
                wrapMode: Text.Wrap
                opacity: 0.7
                Layout.fillWidth: true
            }
        }
    }

    SettingsCard {
        title: "Environment variables"
        iconName: "code-variable"
        help: "Changes apply to apps started after the change, and fully after you log out and back in."

        SettingRow {
            settingId: "env-vars"
            iconName: "format-list-unordered"
            label: tab.vars.length === 0 ? "None set here" : tab.vars.length + (tab.vars.length === 1 ? " variable" : " variables")
            highlight: App.highlight
            enabled: tab.ready
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "addVar"
                text: "Add a variable"
                icon.name: "list-add"
                enabled: tab.idle && tab.vars.length < (tab.limits.vars || 100)
                onClicked: {
                    newVar.name = "";
                    newVar.value = "";
                    newVar.reset();
                    varDialog.open();
                }
            }
        }
        Repeater {
            model: tab.vars
            delegate: EnvVarRow {
                required property var modelData
                required property int index
                name: String(modelData.name)
                value: String(modelData.value)
                warn: modelData.warn === true
                origin: modelData.origin || null
                replacedBy: modelData.replaced_by_label ? String(modelData.replaced_by_label) : ""
                idle: tab.idle
                movable: true
                canMoveUp: index > 0
                canMoveDown: index < tab.vars.length - 1
                valueLimit: tab.limits.value_bytes || 4096
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                onSaveRequested: (n, v) => tab.model.setVar(index, n, v)
                onRemoveRequested: tab.model.removeVar(String(modelData.name))
                onMoveRequested: by => tab.model.moveVar(String(modelData.name), index + by)
            }
        }
    }

    // The switches of Apps and scaling, read-only here: they set variables too.
    SettingsCard {
        title: "Set on Apps and scaling"
        iconName: "preferences-desktop-display"
        help: "These switches set variables of their own. Change them on Apps and scaling."
        actions: [
            QQC2.ToolButton {
                objectName: "openAppsScaling"
                text: "Open Apps and scaling"
                icon.name: "go-next"
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: "Open Apps and scaling"
                onClicked: App.showPage("apps-scaling", "")
            }
        ]

        Repeater {
            model: tab.snap.switches || []
            delegate: RowLayout {
                required property var modelData
                spacing: Kirigami.Units.largeSpacing
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                Kirigami.Icon {
                    source: modelData.on === true ? "emblem-checked" : "dialog-cancel"
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: Kirigami.Units.iconSizes.small
                    opacity: modelData.on === true ? 1 : 0.5
                    Accessible.ignored: true
                }
                UqLabel {
                    text: String(modelData.label)
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                UqLabel {
                    objectName: "switchState_" + modelData.key
                    text: modelData.on === true ? "On" : "Off"
                    opacity: 0.7
                }
            }
        }
    }
}
