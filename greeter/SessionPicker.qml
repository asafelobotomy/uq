import QtQuick
import "core"

// The session to start: click, Enter, Space or the arrow keys step through the list (from
// /usr/share/wayland-sessions, uQ first). Names are plain text.
Rectangle {
    id: picker

    // [{id, name, command}] from greeter.js sessionList.
    property var sessions: []
    property int currentIndex: 0
    readonly property var current: currentIndex >= 0 && currentIndex < sessions.length ? sessions[currentIndex] : null

    signal picked(int index)

    function step(d) {
        if (sessions.length === 0)
            return;
        picked((currentIndex + d + sessions.length) % sessions.length);
    }

    implicitWidth: Math.max(160, label.implicitWidth + Theme.gapOut * 2)
    implicitHeight: 32
    radius: Theme.radius - 2
    color: mouse.containsMouse || activeFocus ? Theme.overlay : Theme.surfaceAlt
    border.color: Theme.accent
    border.width: activeFocus ? 1 : 0
    activeFocusOnTab: true
    Accessible.role: Accessible.ComboBox
    Accessible.name: "Session " + label.text
    Keys.onReturnPressed: step(1)
    Keys.onEnterPressed: step(1)
    Keys.onSpacePressed: step(1)
    Keys.onRightPressed: step(1)
    Keys.onDownPressed: step(1)
    Keys.onLeftPressed: step(-1)
    Keys.onUpPressed: step(-1)

    UqText {
        id: label
        anchors.centerIn: parent
        width: Math.min(implicitWidth, parent.width - Theme.gapIn * 2)
        text: picker.current ? picker.current.name + (picker.sessions.length > 1 ? "  ⇄" : "")
                             : "No sessions found"
        color: picker.current ? Theme.text : Theme.error
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        onClicked: picker.step(1)
    }
}
