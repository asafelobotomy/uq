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
    // The schedule mode on show: the saved one until the user picks another.
    property string mode: schedule.mode || "off"
    readonly property var modes: [
        { id: "off", name: "Off" },
        { id: "fixed", name: "At set times" },
        { id: "sun", name: "Sunrise and sunset" }
    ]

    function modeIndex(id) {
        return modes.findIndex(m => m.id === id);
    }
    function field(name, value) {
        am.setAppearance(name, JSON.stringify(value));
    }
    function hhmm(t, fallback) {
        return t ? String(t).substring(0, 5) : fallback;
    }

    onScheduleChanged: mode = schedule.mode || "off"
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
        SettingRow {
            settingId: "schedule"
            label: "Switch automatically"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            QQC2.ComboBox {
                id: modeBox
                model: page.modes.map(m => m.name)
                currentIndex: page.modeIndex(page.mode)
                Accessible.name: "Switch automatically"
                onActivated: index => {
                    const id = page.modes[index].id;
                    if (id === "off")
                        am.setSchedule(JSON.stringify({ mode: "off" }));
                    page.mode = id;
                    // Until applied, the box keeps showing the saved mode.
                    if (id === "off")
                        modeBox.currentIndex = Qt.binding(() => page.modeIndex(page.mode));
                }
            }
        }
        SettingRow {
            objectName: "fixedTimes"
            label: "Light from, dark from"
            visible: page.mode === "fixed"
            enabled: page.ready
            Layout.fillWidth: true
            QQC2.TextField {
                id: lightAt
                text: page.hhmm(page.schedule.light_at, "07:00")
                inputMethodHints: Qt.ImhPreferNumbers
                validator: RegularExpressionValidator { regularExpression: /^([01]\d|2[0-3]):[0-5]\d$/ }
                Accessible.name: "Light from (HH:MM)"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 5
            }
            QQC2.TextField {
                id: darkAt
                text: page.hhmm(page.schedule.dark_at, "19:00")
                inputMethodHints: Qt.ImhPreferNumbers
                validator: RegularExpressionValidator { regularExpression: /^([01]\d|2[0-3]):[0-5]\d$/ }
                Accessible.name: "Dark from (HH:MM)"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 5
            }
            QQC2.Button {
                Accessible.name: "Apply times"
                enabled: lightAt.acceptableInput && darkAt.acceptableInput
                contentItem: UqLabel {
                    text: "Apply"
                    padding: Kirigami.Units.smallSpacing
                }
                onClicked: am.setSchedule(JSON.stringify({ mode: "fixed", light_at: lightAt.text + ":00", dark_at: darkAt.text + ":00" }))
            }
        }
        SettingRow {
            objectName: "sunPlace"
            label: "Latitude, longitude"
            visible: page.mode === "sun"
            enabled: page.ready
            Layout.fillWidth: true
            QQC2.TextField {
                id: lat
                text: page.schedule.lat !== undefined ? String(page.schedule.lat) : ""
                placeholderText: "51.5"
                validator: DoubleValidator { bottom: -90; top: 90; notation: DoubleValidator.StandardNotation }
                Accessible.name: "Latitude"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 6
            }
            QQC2.TextField {
                id: lon
                text: page.schedule.lon !== undefined ? String(page.schedule.lon) : ""
                placeholderText: "-0.13"
                validator: DoubleValidator { bottom: -180; top: 180; notation: DoubleValidator.StandardNotation }
                Accessible.name: "Longitude"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 6
            }
            QQC2.Button {
                Accessible.name: "Apply location"
                enabled: lat.acceptableInput && lon.acceptableInput
                contentItem: UqLabel {
                    text: "Apply"
                    padding: Kirigami.Units.smallSpacing
                }
                onClicked: am.setSchedule(JSON.stringify({ mode: "sun", lat: Number(lat.text), lon: Number(lon.text) }))
            }
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
            settingId: "font-size"
            label: "Font size"
            highlight: App.highlight
            enabled: page.ready
            from: 6
            to: 32
            fallback: 11
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
            settingId: "cursor-size"
            label: "Cursor size"
            highlight: App.highlight
            enabled: page.ready
            from: 16
            to: 96
            fallback: 24
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
