// Default apps (spec §2.6): which app opens web links, mail, folders, text, pictures, videos,
// music and PDFs, and the terminal for apps that run in one. Only the choices made here are
// changed in your list of default apps; everything else in it stays. App names come from other
// packages' files, so they are shown through UqLabel (plain text) only.
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
            return JSON.parse(dm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var kinds: snap.kinds || []
    readonly property var terminal: snap.terminal || ({})
    readonly property bool idle: page.kinds.length > 0 && !dm.busy

    function refresh() {
        dm.refresh();
    }
    /// The combo's entries for one kind: "Not chosen" when nothing is set, else the apps.
    function labels(kind) {
        const names = (kind.apps || []).map(a => String(a.name));
        const i = (kind.apps || []).findIndex(a => a.id === kind.current);
        if (kind.current && i < 0)
            return names.concat([String(kind.current) + " (not installed)"]);
        if (!kind.current)
            return ["Not chosen"].concat(names);
        return names;
    }
    function index(kind) {
        const apps = kind.apps || [];
        const i = apps.findIndex(a => a.id === kind.current);
        if (!kind.current)
            return 0;
        return i < 0 ? apps.length : i;
    }
    /// The app behind combo entry `i`, or "" for an entry that isn't one.
    function appAt(kind, i) {
        const apps = kind.apps || [];
        const j = kind.current ? i : i - 1;
        return j >= 0 && j < apps.length ? String(apps[j].id) : "";
    }
    readonly property var terminalApps: page.terminal.apps || []
    function terminalIndex() {
        const i = page.terminalApps.findIndex(a => a.id === page.terminal.current);
        return i + 1;
    }

    Accessible.role: Accessible.Pane
    Accessible.name: "Default apps"
    contentWidth: availableWidth

    DefaultAppsModel {
        id: dm
    }

    ColumnLayout {
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 44)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Default apps"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        RuleMessage {
            objectName: "defaultsError"
            negative: true
            closable: false
            lines: dm.error !== "" ? ["That wasn't saved: " + dm.error] : []
            Layout.fillWidth: true
        }

        Repeater {
            model: page.kinds
            delegate: SettingRow {
                id: kindRow
                required property var modelData
                settingId: "default-" + modelData.id
                label: String(modelData.label)
                highlight: App.highlight
                Layout.fillWidth: true

                UqLabel {
                    visible: kindRow.modelData.source === "system"
                    text: "System default"
                    opacity: 0.7
                }
                UqComboBox {
                    id: appCombo
                    model: page.labels(kindRow.modelData)
                    currentIndex: page.index(kindRow.modelData)
                    enabled: page.idle && (kindRow.modelData.apps || []).length > 0
                    implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                    Accessible.name: String(kindRow.modelData.label)
                    onActivated: i => {
                        const app = page.appAt(kindRow.modelData, i);
                        if (app !== "")
                            dm.setDefault(String(kindRow.modelData.id), app);
                        appCombo.currentIndex = Qt.binding(() => page.index(kindRow.modelData));
                    }
                }
            }
        }
        UqLabel {
            visible: page.kinds.some(k => (k.apps || []).length === 0)
            text: "A kind with no choices has no installed app that says it can open it."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }

        SettingRow {
            settingId: "default-terminal"
            label: "Terminal"
            highlight: App.highlight
            enabled: page.terminal.available === true
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            UqComboBox {
                id: terminalCombo
                model: ["uQ's default"].concat(page.terminalApps.map(a => String(a.name)))
                currentIndex: page.terminalIndex()
                enabled: page.terminal.available === true && !dm.busy
                implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                Accessible.name: "Terminal"
                onActivated: i => {
                    dm.setTerminal(i === 0 ? "" : String(page.terminalApps[i - 1].id));
                    terminalCombo.currentIndex = Qt.binding(() => page.terminalIndex());
                }
            }
        }
        UqLabel {
            text: page.terminal.available === true ? "Used for apps that run in a terminal." : "The terminal can be chosen when uQ's settings service is running."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
