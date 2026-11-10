// Appearance: theme, light or dark, automatic switching, and the font, cursor, icon and Qt style
// overrides. Everything is read from and written to uqd through AppearanceModel; a refused change
// shows uqd's message and the controls go back to the last good value.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"

QQC2.ScrollView {
    id: page

    readonly property var snap: {
        try {
            return JSON.parse(am.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var appearance: snap.appearance || {}
    readonly property var schedule: snap.schedule || { mode: "off" }
    readonly property bool ready: App.uqdUp
    /// The rendered theme's tokens: what "Use theme" means right now, for seeding overrides.
    readonly property var tokens: {
        try {
            return JSON.parse(App.themeJson || "{}");
        } catch (e) {
            return {};
        }
    }
    function field(name, value) {
        am.setAppearance(name, JSON.stringify(value));
    }

    Component.onCompleted: am.refresh()
    Accessible.role: Accessible.Pane
    Accessible.name: "Appearance"
    contentWidth: availableWidth

    AppearanceModel {
        id: am
    }

    Connections {
        target: App
        function onUqdUpChanged() {
            if (App.uqdUp)
                am.refresh();
        }
    }

    ColumnLayout {
        id: column
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 40)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Appearance"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start uqd to change these settings."
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "changeError"
            visible: am.error !== ""
            title: "That change wasn't applied"
            detail: am.error
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Theme"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SettingRow {
            settingId: "theme"
            label: "Theme"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            Flow {
                spacing: Kirigami.Units.smallSpacing
                Layout.preferredWidth: Kirigami.Units.gridUnit * 22
                Repeater {
                    model: page.snap.themes || []
                    QQC2.Button {
                        required property string modelData
                        readonly property bool current: modelData === page.snap.theme
                        checkable: true
                    highlighted: current
                        checked: current
                        Accessible.name: modelData
                        contentItem: UqLabel {
                            text: modelData
                            horizontalAlignment: Text.AlignHCenter
                            font.bold: current
                            color: current ? Kirigami.Theme.highlightColor : Kirigami.Theme.textColor
                            padding: Kirigami.Units.smallSpacing
                        }
                        onClicked: {
                            am.setTheme(modelData, page.snap.variant || "dark");
                            checked = Qt.binding(() => current);
                        }
                    }
                }
            }
        }
        SettingRow {
            settingId: "variant"
            label: "Light or dark"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            Repeater {
                model: [{ id: "light", name: "Light" }, { id: "dark", name: "Dark" }]
                QQC2.Button {
                    required property var modelData
                    readonly property bool current: modelData.id === page.snap.variant
                    checkable: true
                    highlighted: current
                    checked: current
                    Accessible.name: modelData.name
                    contentItem: UqLabel {
                        text: modelData.name
                        horizontalAlignment: Text.AlignHCenter
                        font.bold: current
                        color: current ? Kirigami.Theme.highlightColor : Kirigami.Theme.textColor
                        padding: Kirigami.Units.smallSpacing
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

        UqLabel {
            text: "Fonts"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        OverrideCombo {
            settingId: "ui-font"
            label: "Interface font"
            highlight: App.highlight
            enabled: page.ready
            choices: Qt.fontFamilies()
            value: page.appearance.ui_font || null
            onChosen: v => page.field("ui_font", v)
            Layout.fillWidth: true
        }
        OverrideCombo {
            settingId: "mono-font"
            label: "Monospace font"
            highlight: App.highlight
            enabled: page.ready
            choices: Qt.fontFamilies()
            value: page.appearance.mono_font || null
            onChosen: v => page.field("mono_font", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: am.busy
            settingId: "font-size"
            label: "Font size"
            highlight: App.highlight
            enabled: page.ready
            from: 6
            to: 32
            fallback: (page.tokens.fonts && page.tokens.fonts.size) || 11
            unit: "pt"
            value: page.appearance.font_size === undefined ? null : page.appearance.font_size
            onChosen: v => page.field("font_size", v)
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Cursor and icons"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        OverrideCombo {
            settingId: "cursor-theme"
            label: "Cursor theme"
            highlight: App.highlight
            enabled: page.ready
            choices: page.snap.cursorThemes || []
            value: page.appearance.cursor_theme || null
            onChosen: v => page.field("cursor_theme", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: am.busy
            settingId: "cursor-size"
            label: "Cursor size"
            highlight: App.highlight
            enabled: page.ready
            from: 16
            to: 96
            fallback: (page.tokens.icons && page.tokens.icons.cursor_size) || 24
            unit: "px"
            value: page.appearance.cursor_size === undefined ? null : page.appearance.cursor_size
            onChosen: v => page.field("cursor_size", v)
            Layout.fillWidth: true
        }
        OverrideCombo {
            settingId: "icon-theme"
            label: "Icon theme"
            highlight: App.highlight
            enabled: page.ready
            choices: page.snap.iconThemes || []
            value: page.appearance.icon_theme || null
            onChosen: v => page.field("icon_theme", v)
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Qt apps"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        SettingRow {
            settingId: "kvantum"
            label: "Style Qt apps to match the theme"
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
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
