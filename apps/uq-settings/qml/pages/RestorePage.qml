// Restore: every way back in one place. "uQ settings": reset every setting to uQ's defaults,
// save them to a file and bring a file back (each saves a restore point first), and the newest
// restore points (History has them all). "System" (with Bindle): Rebase, Factory Restore and
// keeping the installer, run by `bindle` with its output shown here. "Snapshots" (with snapper):
// take, delete and roll back to snapshots, and how many automatic ones are kept. What Bindle
// and snapper say reaches the page through UqLabel only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"

SettingsPage {
    id: page

    readonly property var snap: {
        try {
            return JSON.parse(rm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var points: {
        try {
            return JSON.parse(HistoryModel.stateJson).points || [];
        } catch (e) {
            return [];
        }
    }
    readonly property var bindle: page.snap.bindle || ({})
    readonly property var status: page.bindle.status || null
    readonly property var run: page.snap.run || ({})
    readonly property bool running: page.run.state === "running"
    readonly property bool ready: App.uqdUp
    readonly property bool idle: !rm.busy && !page.running
    /// The last notice shown, so each is shown once.
    property int noticeSeen: 0

    function refresh() {
        rm.refresh();
        HistoryModel.refresh();
    }
    function showNews() {
        // Read the JSON itself: this handler can run before `snap` is re-evaluated.
        let notice = null;
        try {
            notice = JSON.parse(rm.stateJson).notice;
        } catch (e) {}
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.lines = notice.text !== "" ? [notice.text] : [];
            noticeMessage.dismissed = false;
            // A reset or a backup saved a restore point: show it in the list.
            HistoryModel.refresh();
        }
    }
    function when(t) {
        const d = new Date(t * 1000);
        return d.toLocaleDateString(Qt.locale(), Locale.ShortFormat) + ", " + Qt.formatTime(d, "hh:mm");
    }

    Component.onCompleted: HistoryModel.refresh()

    RestoreModel {
        id: rm
        onStateJsonChanged: page.showNews()
    }

    Kirigami.PromptDialog {
        id: confirmReset
        objectName: "confirmReset"
        title: "Reset all settings?"
        ColumnLayout {
            UqLabel {
                text: "Every uQ setting goes back to how uQ comes: theme, bar, shortcuts, gestures, window rules, startup apps and the rest. Your files, apps and wallpapers stay, and so do your own lines in user.lua.\n\nuQ saves a restore point of your current settings first, so you can come back: it appears under Restore points, marked “Saved automatically before a reset”."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 26
            }
        }
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Reset all settings"
                icon.name: "edit-reset"
                onTriggered: {
                    rm.resetAll();
                    confirmReset.close();
                }
            }
        ]
    }
    Kirigami.PromptDialog {
        id: confirmRestorePoint

        property string pointId

        title: "Restore these settings?"
        ColumnLayout {
            UqLabel {
                text: "Every setting goes back to how it was when this restore point was saved. uQ saves a restore point of your current settings first, so you can come back."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 26
            }
        }
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: HistoryModel.restore(confirmRestorePoint.pointId)
    }
    Kirigami.PromptDialog {
        id: confirmBackup

        property string url

        title: "Bring back the settings in this file?"
        ColumnLayout {
            UqLabel {
                text: "Every setting becomes what the file holds. uQ saves a restore point of your current settings first, so you can come back."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 26
            }
        }
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: rm.restoreBackup(confirmBackup.url)
    }
    FileDialog {
        id: saveDialog
        title: "Save the settings to a file"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "toml" // jargon-ok: the file extension
        nameFilters: ["uQ settings backups (*.toml)"] // jargon-ok: the file filter shows the file's extension
        onAccepted: rm.saveBackup(String(selectedFile))
    }
    FileDialog {
        id: openDialog
        title: "Bring back settings from a file"
        fileMode: FileDialog.OpenFile
        nameFilters: ["uQ settings backups (*.toml)", "All files (*)"] // jargon-ok: the file filter shows the file's extension
        onAccepted: {
            confirmBackup.url = String(selectedFile);
            confirmBackup.open();
        }
    }

    banners: [
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to reset, save or bring back settings."
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "restoreError"
            negative: true
            closable: false
            lines: rm.error !== "" ? ["That wasn't done: " + rm.error] : HistoryModel.error !== "" ? ["That wasn't done: " + HistoryModel.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            objectName: "noticeMessage"
            Layout.fillWidth: true
        },
        RecoveryRun {
            model: rm
            run: page.run
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "uQ settings"
        iconName: "configure"

        SettingsCard {
            title: "Reset all settings"
            iconName: "edit-reset"
            help: "Puts every uQ setting back to how uQ comes. A restore point is saved first, so it can be undone."

            SettingRow {
                settingId: "reset-all"
                label: "Start again from uQ's defaults"
                highlight: App.highlight
                enabled: page.ready && page.idle
                Layout.fillWidth: true

                QQC2.Button {
                    objectName: "resetAll"
                    text: "Reset all settings…"
                    icon.name: "edit-reset"
                    onClicked: confirmReset.open()
                }
            }
            UqLabel {
                text: "Your files, apps, wallpapers and your own user.lua stay as they are. Only uQ's settings change."
                wrapMode: Text.Wrap
                opacity: 0.7
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }
        }

        SettingsCard {
            title: "Settings backup"
            iconName: "document-save-as"
            help: "A file of every uQ setting, to keep safe or bring to another computer."

            SettingRow {
                settingId: "backup"
                label: "Save the settings to a file"
                highlight: App.highlight
                enabled: page.ready && page.idle
                Layout.fillWidth: true

                QQC2.Button {
                    objectName: "saveBackup"
                    text: "Save…"
                    icon.name: "document-save"
                    onClicked: saveDialog.open()
                }
            }
            SettingRow {
                settingId: "backup-restore"
                label: "Bring back settings from a file"
                highlight: App.highlight
                enabled: page.ready && page.idle
                Layout.fillWidth: true

                QQC2.Button {
                    objectName: "restoreBackup"
                    text: "Open…"
                    icon.name: "document-open"
                    onClicked: openDialog.open()
                }
            }
        }

        SettingsCard {
            title: "Restore points"
            iconName: "document-revert"
            help: "uQ saves one before the first change of each day, and before every reset, restore and profile."

            actions: [
                QQC2.Button {
                    objectName: "openHistory"
                    text: "All restore points"
                    icon.name: "go-next"
                    onClicked: App.showPage("history", "restore-points")
                }
            ]

            UqLabel {
                text: "No restore points yet."
                visible: page.points.length === 0
                opacity: 0.7
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }
            Repeater {
                model: page.points.slice(0, 3)
                delegate: RowLayout {
                    id: pointRow

                    required property var modelData

                    spacing: Kirigami.Units.largeSpacing
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.smallSpacing

                    Kirigami.Icon {
                        source: pointRow.modelData.kind === "manual" ? "user-identity" : "chronometer"
                        implicitWidth: Kirigami.Units.iconSizes.smallMedium
                        implicitHeight: Kirigami.Units.iconSizes.smallMedium
                        Accessible.ignored: true
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        UqLabel {
                            text: String(pointRow.modelData.name)
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        UqLabel {
                            text: page.when(pointRow.modelData.time)
                            opacity: 0.7
                        }
                    }
                    QQC2.Button {
                        text: "Restore"
                        icon.name: "edit-undo"
                        Accessible.name: "Restore " + pointRow.modelData.name
                        enabled: page.ready && !HistoryModel.busy && page.idle
                        onClicked: {
                            confirmRestorePoint.pointId = pointRow.modelData.id;
                            confirmRestorePoint.open();
                        }
                    }
                }
            }
        }
    }

    RestoreSystemTab {
        model: rm
        snap: page.snap
        idle: page.idle
        available: page.bindle.found === true
        showSnapshots: () => snapshotsTab.activate()
    }

    RestoreSnapshotsTab {
        id: snapshotsTab
        model: rm
        snap: page.snap
        idle: page.idle
        available: page.status !== null && page.status.snapper === true
    }
}
