// Startup (spec §7.3): the apps that start when you log in, and the session's environment
// variables. Apps found in the autostart folders can be turned on or off, delayed and started
// hidden (in the tray, or minimized); apps and
// commands added here can also be removed. The switches of Apps and scaling are listed
// read-only. Everything goes through StartupModel. App names, commands, the lines a failed app
// printed, and variable names and values are shown through UqLabel (plain text) only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"
import "startup"

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
                    settingId: "start-stagger"
                    iconName: "view-sort-ascending"
                    label: Number(snap.stagger || 0) === 0 ? "Apps start together" : "Apps start " + snap.stagger + " s apart"
                    help: "Starts the apps one after another instead of all at once, so the desktop stays quick while you log in. An app's own delay still counts."
                    highlight: App.highlight
                    enabled: page.ready
                    Layout.fillWidth: true

                    QQC2.SpinBox {
                        id: staggerBox
                        objectName: "staggerBox"
                        from: 0
                        to: page.limits.stagger_max || 30
                        value: Number(snap.stagger || 0)
                        editable: true
                        enabled: page.idle
                        Accessible.name: "Seconds between starts"
                        onValueModified: staggerTimer.restart()
                    }
                    Timer {
                        id: staggerTimer
                        interval: 400
                        onTriggered: sm.setStagger(staggerBox.value)
                    }
                }
                SettingRow {
                    settingId: "start-retry"
                    iconName: "view-refresh"
                    label: snap.retry === true ? "Failed apps try once more" : "Failed apps are left alone"
                    help: "An app that fails when it starts at login, often because the network isn't up yet, is started again once, 10 seconds later."
                    highlight: App.highlight
                    enabled: page.ready
                    Layout.fillWidth: true

                    QQC2.Switch {
                        id: retrySwitch
                        objectName: "retrySwitch"
                        checked: snap.retry === true
                        enabled: page.idle
                        Accessible.name: "Try a failed app once more"
                        onToggled: {
                            sm.setRetry(retrySwitch.checked);
                            retrySwitch.checked = Qt.binding(() => snap.retry === true);
                        }
                    }
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
                        onMinimizeChosen: on => sm.setMinimize(String(modelData.id), on)
                        onRunRequested: sm.runNow(String(modelData.id))
                        onStopRequested: sm.stopNow(String(modelData.id))
                        onRemoveRequested: {
                            confirmRemove.appId = String(modelData.id);
                            confirmRemove.open();
                        }
                    }
                }
            }
        }
    }

    SessionTab {
        title: "Session"
        iconName: "system-log-out"
        snap: page.snap
        model: sm
        idle: page.idle
        ready: page.ready
    }

    ServicesTab {
        title: "Services"
        iconName: "preferences-system-services"
    }

    EnvironmentTab {
        title: "Environment"
        iconName: "code-variable"
        snap: page.snap
        model: sm
        idle: page.idle
        ready: page.ready
    }
}
