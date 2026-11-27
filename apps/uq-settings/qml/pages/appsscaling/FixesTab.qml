// Apps and scaling' App fixes tab: per-app scaling, environment and arguments.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    // Passed in from AppsScalingPage.qml.
    property var cm
    property var confirmRemove
    property var fixDialog
    property var page

    title: "App fixes"
    iconName: "tools-wizard"

    SettingsCard {
        title: "App fixes"
        iconName: "tools-wizard"
        help: "How uQ starts one app: on Wayland or X11, at its own size, with extra settings."
        wide: true

        SettingRow {
            settingId: "app-fixes"
            iconName: page.fixes.length === 0 ? "dialog-information" : ""
            label: page.ready && page.fixes.length === 0 ? "No app has a fix yet" : ""
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "addFix"
                text: "Add a fix"
                icon.name: "list-add"
                enabled: page.idle && page.fixes.length < (page.limits.apps || 100)
                onClicked: fixDialog.startChoosing("")
            }
        }

        // The fixes as tiles, two or three to a row at full width.
        GridLayout {
            columns: Math.max(1, Math.floor(width / (Kirigami.Units.gridUnit * 26)))
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing
            uniformCellWidths: true
            Layout.fillWidth: true

            Repeater {
                model: page.fixes
                delegate: Rectangle {
                    id: fixRow
                    required property var modelData
                    readonly property string appName: String(fixRow.modelData.name || fixRow.modelData.id)
                    readonly property string overrideLine: page.overrideText(fixRow.modelData)
                    radius: Kirigami.Units.cornerRadius
                    color: Kirigami.Theme.backgroundColor
                    border.width: 1
                    border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.1)
                    implicitHeight: fixColumn.implicitHeight + Kirigami.Units.largeSpacing * 2
                    implicitWidth: fixColumn.implicitWidth + Kirigami.Units.largeSpacing * 2
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop
                    Accessible.role: Accessible.Grouping
                    Accessible.name: fixRow.appName

                    ColumnLayout {
                        id: fixColumn
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.largeSpacing
                        spacing: Kirigami.Units.smallSpacing

                        RowLayout {
                            spacing: Kirigami.Units.largeSpacing
                            Layout.fillWidth: true

                            Kirigami.Icon {
                                source: String(fixRow.modelData.icon || "") !== "" ? String(fixRow.modelData.icon) : "application-x-executable"
                                implicitWidth: Kirigami.Units.iconSizes.large
                                implicitHeight: Kirigami.Units.iconSizes.large
                                Accessible.ignored: true
                            }
                            ColumnLayout {
                                spacing: 0
                                Layout.fillWidth: true
                                UqLabel {
                                    objectName: "fixName"
                                    text: fixRow.appName
                                    font.bold: true
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                UqLabel {
                                    text: page.fixSummary(fixRow.modelData)
                                    wrapMode: Text.Wrap
                                    opacity: 0.7
                                    Layout.fillWidth: true
                                }
                            }
                            QQC2.ToolButton {
                                text: "Edit"
                                icon.name: "document-edit"
                                display: QQC2.AbstractButton.IconOnly
                                enabled: page.idle
                                Accessible.description: fixRow.appName
                                QQC2.ToolTip.visible: hovered
                                QQC2.ToolTip.text: "Edit"
                                onClicked: fixDialog.start(String(fixRow.modelData.id), fixRow.appName, fixRow.modelData)
                            }
                            QQC2.ToolButton {
                                text: "Remove"
                                icon.name: "edit-delete"
                                display: QQC2.AbstractButton.IconOnly
                                enabled: page.idle
                                Accessible.description: fixRow.appName
                                QQC2.ToolTip.visible: hovered
                                QQC2.ToolTip.text: "Remove"
                                onClicked: {
                                    confirmRemove.appId = String(fixRow.modelData.id);
                                    confirmRemove.open();
                                }
                            }
                        }
                        QQC2.CheckBox {
                            id: elsewhereBox
                            text: "Also when started from other launchers"
                            checked: fixRow.modelData.elsewhere === true
                            enabled: page.idle
                            Accessible.description: fixRow.appName
                            onToggled: {
                                cm.setElsewhere(String(fixRow.modelData.id), elsewhereBox.checked);
                                elsewhereBox.checked = Qt.binding(() => fixRow.modelData.elsewhere === true);
                            }
                        }
                        UqLabel {
                            objectName: "overrideState"
                            visible: fixRow.overrideLine !== ""
                            text: fixRow.overrideLine
                            color: fixRow.modelData.override === "error" ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                            wrapMode: Text.Wrap
                            opacity: fixRow.modelData.override === "written" ? 0.7 : 1
                            Layout.fillWidth: true
                        }
                        SourceLine {
                            source: fixRow.modelData.origin || null
                            textName: "fixOrigin"
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }
    }
}
