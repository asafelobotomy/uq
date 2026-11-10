// An on/off setting that is either "Use default" (null) or a chosen true or false. The controls
// show `value` again straight after the user changes them, until the model confirms the change,
// so a refused change leaves them on the last good value.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import "."

SettingRow {
    id: root

    /// The override: true, false, or null for "Use default".
    property var value: null
    /// What the switch shows without an override, and what unticking "Use default" starts from.
    property bool fallback: false
    /// The new value: true, false, or null for "Use default".
    signal chosen(var value)
    readonly property alias toggle: sw
    readonly property alias useDefaultBox: useDefault
    readonly property bool shown: value === null ? fallback : value

    QQC2.Switch {
        id: sw
        checked: root.shown
        enabled: root.enabled && root.value !== null
        Accessible.name: root.label
        onToggled: {
            root.chosen(sw.checked);
            sw.checked = Qt.binding(() => root.shown);
        }
    }
    QQC2.CheckBox {
        id: useDefault
        checked: root.value === null
        enabled: root.enabled
        Accessible.name: root.label + ": use default"
        onToggled: {
            root.chosen(useDefault.checked ? null : root.fallback);
            useDefault.checked = Qt.binding(() => root.value === null);
        }
    }
    UqLabel {
        text: "Use default"
        MouseArea {
            anchors.fill: parent
            enabled: useDefault.enabled
            onClicked: useDefault.toggle()
        }
    }
}
