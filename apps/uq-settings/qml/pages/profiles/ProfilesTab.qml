// Profiles' main tab: each profile, with apply, off, the settings it holds and its actions.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    // Passed in from ProfilesPage.qml.
    property var addDialog
    property var confirmDelete
    property var exportDialog
    property var importDialog
    property var newDialog
    property var page
    property var pm
    property var renameDialog

    title: "Profiles"
    iconName: "bookmarks"

    SettingsCard {
        title: "Add a profile"
        iconName: "list-add"
        help: "A profile is a set of settings you turn on together, such as for gaming. Turning it off puts back what it changed, except settings you changed yourself since."

        SettingRow {
            settingId: "profiles"
            iconName: "document-new"
            label: "From your settings"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "newProfile"
                text: "New profile"
                icon.name: "list-add"
                enabled: page.idle
                onClicked: newDialog.start()
            }
        }
        SettingRow {
            settingId: "profile-files"
            iconName: "document-import"
            label: "From a file"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "importProfile"
                text: "Import…"
                icon.name: "document-import"
                enabled: page.idle
                onClicked: importDialog.open()
            }
        }
        UqLabel {
            objectName: "noProfiles"
            text: "No profiles yet."
            visible: page.ready && page.profiles.length === 0
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
    }

    // One card per profile.
    Repeater {
        model: page.profiles
        delegate: SettingsCard {
            id: card

            required property var modelData
            readonly property bool isOn: modelData.active === true
            readonly property var keys: modelData.keys || []

            objectName: "profileCard"
            title: String(modelData.name)
            iconName: modelData.icon || "bookmarks"
            // An "on" profile's card is drawn in the highlight colour.
            border.color: card.isOn ? Kirigami.Theme.highlightColor : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.08)
            border.width: card.isOn ? 2 : 1

            actions: [
                Rectangle {
                    objectName: "onTag"
                    visible: card.isOn
                    radius: height / 2
                    color: Kirigami.Theme.highlightColor
                    implicitWidth: onTag.implicitWidth + Kirigami.Units.largeSpacing
                    implicitHeight: onTag.implicitHeight + Kirigami.Units.smallSpacing
                    UqLabel {
                        id: onTag
                        anchors.centerIn: parent
                        text: "On"
                        color: Kirigami.Theme.highlightedTextColor
                    }
                },
                QQC2.Switch {
                    id: onSwitch
                    objectName: "profileSwitch"
                    checked: card.isOn
                    enabled: page.idle
                    Accessible.name: card.isOn ? "Turn off " + card.modelData.name + " and go back to normal" : "Turn on " + card.modelData.name
                    QQC2.ToolTip.text: card.isOn ? "Back to normal" : "Turn on"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onToggled: {
                        if (onSwitch.checked)
                            pm.apply(card.modelData.id);
                        else
                            pm.off();
                        onSwitch.checked = Qt.binding(() => card.isOn);
                    }
                }
            ]

            RowLayout {
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true

                QQC2.ToolButton {
                    id: expand
                    objectName: "expandKeys"
                    checkable: true
                    enabled: card.keys.length > 0
                    Accessible.name: "Settings in this profile"
                    Accessible.description: page.settingsCount(card.keys.length)
                    contentItem: RowLayout {
                        spacing: Kirigami.Units.smallSpacing
                        Kirigami.Icon {
                            source: expand.checked ? "arrow-down" : "arrow-right"
                            implicitWidth: Kirigami.Units.iconSizes.small
                            implicitHeight: Kirigami.Units.iconSizes.small
                            Accessible.ignored: true
                        }
                        UqLabel {
                            text: page.settingsCount(card.keys.length)
                        }
                    }
                }
                Item {
                    Layout.fillWidth: true
                }
                QQC2.ToolButton {
                    text: "Add settings…"
                    icon.name: "list-add"
                    display: QQC2.AbstractButton.IconOnly
                    enabled: page.idle
                    Accessible.name: "Add settings to " + card.modelData.name
                    QQC2.ToolTip.text: "Add settings…"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: addDialog.start(card.modelData)
                }
                QQC2.ToolButton {
                    text: "Rename…"
                    icon.name: "edit-rename"
                    display: QQC2.AbstractButton.IconOnly
                    enabled: page.idle
                    Accessible.name: "Rename " + card.modelData.name
                    QQC2.ToolTip.text: "Rename…"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: renameDialog.start(card.modelData)
                }
                QQC2.ToolButton {
                    text: "Export…"
                    icon.name: "document-export"
                    display: QQC2.AbstractButton.IconOnly
                    enabled: page.idle
                    Accessible.name: "Export " + card.modelData.name
                    QQC2.ToolTip.text: "Export…"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: {
                        exportDialog.profileId = card.modelData.id;
                        exportDialog.open();
                    }
                }
                QQC2.ToolButton {
                    text: "Delete"
                    icon.name: "edit-delete"
                    display: QQC2.AbstractButton.IconOnly
                    // A disabled button shows no tooltip; keep it hoverable and refuse the click.
                    enabled: page.idle
                    opacity: card.isOn ? 0.4 : 1
                    Accessible.name: "Delete " + card.modelData.name
                    Accessible.description: card.isOn ? "Turn it off first" : ""
                    QQC2.ToolTip.text: card.isOn ? "Turn it off first" : "Delete"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: {
                        if (card.isOn)
                            return;
                        confirmDelete.profileId = card.modelData.id;
                        confirmDelete.open();
                    }
                }
            }
            ProfileKeyList {
                visible: expand.checked
                keys: card.keys
                editable: page.ready
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                onRemoveKey: (domain, pathJson) => pm.removeKey(card.modelData.id, domain, pathJson)
            }
        }
    }
}
