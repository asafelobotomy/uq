// Rules' Windows tab: the window rules, with preview and why.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    id: windowsTab
    // Passed in from RulesPage.qml.
    property var editor
    property var page

    title: "Windows"
    iconName: "window"

    SettingsCard {
        title: "New window rule"
        iconName: "list-add"
        help: "Rules change how windows open and look: float an app's dialogs, open an app on a workspace, keep the screen awake. Later rules win over earlier ones."

        SettingRow {
            settingId: "window-rules"
            iconName: "document-new"
            label: "Blank, or from a template"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            QQC2.Button {
                id: newButton
                objectName: "newRule"
                text: "New rule"
                icon.name: "list-add"
                enabled: page.idle
                onClicked: newMenu.open()

                QQC2.Menu {
                    id: newMenu
                    y: newButton.height

                    QQC2.MenuItem {
                        text: "Blank rule"
                        icon.name: "document-new"
                        onTriggered: editor.startNew("window")
                    }
                    QQC2.MenuSeparator {}
                    Instantiator {
                        model: page.templates
                        delegate: QQC2.MenuItem {
                            id: templateItem
                            required property var modelData
                            // Template names are shown as plain text (F28).
                            contentItem: UqLabel {
                                text: String(templateItem.modelData.name)
                                leftPadding: Kirigami.Units.largeSpacing
                                rightPadding: Kirigami.Units.largeSpacing
                                verticalAlignment: Text.AlignVCenter
                                highlighted: templateItem.highlighted || templateItem.down
                            }
                            Accessible.name: String(modelData.name)
                            onTriggered: page.startTemplate(templateItem.modelData)
                        }
                        // After "Blank rule" and the separator.
                        onObjectAdded: (index, object) => newMenu.insertItem(index + 2, object)
                        onObjectRemoved: (index, object) => newMenu.removeItem(object)
                    }
                }
            }
        }
        SettingRow {
            settingId: "pick-window"
            iconName: "edit-select"
            label: "For a window you click"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "pickWindow"
                text: "Pick a window"
                icon.name: "edit-select"
                enabled: page.idle
                onClicked: page.startPick("new")
            }
        }
        SettingRow {
            settingId: "why-window"
            iconName: "help-contextual"
            label: "Rules a window gets"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "whyWindow"
                text: "Why does this window…?"
                icon.name: "edit-find"
                enabled: page.idle
                onClicked: page.startPick("why")
            }
        }
        UqLabel {
            visible: page.ready && (page.snap.window || []).length === 0
            text: "No window rules yet."
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
    }
    Repeater {
        model: page.snap.window || []
        delegate: RuleCard {
            pg: page
            kind: "window"
            count: (page.snap.window || []).length
        }
    }
}
