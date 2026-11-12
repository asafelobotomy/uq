import QtQuick
import "../core"

// A quick-settings slider (0..1): drag or click to set, arrow keys and the wheel step. Used by
// the Audio and Brightness groups.
Item {
    id: s
    property real value: 0          // 0..1
    property bool dim: false
    property bool dragging: mouse.pressed
    property real dragValue: 0
    readonly property real shown: Math.max(0, Math.min(1, dragging ? dragValue : value))
    signal moved(real v)
    signal stepped(int dir)

    implicitHeight: 24
    activeFocusOnTab: true
    Accessible.role: Accessible.Slider
    Accessible.onIncreaseAction: s.stepped(1)
    Accessible.onDecreaseAction: s.stepped(-1)

    function pick(x) {
        dragValue = Math.max(0, Math.min(1, x / Math.max(1, width)));
        moved(dragValue);
    }

    Keys.onPressed: function (event) {
        if (event.key === Qt.Key_Right || event.key === Qt.Key_Up)
            s.stepped(1);
        else if (event.key === Qt.Key_Left || event.key === Qt.Key_Down)
            s.stepped(-1);
        else
            return;
        event.accepted = true;
    }

    Rectangle {
        id: track
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        height: 6
        radius: 3
        color: Theme.overlay
        Rectangle {
            width: track.width * s.shown
            height: parent.height
            radius: parent.radius
            color: s.dim ? Theme.textMuted : Theme.accent
        }
    }
    Rectangle {
        anchors.verticalCenter: parent.verticalCenter
        x: s.shown * (s.width - width)
        width: 14
        height: 14
        radius: 7
        color: Theme.text
        border.color: s.activeFocus ? Theme.accent : Theme.border
        border.width: s.activeFocus ? 2 : 1
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        onPressed: function (m) { s.forceActiveFocus(); s.pick(m.x); }
        onPositionChanged: function (m) { if (pressed) s.pick(m.x); }
        onWheel: function (w) {
            if (w.angleDelta.y !== 0)
                s.stepped(w.angleDelta.y > 0 ? 1 : -1);
        }
    }
}
