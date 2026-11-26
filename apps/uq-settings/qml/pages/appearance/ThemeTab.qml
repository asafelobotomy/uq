// Appearance's Theme tab: the theme, light or dark and the schedule, and editing a theme.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    // Passed in from AppearancePage.qml.
    property var am
    property var confirmDelete
    property var exportDialog
    property var page
    property var tm

    title: "Theme"
    iconName: "preferences-desktop-theme-global"

    SettingsCard {
        id: galleryCard
        title: "Theme"
        iconName: "color-management"
        wide: true

        SettingRow {
            settingId: "theme"
            highlight: App.highlight
            enabled: page.ready
            wideControls: true
            Layout.fillWidth: true

            ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
                Flow {
                    objectName: "themeGallery"
                    spacing: Kirigami.Units.largeSpacing
                    Layout.fillWidth: true
                    Repeater {
                        model: page.themes
                        ThemeChip {
                            required property var modelData
                            theme: Object.assign({}, modelData, {
                                active: modelData.id === page.snap.theme,
                                swatches: page.swatchesOf(modelData.id)
                            })
                            onChosen: id => am.setTheme(id, page.snap.variant || "dark")
                            onEdit: id => {
                                page.editorWanted = true;
                                tm.openEditor(id, true);
                            }
                            onExportTheme: id => {
                                exportDialog.themeId = id;
                                exportDialog.open();
                            }
                            onRemove: id => {
                                const t = page.themeById(id);
                                confirmDelete.themeId = id;
                                confirmDelete.themeName = t ? String(t.name) : id;
                                confirmDelete.active = id === page.snap.theme;
                                confirmDelete.open();
                            }
                        }
                    }
                }
                RowLayout {
                    visible: page.tsnap.rootIsLink === true
                    spacing: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true
                    Kirigami.Icon {
                        source: "dialog-warning"
                        implicitWidth: Kirigami.Units.iconSizes.small
                        implicitHeight: Kirigami.Units.iconSizes.small
                        Accessible.ignored: true
                    }
                    UqLabel {
                        objectName: "rootIsLink"
                        text: "Your themes folder is a link, so uQ won't use or change it."
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                }
                RowLayout {
                    visible: page.skipped.length > 0
                    spacing: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true
                    Kirigami.Icon {
                        source: "dialog-warning"
                        implicitWidth: Kirigami.Units.iconSizes.small
                        implicitHeight: Kirigami.Units.iconSizes.small
                        Accessible.ignored: true
                    }
                    UqLabel {
                        objectName: "skippedLine"
                        text: page.skipped.length === 1 ? "1 theme in your themes folder couldn't be used." : page.skipped.length + " themes in your themes folder couldn't be used."
                        wrapMode: Text.Wrap
                    }
                    QQC2.ToolButton {
                        id: skippedToggle
                        objectName: "skippedToggle"
                        text: "Details"
                        icon.name: checked ? "arrow-down" : "arrow-right"
                        checkable: true
                        display: QQC2.AbstractButton.TextBesideIcon
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                }
                ColumnLayout {
                    visible: page.skipped.length > 0 && skippedToggle.checked
                    spacing: Kirigami.Units.smallSpacing
                    Layout.leftMargin: Kirigami.Units.largeSpacing * 2
                    Layout.fillWidth: true
                    Repeater {
                        model: page.skipped
                        delegate: ColumnLayout {
                            required property var modelData
                            spacing: 0
                            Layout.fillWidth: true
                            UqLabel {
                                text: String(modelData.folder)
                                visible: String(modelData.folder) !== ""
                                font.bold: true
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                            UqLabel {
                                text: String(modelData.reason)
                                wrapMode: Text.Wrap
                                opacity: 0.7
                                Layout.fillWidth: true
                            }
                        }
                    }
                }
            }
        }
    }

    SettingsCard {
        title: "Light or dark"
        iconName: "weather-clear-night"

        SettingRow {
            settingId: "variant"
            iconName: "contrast"
            label: "Style"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            // Two tiles, a sun and a moon: the one in use is checked.
            Repeater {
                model: [
                    { id: "light", name: "Light", icon: "weather-clear" },
                    { id: "dark", name: "Dark", icon: "weather-clear-night" }
                ]
                QQC2.Button {
                    required property var modelData
                    readonly property bool current: modelData.id === page.snap.variant
                    objectName: "variant-" + modelData.id
                    checkable: true
                    checked: current
                    Accessible.name: modelData.name
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                    contentItem: ColumnLayout {
                        spacing: Kirigami.Units.smallSpacing
                        Kirigami.Icon {
                            source: modelData.icon
                            implicitWidth: Kirigami.Units.iconSizes.large
                            implicitHeight: Kirigami.Units.iconSizes.large
                            Layout.alignment: Qt.AlignHCenter
                            Accessible.ignored: true
                        }
                        UqLabel {
                            text: modelData.name
                            font.bold: current
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                        }
                    }
                    onClicked: {
                        am.setTheme(page.snap.theme || "", modelData.id);
                        checked = Qt.binding(() => current);
                    }
                }
            }
        }
        ScheduleEditor {
            schedule: page.schedule
            highlight: App.highlight
            enabled: page.ready
            onApply: json => am.setSchedule(json)
            Layout.fillWidth: true
        }
    }

    SettingsCard {
        title: "Other apps"
        iconName: "preferences-desktop-theme-applications"

        SettingRow {
            settingId: "theme-apps"
            iconName: "preferences-desktop-color"
            label: "Theme other apps"
            help: "Qt, KDE, GTK and Flatpak apps take the theme's colours, fonts, icons and corners, even after another app changed their settings. Off leaves those settings alone."
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            QQC2.Switch {
                id: ta
                checked: page.appearance.theme_apps !== false
                Accessible.name: "Theme other apps"
                onToggled: {
                    page.field("theme_apps", ta.checked);
                    ta.checked = Qt.binding(() => page.appearance.theme_apps !== false);
                }
            }
        }
        SettingRow {
            settingId: "kvantum"
            iconName: "preferences-desktop-theme-applications"
            label: "Style Qt apps to match the theme"
            help: "KDE and other Qt apps take the theme's colours and shapes."
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            QQC2.Switch {
                id: kv
                checked: page.appearance.kvantum !== false
                Accessible.name: "Style Qt apps to match the theme"
                onToggled: {
                    page.field("kvantum", kv.checked);
                    kv.checked = Qt.binding(() => page.appearance.kvantum !== false);
                }
            }
        }
    }
}
