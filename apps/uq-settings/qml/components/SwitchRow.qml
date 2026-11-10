// A plain on/off setting: a label and a switch. The switch shows `value` again straight after
// the user flips it, until the model confirms, so a refused change snaps back.
import QtQuick
import QtQuick.Controls as QQC2
import "."

SettingRow {
    id: root

    property bool value: false
    /// The new state, after the user flips the switch.
    signal chosen(bool value)
    readonly property alias toggle: sw

    QQC2.Switch {
        id: sw
        checked: root.value
        enabled: root.enabled
        Accessible.name: root.label
        onToggled: {
            root.chosen(sw.checked);
            sw.checked = Qt.binding(() => root.value);
        }
    }
}
