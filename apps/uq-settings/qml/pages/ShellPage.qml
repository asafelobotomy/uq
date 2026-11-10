// Shell: pinned apps, launcher providers, the bar's islands and clock, the terminal, the
// notification backend, idle timings and privacy. Everything is read from and written to uqd
// through ShellModel; a refused change (an idle stage out of order, say) shows uqd's message and
// the controls go back to the last good value. App names and provider names are other
// packages' text: they only ever reach the screen through UqLabel.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"

QQC2.ScrollView {
    id: page

    readonly property var snap: {
        try {
            return JSON.parse(sm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var settings: snap.settings || {}
    readonly property var shell: snap.shell || {}
    readonly property var islands: shell.islands || {}
    readonly property var apps: snap.apps || []
    readonly property var terminals: snap.terminals || []
    readonly property var knownProviders: snap.providers || []
    readonly property bool ready: App.uqdUp
    readonly property var islandLabels: [
        { id: "system", name: "System (volume, network, battery)" },
        { id: "work", name: "Workspaces" },
        { id: "time", name: "Clock" },
        { id: "notif", name: "Notifications" },
        { id: "status", name: "Status icons" }
    ]
    readonly property var backendIds: ["auto", "notifi", "builtin", "swaync", "mako", "dunst"]
    readonly property var backendLabels: ({
        auto: "Automatic", notifi: "noti-fi", builtin: "uQ's built-in", swaync: "SwayNotificationCenter",
        mako: "mako", dunst: "dunst"
    })

    /// The pinned ids with the installed app's name, or the raw id when it isn't installed.
    readonly property var pinnedRows: (settings.pinned || []).map(id => {
        const app = apps.find(a => a.id === id);
        return { id: id, name: app ? app.name : id, enabled: true };
    })
    /// The providers in the saved order, then the others in the default order, with their state.
    readonly property var providerRows: {
        const saved = shell.providers || [];
        const byId = {};
        for (const p of knownProviders)
            byId[p.id] = p;
        const rows = [];
        const seen = {};
        for (const s of saved) {
            const p = byId[s.id] || { id: s.id, name: s.id, missing: true };
            seen[s.id] = true;
            rows.push({ id: s.id, name: p.name, enabled: s.enabled !== false, missing: p.missing === true });
        }
        for (const p of knownProviders)
            if (!seen[p.id])
                rows.push({ id: p.id, name: p.name, enabled: true, missing: p.missing === true });
        return rows;
    }
    /// The terminal choices: the default, the installed emulators, and the saved one when it is
    /// no longer installed.
    readonly property var terminalIds: {
        const ids = terminals.map(t => t.id);
        if (shell.terminal && ids.indexOf(shell.terminal) < 0)
            ids.push(shell.terminal);
        return ids;
    }
    readonly property var terminalNames: {
        const names = ["Default (kitty)"];
        for (const id of terminalIds) {
            const t = terminals.find(x => x.id === id);
            names.push(t ? t.name : id);
        }
        return names;
    }

    function setSetting(key, value) {
        sm.setSetting(key, JSON.stringify(value));
    }
    function setShell(field, value) {
        sm.setShell(field, JSON.stringify(value));
    }
    function terminalIndex() {
        return shell.terminal ? terminalIds.indexOf(shell.terminal) + 1 : 0;
    }
    function backendIndex() {
        const i = backendIds.indexOf((settings.notif || {}).backend);
        return i < 0 ? 0 : i;
    }

    Component.onCompleted: sm.refresh()
    Accessible.role: Accessible.Pane
    Accessible.name: "Shell"
    contentWidth: availableWidth

    ShellModel {
        id: sm
    }

    Connections {
        target: App
        function onUqdUpChanged() {
            if (App.uqdUp)
                sm.refresh();
        }
    }

    Kirigami.PromptDialog {
        id: confirmClear
        title: "Clear launch history?"
        subtitle: "uQ forgets how often you launched each app, so the launcher stops ranking by it. This can't be undone."
        standardButtons: Kirigami.Dialog.Yes | Kirigami.Dialog.No
        onAccepted: sm.clearLaunchCounts()
    }

    ColumnLayout {
        id: column
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 40)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Shell"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to change these settings."
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "changeError"
            visible: sm.error !== ""
            title: "That change wasn't applied"
            detail: sm.error
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Pinned apps"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SettingRow {
            settingId: "pins"
            label: "Apps pinned to the launcher"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
        }
        UqLabel {
            text: "Nothing is pinned."
            visible: page.pinnedRows.length === 0
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        }
        OrderedToggleList {
            objectName: "pinnedList"
            model: page.pinnedRows
            showSwitch: false
            removable: true
            // Edits are intents applied to uqd's current list, and one at a time.
            enabled: page.ready && !sm.busy
            onMoved: (id, from, to) => sm.pinMove(id, from, to)
            onRemoved: id => sm.pinRemove(id)
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Launcher"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SettingRow {
            settingId: "providers"
            label: "Search providers, in the order their results appear"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
        }
        OrderedToggleList {
            objectName: "providerList"
            model: page.providerRows
            enabled: page.ready && !sm.busy
            onMoved: (id, from, to) => sm.providerMove(id, from, to)
            onToggled: (id, on) => sm.providerToggle(id, on)
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Bar"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SettingRow {
            settingId: "islands"
            label: "Islands shown in the bar"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
        }
        Repeater {
            model: page.islandLabels
            SwitchRow {
                required property var modelData
                label: modelData.name
                enabled: page.ready
                value: page.islands[modelData.id] !== false
                onChosen: v => page.setShell("islands." + modelData.id, v)
                Layout.leftMargin: Kirigami.Units.largeSpacing * 2
                Layout.fillWidth: true
            }
        }
        SwitchRow {
            settingId: "clock-24h"
            label: "24-hour clock"
            highlight: App.highlight
            enabled: page.ready
            value: page.shell.clock_24h !== false
            onChosen: v => page.setShell("clock_24h", v)
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Terminal"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SettingRow {
            settingId: "terminal"
            label: "Terminal for apps that run in one"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            UqComboBox {
                id: terminalCombo
                model: page.terminalNames
                currentIndex: page.terminalIndex()
                enabled: page.ready
                implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                Accessible.name: "Terminal for apps that run in one"
                onActivated: index => {
                    page.setShell("terminal", index === 0 ? null : page.terminalIds[index - 1]);
                    terminalCombo.currentIndex = Qt.binding(() => page.terminalIndex());
                }
            }
        }

        UqLabel {
            text: "Notifications"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SettingRow {
            settingId: "notif-backend"
            label: "Notification service"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            UqComboBox {
                id: backendCombo
                model: page.backendIds.map(id => page.backendLabels[id])
                currentIndex: page.backendIndex()
                enabled: page.ready
                implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                Accessible.name: "Notification service"
                onActivated: index => {
                    page.setSetting("notif.backend", page.backendIds[index]);
                    backendCombo.currentIndex = Qt.binding(() => page.backendIndex());
                }
            }
        }
        SwitchRow {
            settingId: "notif-summary"
            label: "Show a summary of notifications in the bar"
            highlight: App.highlight
            enabled: page.ready
            value: (page.settings.notif || {}).show_summary === true
            onChosen: v => page.setSetting("notif.show_summary", v)
            Layout.fillWidth: true
        }

        IdleSettings {
            idle: page.settings.idle || ({})
            highlight: App.highlight
            busy: sm.busy
            enabled: page.ready
            onChosen: (key, value) => page.setSetting(key, value)
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Privacy"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SwitchRow {
            settingId: "privacy-ip"
            label: "Look up my public IP address online"
            highlight: App.highlight
            enabled: page.ready
            value: (page.settings.privacy || {}).public_ip_lookup !== false
            onChosen: v => page.setSetting("privacy.public_ip_lookup", v)
            Layout.fillWidth: true
        }
        SwitchRow {
            settingId: "privacy-launches"
            label: "Remember which apps I launch"
            highlight: App.highlight
            enabled: page.ready
            value: (page.settings.privacy || {}).record_launches !== false
            onChosen: v => page.setSetting("privacy.record_launches", v)
            Layout.fillWidth: true
        }
        SettingRow {
            settingId: "clear-launch-counts"
            label: "Launch history"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            QQC2.Button {
                objectName: "clearLaunches"
                text: "Clear launch history"
                enabled: page.ready
                onClicked: confirmClear.open()
            }
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
