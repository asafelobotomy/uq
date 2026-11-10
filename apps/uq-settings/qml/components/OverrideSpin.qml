// A numeric setting that is either "Use theme" (null) or a number in a range. The controls show
// `value` again straight after the user changes them, until the model confirms the change. Spin
// changes are sent 400 ms after the last step, so holding an arrow sends one change, not dozens.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import "."

SettingRow {
    id: root

    property var value: null
    property int from: 1
    property int to: 100
    /// What unticking "Use theme" starts from: the theme's current value.
    property int fallback: 10
    property string unit: ""
    /// The caption of the checkbox, for settings whose fallback is not a theme value.
    property string useLabel: "Use theme"
    /// True while a request is in flight; the spin box then keeps what the user chose, and shows
    /// the saved value again once it is done (so a refused change snaps back).
    property bool busy: false
    /// The new value: a number, or null for "Use theme".
    signal chosen(var value)
    readonly property alias spinBox: spin
    readonly property alias useThemeBox: useTheme

    QQC2.SpinBox {
        id: spin
        from: root.from
        to: root.to
        value: root.value === null ? root.fallback : root.value
        editable: true
        enabled: root.enabled && root.value !== null
        Accessible.name: root.label
        onValueModified: settle.restart()
    }
    Timer {
        id: settle
        interval: 400
        onTriggered: root.chosen(spin.value)
    }
    onBusyChanged: {
        if (!busy)
            spin.value = Qt.binding(() => root.value === null ? root.fallback : root.value);
    }
    UqLabel {
        text: root.unit
        visible: root.unit !== ""
    }
    QQC2.CheckBox {
        id: useTheme
        checked: root.value === null
        enabled: root.enabled
        Accessible.name: root.label + ": use theme"
        onToggled: {
            settle.stop();
            root.chosen(useTheme.checked ? null : root.fallback);
            useTheme.checked = Qt.binding(() => root.value === null);
        }
    }
    UqLabel {
        text: root.useLabel
        MouseArea {
            anchors.fill: parent
            enabled: useTheme.enabled
            onClicked: useTheme.toggle()
        }
    }
}
