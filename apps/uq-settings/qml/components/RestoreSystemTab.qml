// Restore › System: Bindle's ways back for the whole computer, each saying what stays and what
// goes, as Bindle's own Recovery window does. Rebase and Factory Restore ask first (Factory
// Restore wants ERASE typed), then `bindle` runs them; its output shows in the run card, with
// Restart now when the next start does the work. Bindle asks for the password itself. Reasons
// an action can't run here are Bindle's text, shown through UqLabel.
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
    /// Nothing is in flight: no request and no run.
    required property bool idle
    readonly property var bindle: tab.snap.bindle || ({})
    readonly property var status: tab.bindle.status || null
    /// Show the Snapshots tab, where each snapshot has Roll back.
    property var showSnapshots: null
    readonly property string restoreWhyNot: tab.status && tab.status.restore ? String(tab.status.restore) : ""

    title: "System"
    iconName: "drive-harddisk"

    Kirigami.PromptDialog {
        id: confirmRebase
        objectName: "confirmRebase"
        title: "Rebase the system?"
        ColumnLayout {
            UqLabel {
                text: "Bindle puts the system back to how it was installed: it reinstalls those packages, puts back any that were removed, resets their changed config files and applies Bindle's setup again.\n\nStays: your home folder, the apps you added yourself, Flatpaks, and this computer's own settings (accounts, disks, network, language).\nGoes: changes to the installed packages' config files. Bindle keeps a copy of each, so they can be put back.\n\nBindle takes a snapshot first where it can, and asks for your password."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 26
            }
        }
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Rebase"
                icon.name: "view-refresh"
                onTriggered: {
                    tab.model.recover("rebase", 0);
                    confirmRebase.close();
                }
            }
        ]
    }

    Kirigami.PromptDialog {
        id: confirmErase
        objectName: "confirmErase"
        title: "Erase this computer and install it again?"
        standardButtons: Kirigami.Dialog.Cancel
        onOpened: eraseField.clear()
        customFooterActions: [
            Kirigami.Action {
                text: "Prepare Factory Restore"
                icon.name: "edit-delete"
                enabled: eraseField.text === "ERASE"
                onTriggered: {
                    tab.model.recover("restore", 0);
                    confirmErase.close();
                }
            }
        ]

        ColumnLayout {
            spacing: Kirigami.Units.largeSpacing

            UqLabel {
                text: "The computer restarts into the Arch Linux installer with Bindle's wizard, from this disk. Everything on the disk goes: the system, your home folder and all your files. Save what you want to keep somewhere else first.\n\nNothing is erased until the installer's last question: quitting before then starts this system again as it is. Bindle downloads the installer (about 1.4 GB) unless a copy is kept on this computer, and asks for your password."
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 26
            }
            UqLabel {
                text: "Type ERASE to go on."
            }
            QQC2.TextField {
                id: eraseField
                objectName: "eraseField"
                Accessible.name: "Type ERASE to go on"
                Layout.fillWidth: true
            }
        }
    }

    SettingsCard {
        title: "Bindle"
        iconName: "bindle"
        visible: tab.status === null
        help: "Bindle couldn't say what this computer allows."

        UqLabel {
            text: String(tab.bindle.error || "Checking…")
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        QQC2.Button {
            text: "Check again"
            icon.name: "view-refresh"
            enabled: tab.idle
            onClicked: tab.model.probe()
        }
    }

    SettingsCard {
        title: "Rebase"
        iconName: "view-refresh"
        help: "The system as it was installed, keeping your home and your own apps."
        visible: tab.status !== null

        SettingRow {
            settingId: "rebase"
            label: "Put the system back to how it was installed"
            highlight: App.highlight
            enabled: tab.idle
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "rebase"
                text: "Rebase…"
                icon.name: "view-refresh"
                onClicked: confirmRebase.open()
            }
        }
        UqLabel {
            text: "Stays: your home folder, apps you added, Flatpaks and this computer's own settings. Goes: changes to the installed packages' config files (a copy of each is kept)."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
    }

    SettingsCard {
        title: "Factory Restore"
        iconName: "edit-delete"
        help: "Erase this computer and install it from new with Bindle's installer."
        visible: tab.status !== null

        SettingRow {
            settingId: "factory-restore"
            label: "Erase and install again"
            highlight: App.highlight
            enabled: tab.idle && tab.restoreWhyNot === ""
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "factoryRestore"
                text: "Factory Restore…"
                icon.name: "edit-delete"
                onClicked: confirmErase.open()
            }
        }
        UqLabel {
            text: tab.restoreWhyNot !== "" ? "Not on this computer: " + tab.restoreWhyNot : "Stays: nothing on the disk. Goes: everything. Nothing is erased until the installer's last question."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        SwitchRow {
            settingId: "keep-installer"
            label: "Keep the installer on this computer"
            help: "A checked copy of the installer (about 1.4 GB), for Factory Restore without the internet and, with snapshots, a “Roll back the system” entry in the boot menu."
            highlight: App.highlight
            value: tab.status !== null && tab.status.imageKept === true
            enabled: tab.idle && tab.restoreWhyNot === ""
            Layout.fillWidth: true
            onChosen: v => tab.model.recover(v ? "keep-on" : "keep-off", 0)
        }
    }

    SettingsCard {
        title: "Roll back"
        iconName: "document-revert"
        help: "The system as it was in a snapshot, from the next start."
        visible: tab.status !== null

        SettingRow {
            settingId: "rollback"
            label: "Roll back to a snapshot"
            highlight: App.highlight
            enabled: tab.idle && tab.status !== null && !tab.status.rollback
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "chooseSnapshot"
                text: "Choose a snapshot…"
                icon.name: "go-next"
                onClicked: {
                    if (tab.showSnapshots)
                        tab.showSnapshots();
                }
            }
        }

        UqLabel {
            text: tab.status && tab.status.rollback ? "Not on this computer: " + String(tab.status.rollback) : "Stays: your home folder. Goes: system changes since the snapshot, which are kept aside until you remove them."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
    }
}
