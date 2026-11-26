// Appearance's Import and export tab: theme files in and out.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    // Passed in from AppearancePage.qml.
    property var exportDialog
    property var importDialog
    property var page
    property var tm
    // Read by AppearancePage.qml's import and export dialogs.
    property alias importLookChecked: importLookBox.checked
    property alias exportLookChecked: exportLookBox.checked

    title: "Import and export"
    iconName: "document-export"

    SettingsCard {
        title: "Make a theme"
        iconName: "list-add"

        SettingRow {
            settingId: "theme-new"
            iconName: "document-new"
            label: "New theme from this one"
            help: "Starts from the colours of the theme in use."
            highlight: App.highlight
            enabled: page.idle
            Layout.fillWidth: true
            QQC2.Button {
                text: "New…"
                icon.name: "list-add"
                Accessible.name: "New theme from this one"
                onClicked: {
                    page.editorWanted = true;
                    tm.openEditor(page.snap.theme || "", false);
                }
            }
        }
    }
    SettingsCard {
        title: "Share themes"
        iconName: "document-share"

        SettingRow {
            settingId: "theme-import"
            iconName: "document-import"
            label: "Import a theme"
            highlight: App.highlight
            enabled: page.idle
            Layout.fillWidth: true
            QQC2.CheckBox {
                id: importLookBox
                text: "With its window look"
                Accessible.name: "Also use its window gaps, corners and effects"
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: "Also use its window gaps, corners and effects"
            }
            QQC2.Button {
                text: "Import…"
                icon.name: "document-import"
                onClicked: importDialog.open()
            }
        }
        SettingRow {
            settingId: "theme-export"
            iconName: "document-export"
            label: "Export the theme in use"
            highlight: App.highlight
            enabled: page.idle
            Layout.fillWidth: true
            UqLabel {
                objectName: "exportsWhich"
                text: page.currentThemeName
                elide: Text.ElideRight
                opacity: 0.7
                Layout.maximumWidth: Kirigami.Units.gridUnit * 8
            }
            QQC2.CheckBox {
                id: exportLookBox
                text: "With the window look"
                Accessible.name: "Include window gaps, corners and effects"
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: "Include window gaps, corners and effects"
            }
            QQC2.Button {
                text: "Export…"
                icon.name: "document-export"
                onClicked: {
                    exportDialog.themeId = page.snap.theme || "";
                    exportDialog.open();
                }
            }
        }
    }
}
