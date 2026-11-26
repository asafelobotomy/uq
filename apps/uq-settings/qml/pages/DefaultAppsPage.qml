// Default apps (spec §2.6). Apps: which app opens web links, mail, calendar invitations,
// folders, text, pictures, videos, music, PDFs, archives and torrents, and the terminal for apps
// that run in one, with uQ's own apps marked and one button to use them all. File types and
// Links: every single type an installed app opens. Only the choices made here are changed in
// your list of default apps; everything else in it stays. App names come from other packages'
// files, so they are shown through UqLabel (plain text) only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"
import "defaultapps"

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
            "calendar": "office-calendar",
            "files": "system-file-manager",
            "text": "accessories-text-editor",
            "images": "image-x-generic",
            "video": "video-x-generic",
            "music": "audio-x-generic",
            "pdf": "application-pdf",
            "archives": "package-x-generic",
            "torrents": "application-x-bittorrent"
        })
    /// The kinds on the first card; the rest go on the second.
    readonly property var everyday: ["browser", "mail", "calendar", "files", "text"]
    readonly property var terminalApps: page.terminal.apps || []
    function terminalIndex() {
        const i = page.terminalApps.findIndex(a => a.id === page.terminal.current);
        return i + 1;
    }

    DefaultAppsModel {
        id: dm
    }

    TypePicker {
        id: picker
        apps: page.snap.apps || ({})
        idle: page.idle
        onChosen: (mime, app) => dm.setType(mime, app)
        onResetRequested: mime => dm.resetType(mime)
    }

    banners: [
        RuleMessage {
            objectName: "defaultsError"
            negative: true
            closable: false
            lines: dm.error !== "" ? ["That wasn't saved: " + dm.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "overrideNote"
            closable: false
            lines: page.snap.override_file ? ["Some choices are set in " + String(page.snap.override_file) + ", which wins over this page. Choosing an app here moves that choice to this page."] : []
            Layout.fillWidth: true
        }
    ]

    Component {
        id: kindDelegate

        KindRow {
            required property var modelData
            kind: modelData
            iconFor: page.kindIcons[modelData.id] || "application-x-executable"
            idle: page.idle
            onChosen: app => dm.setDefault(String(modelData.id), app)
            onResetRequested: dm.resetDefault(String(modelData.id))
        }
    }

    SettingsTab {
        title: "Apps"
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
                help: page.terminal.available === true ? "Opens with Super+Return, for apps that run in a terminal, and for Open Terminal in Dolphin and other apps. uQ's default is kyko when it's installed." : "The terminal can be chosen when uQ's settings service is running."
                highlight: App.highlight
                enabled: page.terminal.available === true
                Layout.fillWidth: true

                IconComboBox {
                    id: terminalCombo
                    model: [{
                            text: "uQ's default",
                            icon: "utilities-terminal"
                        }].concat(page.terminalApps.map(a => ({
                                text: String(a.name) + (a.native === true ? " (uQ's own)" : ""),
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
                QQC2.ToolButton {
                    objectName: "resetTerminal"
                    icon.name: "edit-undo"
                    text: "Use uQ's default"
                    display: QQC2.AbstractButton.IconOnly
                    // Kept in place when hidden, so the boxes line up with the rows above.
                    opacity: page.terminal.current ? 1 : 0
                    enabled: !!page.terminal.current && !dm.busy
                    Accessible.ignored: !page.terminal.current
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: "Use uQ's default"
                    onClicked: dm.setTerminal("")
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

        SettingsCard {
            title: "uQ's own apps"
            iconName: "start-here-symbolic"
            help: "uQ's own apps open what they're installed for when you haven't chosen something else yourself."

            SettingRow {
                settingId: "default-native"
                iconName: "emblem-favorite"
                label: page.snap.native_left === true ? "Some kinds open with other apps" : "uQ's own apps are in use"
                help: "uQview for pictures, reelo for videos and music, and uQbit for torrents, when they're installed."
                highlight: App.highlight
                Layout.fillWidth: true

                QQC2.Button {
                    objectName: "useNative"
                    text: "Use uQ's apps"
                    icon.name: "emblem-checked"
                    enabled: page.idle && page.snap.native_left === true
                    onClicked: dm.useNative()
                }
            }
        }
    }

    TypesTab {
        title: "File types"
        iconName: "document-properties"
        links: false
        types: page.snap.types || []
        apps: page.snap.apps || ({})
        idle: page.idle
        onPick: e => picker.openFor(e)
    }

    TypesTab {
        title: "Links"
        iconName: "link"
        links: true
        types: page.snap.types || []
        apps: page.snap.apps || ({})
        idle: page.idle
        onPick: e => picker.openFor(e)
    }
}
