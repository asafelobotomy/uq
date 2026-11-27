// Startup › Session: reopen last session's apps on their workspaces at login, and the commands
// run when you log out. Command names and lines are shown through UqLabel (plain text) only.
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
    readonly property var logout: tab.snap.logout || []
    readonly property int logoutMax: (tab.snap.limits || {}).logout || 20

    Kirigami.Dialog {
        id: logoutDialog

        title: "Run a command at logout"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 28
        padding: Kirigami.Units.largeSpacing
        onOpened: commandField.forceActiveFocus()
        customFooterActions: [
            Kirigami.Action {
                text: "Add"
                icon.name: "list-add"
                enabled: tab.idle && commandField.text.trim() !== ""
                onTriggered: {
                    tab.model.addLogout(nameField.text.trim(), commandField.text.trim());
                    logoutDialog.close();
                }
            }
        ]

        Kirigami.FormLayout {
            QQC2.TextField {
                id: commandField
                objectName: "logoutCommand"
                Kirigami.FormData.label: "Command:"
                placeholderText: "rsync -a ~/Notes /mnt/backup"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 18
            }
            QQC2.TextField {
                id: nameField
                objectName: "logoutName"
                Kirigami.FormData.label: "Name:"
                placeholderText: "Named after the program"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 18
            }
            UqLabel {
                text: "uQ waits up to 10 seconds for these before you're logged out."
                wrapMode: Text.Wrap
                opacity: 0.7
                Layout.preferredWidth: Kirigami.Units.gridUnit * 18
            }
        }
    }

    SettingsCard {
        title: "Restore your session"
        iconName: "document-revert"
        help: "Off by default. Apps that already start at login aren't opened twice."

        SettingRow {
            settingId: "start-restore"
            iconName: "window-duplicate"
            label: tab.snap.restore === true ? "Last session's apps reopen at login" : "Start with an empty desktop"
            help: "uQ remembers the apps you have open and their workspaces, and opens them there next time you log in. Open documents and tabs are up to each app."
            highlight: App.highlight
            enabled: tab.ready
            Layout.fillWidth: true

            QQC2.Switch {
                id: restoreSwitch
                objectName: "restoreSwitch"
                checked: tab.snap.restore === true
                enabled: tab.idle
                Accessible.name: "Reopen last session's apps"
                onToggled: {
                    tab.model.setRestore(restoreSwitch.checked);
                    restoreSwitch.checked = Qt.binding(() => tab.snap.restore === true);
                }
            }
        }
    }

    SettingsCard {
        title: "When you log out"
        iconName: "system-log-out"
        help: "Commands that run as you log out, one after another, such as a backup or a sync."

        SettingRow {
            settingId: "logout-commands"
            iconName: "utilities-terminal"
            label: tab.logout.length === 0 ? "Nothing runs at logout" : tab.logout.length + (tab.logout.length === 1 ? " command" : " commands")
            highlight: App.highlight
            enabled: tab.ready
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "addLogout"
                text: "Add a command"
                icon.name: "list-add"
                enabled: tab.idle && tab.logout.length < tab.logoutMax
                onClicked: {
                    commandField.text = "";
                    nameField.text = "";
                    logoutDialog.open();
                }
            }
        }
        Repeater {
            model: tab.logout
            delegate: RowLayout {
                id: logoutRow
                required property var modelData
                spacing: Kirigami.Units.largeSpacing
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing

                Kirigami.Icon {
                    source: "utilities-terminal"
                    implicitWidth: Kirigami.Units.iconSizes.smallMedium
                    implicitHeight: Kirigami.Units.iconSizes.smallMedium
                    opacity: logoutRow.modelData.enabled ? 1 : 0.5
                    Accessible.ignored: true
                }
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    UqLabel {
                        text: String(logoutRow.modelData.name)
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    UqLabel {
                        text: String(logoutRow.modelData.command_text)
                        font: Kirigami.Theme.fixedWidthFont
                        elide: Text.ElideMiddle
                        opacity: 0.7
                        Layout.fillWidth: true
                    }
                }
                QQC2.Switch {
                    checked: logoutRow.modelData.enabled === true
                    enabled: tab.idle
                    Accessible.name: "Run this command at logout"
                    onToggled: tab.model.setLogoutEnabled(String(logoutRow.modelData.id), checked)
                }
                QQC2.ToolButton {
                    icon.name: "edit-delete"
                    text: "Remove"
                    display: QQC2.AbstractButton.IconOnly
                    enabled: tab.idle
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: "Remove"
                    onClicked: tab.model.removeLogout(String(logoutRow.modelData.id))
                }
            }
        }
    }
}
