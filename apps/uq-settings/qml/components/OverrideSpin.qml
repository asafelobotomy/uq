// A numeric setting that is either "Use theme" (null) or a number in a range. The controls show
// `value` again straight after the user changes them, until the model confirms the change.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import "."

SettingRow {
    id: root

    property var value: null
    property int from: 1
    property int to: 100
    property int fallback: 10
    property string unit: ""
    /// True while a request is in flight; the spin box then keeps what the user chose, and shows
    /// the saved value again once it is done (so a refused change snaps back).
    property bool busy: false
    /// The new value: a number, or null for "Use theme".
    signal chosen(var value)

    QQC2.SpinBox {
        id: spin
        from: root.from
        to: root.to
        value: root.value === null ? root.fallback : root.value
        editable: true
        enabled: root.enabled && root.value !== null
        Accessible.name: root.label
        onValueModified: {
            root.chosen(spin.value);
        }
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
            root.chosen(useTheme.checked ? null : root.fallback);
            useTheme.checked = Qt.binding(() => root.value === null);
        }
    }
    UqLabel {
        text: "Use theme"
        MouseArea {
            anchors.fill: parent
            enabled: useTheme.enabled
            onClicked: useTheme.toggle()
        }
    }
}
