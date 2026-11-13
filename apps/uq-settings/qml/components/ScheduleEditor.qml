// The automatic light/dark schedule: three mode buttons, plus time or location fields with Apply.
// `schedule` is uqd's saved schedule. The editor's own state (the chosen mode, half-typed text)
// is reloaded only when the saved schedule's content changes, never on a refresh that brings
// the same content.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

ColumnLayout {
    id: root

    property var schedule: ({ mode: "off" })
    property string highlight
    /// The mode on show: the saved one until the user picks another.
    property string mode: "off"
    readonly property string savedKey: JSON.stringify(schedule || {})
    readonly property alias lightField: lightAt
    readonly property alias darkField: darkAt
    readonly property alias latField: lat
    readonly property alias lonField: lon
    readonly property var modes: [
        { id: "off", name: "Off", icon: "dialog-cancel", tip: "Stay on the light or dark chosen above" },
        { id: "fixed", name: "At set times", icon: "clock", tip: "Light and dark at the times you set" },
        { id: "sun", name: "Sunrise and sunset", icon: "weather-few-clouds", tip: "Follow the sun where you are" }
    ]
    /// `json` for SetSchedule.
    signal apply(string json)

    function modeIndex(id) {
        return modes.findIndex(m => m.id === id);
    }
    function hhmm(t, fallback) {
        return t ? String(t).substring(0, 5) : fallback;
    }
    function reload() {
        const s = schedule || {};
        mode = s.mode || "off";
        lightAt.text = hhmm(s.light_at, "07:00");
        darkAt.text = hhmm(s.dark_at, "19:00");
        lat.text = s.lat !== undefined ? String(s.lat) : "";
        lon.text = s.lon !== undefined ? String(s.lon) : "";
    }

    onSavedKeyChanged: reload()
    Component.onCompleted: reload()
    spacing: Kirigami.Units.smallSpacing

    SettingRow {
        settingId: "schedule"
        iconName: "chronometer"
        label: "Switch automatically"
        highlight: root.highlight
        enabled: root.enabled
        Layout.fillWidth: true

        // The modes as buttons, icon first. "Off" applies at once; the others wait for Apply.
        QQC2.ButtonGroup {
            id: modeGroup
        }
        Repeater {
            id: modeBox
            objectName: "modeBox"
            model: root.modes
            QQC2.Button {
                required property var modelData
                objectName: "mode-" + modelData.id
                checkable: true
                checked: root.mode === modelData.id
                QQC2.ButtonGroup.group: modeGroup
                Accessible.name: modelData.name
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: modelData.tip
                contentItem: RowLayout {
                    spacing: Kirigami.Units.smallSpacing
                    Kirigami.Icon {
                        source: modelData.icon
                        implicitWidth: Kirigami.Units.iconSizes.small
                        implicitHeight: Kirigami.Units.iconSizes.small
                        Accessible.ignored: true
                    }
                    UqLabel {
                        text: modelData.name
                    }
                }
                onClicked: {
                    const id = modelData.id;
                    root.mode = id;
                    if (id === "off")
                        root.apply(JSON.stringify({ mode: "off" }));
                    // Until applied, the buttons show the mode in `root.mode`.
                    checked = Qt.binding(() => root.mode === modelData.id);
                }
            }
        }
    }
    SettingRow {
        objectName: "fixedTimes"
        iconName: "clock"
        label: "Light from, dark from"
        visible: root.mode === "fixed"
        enabled: root.enabled
        Layout.fillWidth: true
        Kirigami.Icon {
            source: "weather-clear"
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: Kirigami.Units.iconSizes.small
            Accessible.ignored: true
        }
        QQC2.TextField {
            id: lightAt
            inputMethodHints: Qt.ImhPreferNumbers
            validator: RegularExpressionValidator { regularExpression: /^([01]\d|2[0-3]):[0-5]\d$/ }
            Accessible.name: "Light from (HH:MM)"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 4
        }
        Kirigami.Icon {
            source: "weather-clear-night"
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: Kirigami.Units.iconSizes.small
            Accessible.ignored: true
        }
        QQC2.TextField {
            id: darkAt
            inputMethodHints: Qt.ImhPreferNumbers
            validator: RegularExpressionValidator { regularExpression: /^([01]\d|2[0-3]):[0-5]\d$/ }
            Accessible.name: "Dark from (HH:MM)"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 4
        }
        ChoiceButton {
            Accessible.name: "Apply times"
            enabled: lightAt.acceptableInput && darkAt.acceptableInput
            label: "Apply"
            onClicked: root.apply(JSON.stringify({ mode: "fixed", light_at: lightAt.text + ":00", dark_at: darkAt.text + ":00" }))
        }
    }
    SettingRow {
        objectName: "sunPlace"
        iconName: "mark-location"
        label: "Latitude, longitude"
        help: "Where you are, to work out sunrise and sunset."
        visible: root.mode === "sun"
        enabled: root.enabled
        Layout.fillWidth: true
        QQC2.TextField {
            id: lat
            placeholderText: "51.5"
            validator: DoubleValidator { bottom: -90; top: 90; notation: DoubleValidator.StandardNotation }
            Accessible.name: "Latitude"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 6
        }
        QQC2.TextField {
            id: lon
            placeholderText: "-0.13"
            validator: DoubleValidator { bottom: -180; top: 180; notation: DoubleValidator.StandardNotation }
            Accessible.name: "Longitude"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 6
        }
        ChoiceButton {
            Accessible.name: "Apply location"
            enabled: lat.acceptableInput && lon.acceptableInput
            label: "Apply"
            onClicked: root.apply(JSON.stringify({ mode: "sun", lat: Number(lat.text), lon: Number(lon.text) }))
        }
    }
}
