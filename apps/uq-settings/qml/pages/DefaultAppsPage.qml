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

SettingsPage {
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
    readonly property var kindIcons: ({
            "browser": "applications-internet",
            "mail": "internet-mail",
            "files": "system-file-manager",
            "text": "accessories-text-editor",
            "images": "image-x-generic",
            "video": "video-x-generic",
            "music": "audio-x-generic",
            "pdf": "application-pdf"
        })
    /// The kinds on the first card; the rest (pictures, videos, music, PDFs) go on the second.
    readonly property var everyday: ["browser", "mail", "files", "text"]
    /// The combo's entries for one kind, each `{text, icon}`: "Not chosen" when nothing is
    /// set, else the apps.
    function items(kind) {
        const apps = (kind.apps || []).map(a => ({
                    text: String(a.name),
                    icon: String(a.icon || "")
                }));
        const i = (kind.apps || []).findIndex(a => a.id === kind.current);
        if (kind.current && i < 0)
            return apps.concat([{
                    text: String(kind.current) + " (not installed)",
                    icon: "dialog-warning"
                }]);
        if (!kind.current)
            return [{
                    text: "Not chosen",
                    icon: "dialog-cancel"
                }].concat(apps);
        return apps;
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

    DefaultAppsModel {
        id: dm
    }

    banners: [
        RuleMessage {
            objectName: "defaultsError"
            negative: true
            closable: false
            lines: dm.error !== "" ? ["That wasn't saved: " + dm.error] : []
            Layout.fillWidth: true
        }
    ]

    // One kind: its icon, its name and the app that opens it.
    Component {
        id: kindDelegate

        SettingRow {
            id: kindRow
            required property var modelData
            settingId: "default-" + modelData.id
            iconName: page.kindIcons[modelData.id] || "application-x-executable"
            label: String(modelData.label)
            help: (modelData.apps || []).length === 0 ? "No installed app says it can open this." : modelData.source === "system" ? "The system's default. Choosing one here makes it yours." : ""
            highlight: App.highlight
            Layout.fillWidth: true

            IconComboBox {
                id: appCombo
                model: page.items(kindRow.modelData)
                currentIndex: page.index(kindRow.modelData)
                fallbackIcon: "application-x-executable"
                enabled: page.idle && (kindRow.modelData.apps || []).length > 0
                Accessible.name: String(kindRow.modelData.label)
                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                onActivated: i => {
                    const app = page.appAt(kindRow.modelData, i);
                    if (app !== "")
                        dm.setDefault(String(kindRow.modelData.id), app);
                    appCombo.currentIndex = Qt.binding(() => page.index(kindRow.modelData));
                }
            }
        }
    }

    SettingsTab {
        title: "Default apps"
        iconName: "preferences-desktop-default-applications"

        SettingsCard {
            title: "Everyday"
            iconName: "applications-internet"

            Repeater {
                model: page.kinds.filter(k => page.everyday.indexOf(k.id) >= 0)
                delegate: kindDelegate
            }
            SettingRow {
                settingId: "default-terminal"
                iconName: "utilities-terminal"
                label: "Terminal"
                help: page.terminal.available === true ? "Used for apps that run in a terminal." : "The terminal can be chosen when uQ's settings service is running."
                highlight: App.highlight
                enabled: page.terminal.available === true
                Layout.fillWidth: true

                IconComboBox {
                    id: terminalCombo
                    model: [{
                            text: "uQ's default",
                            icon: "utilities-terminal"
                        }].concat(page.terminalApps.map(a => ({
                                text: String(a.name),
                                icon: String(a.icon || "")
                            })))
                    currentIndex: page.terminalIndex()
                    fallbackIcon: "utilities-terminal"
                    enabled: page.terminal.available === true && !dm.busy
                    Accessible.name: "Terminal"
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                    onActivated: i => {
                        dm.setTerminal(i === 0 ? "" : String(page.terminalApps[i - 1].id));
                        terminalCombo.currentIndex = Qt.binding(() => page.terminalIndex());
                    }
                }
            }
        }

        SettingsCard {
            title: "Media and documents"
            iconName: "folder-videos"

            Repeater {
                model: page.kinds.filter(k => page.everyday.indexOf(k.id) < 0)
                delegate: kindDelegate
            }
        }
    }
}
