// A fractional setting that is either "Use default" (null) or a number in a range. Like
// OverrideSpin, a change is sent 400 ms after the last move, and the slider shows `value` again
// once the model is done, so a refused change snaps back.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import "."

SettingRow {
    id: root

    property var value: null
    property real from: 0
    property real to: 1
    property real stepSize: 0.05
    /// What unticking "Use default" starts from, and what the slider shows until then.
    property real fallback: 1
    /// Captions at the two ends, "" for none.
    property string lowLabel: ""
    property string highLabel: ""
    /// True while a request is in flight; the slider keeps what the user chose until it is done.
    property bool busy: false
    /// The new value: a number rounded to two places, or null for "Use default".
    signal chosen(var value)
    readonly property alias slider: sl
    readonly property alias useDefaultBox: useDefault
    readonly property real shown: value === null ? fallback : value

    UqLabel {
        text: root.lowLabel
        visible: root.lowLabel !== ""
    }
    QQC2.Slider {
        id: sl
        from: root.from
        to: root.to
        stepSize: root.stepSize
        value: root.shown
        enabled: root.enabled && root.value !== null
        Accessible.name: root.label
        onMoved: settle.restart()
    }
    UqLabel {
        text: root.highLabel
        visible: root.highLabel !== ""
    }
    UqLabel {
        text: (Math.round(sl.value * 100) / 100).toString()
        Layout.minimumWidth: 36
    }
    Timer {
        id: settle
        interval: 400
        onTriggered: root.chosen(Math.round(sl.value * 100) / 100)
    }
    onBusyChanged: {
        if (!busy)
            sl.value = Qt.binding(() => root.shown);
    }
    QQC2.CheckBox {
        id: useDefault
        checked: root.value === null
        enabled: root.enabled
        Accessible.name: root.label + ": use default"
        onToggled: {
            settle.stop();
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
