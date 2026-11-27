// Startup › Services: the background services of your account, whether each starts at login,
// whether it is running now, start, stop and restart, and its last log lines. uQ's own services
// are listed but locked. Talks to ServicesModel, never to uqd. Service names, descriptions and
// log lines are shown through UqLabel (plain text) only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    id: tab

    readonly property var snap: {
        try {
            return JSON.parse(services.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var rows: tab.snap.services || []
    readonly property bool idle: !services.busy
    property string filter: ""
    readonly property var shown: {
        const f = tab.filter.trim().toLowerCase();
        if (f === "")
            return tab.rows;
        return tab.rows.filter(r => String(r.name).toLowerCase().includes(f) || String(r.description).toLowerCase().includes(f));
    }
    readonly property int running: tab.rows.filter(r => r.state === "running").length
    readonly property int failed: tab.rows.filter(r => r.state === "failed").length

    ServicesModel {
        id: services
    }

    Kirigami.Dialog {
        id: logDialog

        readonly property var log: tab.snap.log || null
        title: "Recent log"
        standardButtons: Kirigami.Dialog.Close
        preferredWidth: Kirigami.Units.gridUnit * 44
        preferredHeight: Kirigami.Units.gridUnit * 26
        padding: Kirigami.Units.largeSpacing
        onClosed: services.showLog("")
        customFooterActions: [
            Kirigami.Action {
                text: "Refresh"
                icon.name: "view-refresh"
                enabled: tab.idle
                onTriggered: services.refresh()
            }
        ]

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing
            UqLabel {
                text: logDialog.log ? String(logDialog.log.unit) : ""
                font.bold: true
                Layout.fillWidth: true
            }
            UqLabel {
                visible: logDialog.log !== null && (logDialog.log.lines || []).length === 0
                text: logDialog.log && logDialog.log.error ? "The log couldn't be read: " + String(logDialog.log.error) : "Nothing logged yet."
                wrapMode: Text.Wrap
                opacity: 0.7
                Layout.fillWidth: true
            }
            UqLabel {
                objectName: "serviceLog"
                text: logDialog.log ? (logDialog.log.lines || []).join("\n") : ""
                font: Kirigami.Theme.fixedWidthFont
                wrapMode: Text.WrapAnywhere
                Layout.fillWidth: true
            }
        }
    }

    SettingsCard {
        title: "Background services"
        iconName: "preferences-system-services"
        help: "Programs that run without a window, such as a sync or a backup. Turning one on starts it at every login; start and stop act now."
        wide: true

        SettingRow {
            settingId: "background-services"
            iconName: "system-run"
            label: tab.snap.error !== undefined ? "Couldn't read the services" : tab.rows.length + " listed, " + tab.running + " running" + (tab.failed > 0 ? ", " + tab.failed + " failed" : "")
            highlight: App.highlight
            Layout.fillWidth: true

            Kirigami.SearchField {
                objectName: "serviceFilter"
                placeholderText: "Find a service"
                onTextChanged: tab.filter = text
                Layout.preferredWidth: Kirigami.Units.gridUnit * 12
            }
        }
        SettingRow {
            settingId: "services-show-all"
            iconName: "view-list-details"
            label: tab.snap.show_all === true ? "Showing every service" : "Showing the ones you can turn on or off"
            help: "Also list the services other parts of the system start when they need them. Those can be started and stopped, but not turned on for login."
            highlight: App.highlight
            Layout.fillWidth: true

            QQC2.Switch {
                id: allSwitch
                objectName: "showAllServices"
                checked: tab.snap.show_all === true
                enabled: tab.idle
                Accessible.name: "Show every service"
                onToggled: {
                    services.setShowAll(allSwitch.checked);
                    allSwitch.checked = Qt.binding(() => tab.snap.show_all === true);
                }
            }
        }
        RuleMessage {
            negative: true
            closable: false
            lines: {
                const out = [];
                if (tab.snap.error !== undefined)
                    out.push("The services couldn't be read: " + String(tab.snap.error));
                if (services.error !== "")
                    out.push("That wasn't done: " + services.error);
                return out;
            }
            Layout.fillWidth: true
        }
        Kirigami.Separator {
            visible: tab.shown.length > 0
            Layout.fillWidth: true
        }
        UqLabel {
            visible: tab.snap.services !== undefined && tab.shown.length === 0
            text: tab.filter !== "" ? "No service matches." : "No services of your own yet."
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        GridLayout {
            columns: tab.width > Kirigami.Units.gridUnit * 80 ? 2 : 1
            columnSpacing: Kirigami.Units.largeSpacing * 2
            rowSpacing: Kirigami.Units.smallSpacing
            uniformCellWidths: true
            Layout.fillWidth: true

            Repeater {
                model: tab.shown
                delegate: ServiceRow {
                    required property var modelData
                    service: modelData
                    idle: tab.idle
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop
                    onEnabledChosen: on => services.setEnabled(String(modelData.unit), on)
                    onActRequested: action => services.act(String(modelData.unit), action)
                    onLogRequested: {
                        services.showLog(String(modelData.unit));
                        logDialog.open();
                    }
                }
            }
        }
    }
}
