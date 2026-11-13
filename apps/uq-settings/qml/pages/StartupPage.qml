// Startup (spec §7.3): the apps that start when you log in, and the session's environment
// variables. Apps found in the autostart folders can be turned on or off and delayed; apps and
// commands added here can also be removed. The switches of Apps and scaling are listed
// read-only. Everything goes through StartupModel. App names, commands, the lines a failed app
// printed, and variable names and values are shown through UqLabel (plain text) only.
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
            return JSON.parse(sm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var limits: snap.limits || ({})
    readonly property var apps: snap.apps || []
    readonly property var vars: snap.vars || []
    readonly property var section: snap.section || ({})
    readonly property bool ready: App.uqdUp && snap.error === undefined
    readonly property bool idle: page.ready && !sm.busy
    /// A section's last write failed: its message, or "".
    function sectionMessage(key) {
        const s = page.section[key];
        return s && s.source === "error" ? String(s.message || "") : "";
    }
    readonly property var problems: {
        const out = [];
        if (snap.apps_error)
            out.push("The startup apps couldn't be read: " + String(snap.apps_error));
        const env = page.sectionMessage("env");
        if (env !== "")
            out.push("The last change to the variables wasn't fully applied: " + env);
        const startup = page.sectionMessage("autostart");
        if (startup !== "")
            out.push("The last change to the startup apps wasn't fully applied: " + startup);
        return out;
    }
    /// The last notice shown, so each is shown once.
    property int noticeSeen: 0

    /// Read everything again; Main calls it when the window comes back to the front.
    function refresh() {
        sm.refresh();
    }
    function showNews() {
        const notice = page.snap.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.show(notice.text !== "" ? [notice.text] : []);
        }
    }

    readonly property int addedCount: page.apps.filter(a => a.source === "added").length
    readonly property bool canAdd: page.idle && page.addedCount < (page.limits.added || 50)
    readonly property int onCount: page.apps.filter(a => a.enabled === true).length
    readonly property var waiting: page.apps.filter(a => a.enabled === true && Number(a.delay || 0) > 0)

    Component.onCompleted: sm.refresh()

    StartupModel {
        id: sm
        onStateJsonChanged: page.showNews()
    }
    Connections {
        target: App
        function onUqdUpChanged() {
            if (App.uqdUp)
                sm.refresh();
        }
    }

    AddStartupDialog {
        id: addDialog
        apps: page.snap.installed || []
        idle: page.idle
        onAppChosen: id => sm.addApp(id)
        onCommandChosen: (name, command) => sm.addCommand(name, command)
    }

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
                enabled: page.idle && newVar.nameField.text.trim() !== ""
                onTriggered: {
                    sm.setVar(-1, newVar.nameField.text.trim(), newVar.valueField.text);
                    varDialog.close();
                }
            }
        ]

        ColumnLayout {
            EnvVarRow {
                id: newVar
                showSave: false
                removable: false
                valueLimit: page.limits.value_bytes || 4096
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

    Kirigami.PromptDialog {
        id: confirmRemove

        property string appId

        title: "Remove this app from startup?"
        subtitle: "You can undo this from the history."
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Remove"
                icon.name: "edit-delete"
                enabled: page.idle
                onTriggered: {
                    sm.remove(confirmRemove.appId);
                    confirmRemove.close();
                }
            }
        ]
    }

    banners: [
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to see and change the startup apps."
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "sectionError"
            negative: true
            closable: false
            lines: page.problems
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "startupError"
            negative: true
            closable: false
            lines: sm.error !== "" ? ["That wasn't done: " + sm.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            objectName: "noticeMessage"
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "Apps"
        iconName: "preferences-system-startup"

        SettingsCard {
            title: "Apps that start when you log in"
            iconName: "preferences-system-startup"
            help: "Turning an app on takes effect next time you log in; turning it off doesn't close it now."
            wide: true

            GridLayout {
                columns: page.width > Kirigami.Units.gridUnit * 80 ? 3 : 1
                columnSpacing: Kirigami.Units.largeSpacing
                rowSpacing: 0
                uniformCellWidths: true
                Layout.fillWidth: true

                SettingRow {
                    settingId: "login-apps"
                    iconName: "system-run"
                    label: page.apps.length === 0 ? "None yet" : page.onCount + " of " + page.apps.length + " turned on"
                    highlight: App.highlight
                    Layout.fillWidth: true
                }
                SettingRow {
                    settingId: "start-delay"
                    iconName: "chronometer"
                    label: page.waiting.length === 0 ? "None wait after login" : page.waiting.length + (page.waiting.length === 1 ? " waits" : " wait") + ", up to " + Math.max(...page.waiting.map(a => Number(a.delay))) + " s"
                    help: "An app can wait a few seconds after login, so the desktop is ready first."
                    highlight: App.highlight
                    Layout.fillWidth: true
                }
                SettingRow {
                    settingId: "add-startup"
                    iconName: "list-add"
                    label: "Add"
                    highlight: App.highlight
                    enabled: page.ready
                    Layout.fillWidth: true

                    QQC2.Button {
                        objectName: "addApp"
                        text: "An app"
                        icon.name: "application-x-executable"
                        enabled: page.canAdd
                        Accessible.name: "Add an app"
                        onClicked: addDialog.start("app")
                    }
                    QQC2.Button {
                        objectName: "addCommand"
                        text: "A command"
                        icon.name: "utilities-terminal"
                        enabled: page.canAdd
                        Accessible.name: "Add a command"
                        onClicked: addDialog.start("command")
                    }
                }
            }
            Kirigami.Separator {
                visible: page.apps.length > 0
                Layout.fillWidth: true
            }
            UqLabel {
                visible: page.ready && page.apps.length === 0 && !page.snap.apps_error
                text: "No apps start when you log in."
                opacity: 0.7
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }
            // The apps, two to a line on a wide window.
            GridLayout {
                columns: page.width > Kirigami.Units.gridUnit * 80 ? 2 : 1
                columnSpacing: Kirigami.Units.largeSpacing * 2
                rowSpacing: Kirigami.Units.smallSpacing
                uniformCellWidths: true
                Layout.fillWidth: true

                Repeater {
                    model: page.apps
                    delegate: AutostartRow {
                        required property var modelData
                        app: modelData
                        idle: page.idle
                        delayMax: page.limits.delay_max || 600
                        Layout.alignment: Qt.AlignTop
                        onEnabledChosen: on => sm.setEnabled(String(modelData.id), on)
                        onDelayChosen: secs => sm.setDelay(String(modelData.id), secs)
                        onRemoveRequested: {
                            confirmRemove.appId = String(modelData.id);
                            confirmRemove.open();
                        }
                    }
                }
            }
        }
    }

    SettingsTab {
        title: "Environment"
        iconName: "code-variable"

        SettingsCard {
            title: "Environment variables"
            iconName: "code-variable"
            help: "Changes apply to apps started after the change, and fully after you log out and back in."

            SettingRow {
                settingId: "env-vars"
                iconName: "format-list-unordered"
                label: page.vars.length === 0 ? "None set here" : page.vars.length + (page.vars.length === 1 ? " variable" : " variables")
                highlight: App.highlight
                enabled: page.ready
                Layout.fillWidth: true

                QQC2.Button {
                    objectName: "addVar"
                    text: "Add a variable"
                    icon.name: "list-add"
                    enabled: page.idle && page.vars.length < (page.limits.vars || 100)
                    onClicked: {
                        newVar.name = "";
                        newVar.value = "";
                        newVar.reset();
                        varDialog.open();
                    }
                }
            }
            Repeater {
                model: page.vars
                delegate: EnvVarRow {
                    required property var modelData
                    required property int index
                    name: String(modelData.name)
                    value: String(modelData.value)
                    warn: modelData.warn === true
                    origin: modelData.origin || null
                    replacedBy: modelData.replaced_by_label ? String(modelData.replaced_by_label) : ""
                    idle: page.idle
                    movable: true
                    canMoveUp: index > 0
                    canMoveDown: index < page.vars.length - 1
                    valueLimit: page.limits.value_bytes || 4096
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    onSaveRequested: (n, v) => sm.setVar(index, n, v)
                    onRemoveRequested: sm.removeVar(String(modelData.name))
                    onMoveRequested: by => sm.moveVar(String(modelData.name), index + by)
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
                model: page.snap.switches || []
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
}
