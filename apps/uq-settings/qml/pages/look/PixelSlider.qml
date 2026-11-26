// A number in pixels (or steps) on a slider with its value beside it, or "Use theme" (null).
// Like OverrideSpin, a change is sent 400 ms after the last move and the slider shows `value`
// again once the model is done, so a refused change snaps back. `live` is what the slider
// shows right now, for the preview.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingRow {
    id: ps

    property var value: null
    property int from: 0
    property int to: 100
    property int fallback: 0
    property string unit: "px"
    property string useLabel: "Use theme"
    property bool busy: false
    readonly property int live: Math.round(sl.value)
    signal chosen(var value)

    highlight: App.highlight
    Layout.fillWidth: true

    QQC2.Slider {
        id: sl
        from: ps.from
        to: ps.to
        stepSize: 1
        snapMode: QQC2.Slider.SnapAlways
        value: ps.value === null ? ps.fallback : ps.value
        enabled: ps.enabled && ps.value !== null
        Accessible.name: ps.label
        Layout.preferredWidth: Kirigami.Units.gridUnit * 8
        onMoved: settle.restart()
    }
    UqLabel {
        objectName: "sliderValue"
        text: ps.live + (ps.unit !== "" ? " " + ps.unit : "")
        horizontalAlignment: Text.AlignRight
        opacity: sl.enabled ? 1 : 0.6
        Layout.minimumWidth: Kirigami.Units.gridUnit * 2.5
    }
    Timer {
        id: settle
        interval: 400
        onTriggered: ps.chosen(Math.round(sl.value))
    }
    onBusyChanged: {
        if (!busy)
            sl.value = Qt.binding(() => ps.value === null ? ps.fallback : ps.value);
    }
    QQC2.CheckBox {
        id: useBox
        checked: ps.value === null
        enabled: ps.enabled
        Accessible.name: ps.label + ": " + ps.useLabel.toLowerCase()
        onToggled: {
            settle.stop();
            ps.chosen(useBox.checked ? null : ps.fallback);
            useBox.checked = Qt.binding(() => ps.value === null);
        }
    }
    UqLabel {
        text: ps.useLabel
        MouseArea {
            anchors.fill: parent
            enabled: useBox.enabled
            onClicked: useBox.toggle()
        }
    }
}
