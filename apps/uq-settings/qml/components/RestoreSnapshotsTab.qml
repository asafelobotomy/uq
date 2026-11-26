// Restore › Snapshots: the system's snapper snapshots, through `bindle snapshot`. Take one now,
// see each with its date and what made it, roll back to one (from the next start) or delete it,
// and choose how many automatic snapshots are kept. Reading them may need the password; the
// tab asks with a button rather than on opening. Descriptions are snapper's or pacman's text,
// shown through UqLabel.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "."

SettingsTab {
    id: tab

    /// The page's RestoreModel.
    required property var model
    required property var snap
    required property bool idle
    readonly property var status: (tab.snap.bindle || {}).status || null
    readonly property bool canRollBack: tab.status !== null && !tab.status.rollback
    readonly property var snapshots: tab.snap.snapshots || null
    readonly property var config: tab.snap.snapper || ({})

    function kindText(s) {
        return s.type === "pre" ? "before a change" : s.type === "post" ? "after a change" : String(s.cleanup || "") === "timeline" ? "automatic" : "";
    }
    /// A limit as a number for the spin boxes; "2-10" is its upper end.
    function limit(key) {
        const v = String(tab.config[key] || "0");
        return parseInt(v.split("-").pop(), 10) || 0;
    }

    title: "Snapshots"
    iconName: "camera-photo"

    Kirigami.PromptDialog {
        id: confirmRollback

        property int number: 0

        objectName: "confirmRollback"
        title: "Roll back to snapshot " + confirmRollback.number + "?"
        ColumnLayout {
            UqLabel {
                text: "From the next start, the system is as it was in this snapshot.\n\nStays: your home folder and everything in it.\nGoes: system changes made since the snapshot. The system as it is now is kept aside, to go back to or remove later.\n\nBindle asks for your password, then offers to restart."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 26
            }
        }
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Roll back"
                icon.name: "document-revert"
                onTriggered: {
                    tab.model.recover("rollback", confirmRollback.number);
                    confirmRollback.close();
                }
            }
        ]
    }
    Kirigami.PromptDialog {
        id: confirmDelete

        property int number: 0

        title: "Delete snapshot " + confirmDelete.number + "?"
        ColumnLayout {
            UqLabel {
                text: "The system stays as it is now. This snapshot can't be rolled back to afterwards."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 26
            }
        }
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Delete"
                icon.name: "edit-delete"
                onTriggered: {
                    tab.model.deleteSnapshot(confirmDelete.number);
                    confirmDelete.close();
                }
            }
        ]
    }

    SettingsCard {
        title: "Your snapshots"
        iconName: "camera-photo"
        help: "A snapshot is the whole system as it was at one moment, apart from your home folder."
        visible: tab.snapshots === null

        UqLabel {
            text: tab.snap.snapshotsError ? String(tab.snap.snapshotsError) : tab.snap.needsPassword ? "Seeing the snapshots needs your password." : "Reading the snapshots…"
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        QQC2.Button {
            objectName: "showSnapshots"
            text: "Show snapshots"
            icon.name: "unlock"
            enabled: tab.idle
            onClicked: tab.model.loadSnapshots()
        }
    }

    SettingsCard {
        title: "Take a snapshot"
        iconName: "camera-photo"
        help: "Before a big change, so you can roll back to now."

        SettingRow {
            settingId: "take-snapshot"
            label: "Name"
            highlight: App.highlight
            enabled: tab.idle
            Layout.fillWidth: true

            QQC2.TextField {
                id: snapName
                objectName: "snapshotName"
                maximumLength: 100
                placeholderText: "Such as Before trying a new driver"
                Accessible.name: "Name of the new snapshot"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                onAccepted: takeButton.clicked()
            }
            QQC2.Button {
                id: takeButton
                objectName: "takeSnapshot"
                text: "Take snapshot"
                icon.name: "camera-photo"
                onClicked: {
                    tab.model.createSnapshot(snapName.text);
                    snapName.clear();
                }
            }
        }
    }

    SettingsCard {
        title: "Automatic snapshots"
        iconName: "chronometer"
        help: "Taken every hour, and before and after each update. The oldest are removed to keep these numbers."
        visible: tab.snapshots !== null && Object.keys(tab.config).length > 0

        SwitchRow {
            settingId: "snapshot-timeline"
            label: "Take a snapshot every hour"
            highlight: App.highlight
            value: tab.config.TIMELINE_CREATE === "yes"
            enabled: tab.idle
            Layout.fillWidth: true
            onChosen: v => tab.model.setSnapper("TIMELINE_CREATE", v ? "yes" : "no")
        }
        Repeater {
            model: [
                { key: "TIMELINE_LIMIT_HOURLY", label: "Hourly snapshots kept" },
                { key: "TIMELINE_LIMIT_DAILY", label: "Daily snapshots kept" },
                { key: "TIMELINE_LIMIT_WEEKLY", label: "Weekly snapshots kept" },
                { key: "TIMELINE_LIMIT_MONTHLY", label: "Monthly snapshots kept" },
                { key: "NUMBER_LIMIT", label: "Update snapshots kept" }
            ]
            delegate: SettingRow {
                id: limitRow

                required property var modelData

                settingId: "snapshot-limits"
                label: limitRow.modelData.label
                highlight: App.highlight
                enabled: tab.idle && tab.config[limitRow.modelData.key] !== undefined
                Layout.fillWidth: true

                QQC2.SpinBox {
                    from: 0
                    to: 100
                    value: tab.limit(limitRow.modelData.key)
                    Accessible.name: limitRow.modelData.label
                    onValueModified: tab.model.setSnapper(limitRow.modelData.key, String(value))
                }
            }
        }
    }

    SettingsCard {
        title: "Snapshots on this computer"
        iconName: "view-list-details"
        wide: true
        visible: tab.snapshots !== null

        UqLabel {
            text: "No snapshots yet."
            visible: (tab.snapshots || []).length === 0
            opacity: 0.7
        }
        Repeater {
            model: tab.snapshots || []
            delegate: RowLayout {
                id: shot

                required property var modelData

                objectName: "snapshot"
                spacing: Kirigami.Units.largeSpacing
                Layout.fillWidth: true

                UqLabel {
                    text: String(shot.modelData.number)
                    opacity: 0.7
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 2
                }
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    UqLabel {
                        text: String(shot.modelData.description || "No name")
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    UqLabel {
                        text: String(shot.modelData.date || "").slice(0, 16) + (tab.kindText(shot.modelData) !== "" ? ", " + tab.kindText(shot.modelData) : "")
                        opacity: 0.7
                    }
                }
                QQC2.Button {
                    text: "Roll back"
                    icon.name: "document-revert"
                    visible: tab.canRollBack
                    enabled: tab.idle
                    Accessible.name: "Roll back to snapshot " + shot.modelData.number
                    onClicked: {
                        confirmRollback.number = shot.modelData.number;
                        confirmRollback.open();
                    }
                }
                QQC2.ToolButton {
                    text: "Delete"
                    icon.name: "edit-delete"
                    display: QQC2.AbstractButton.IconOnly
                    enabled: tab.idle
                    Accessible.name: "Delete snapshot " + shot.modelData.number
                    QQC2.ToolTip.text: "Delete"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: {
                        confirmDelete.number = shot.modelData.number;
                        confirmDelete.open();
                    }
                }
            }
        }
    }
}
