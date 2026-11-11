import QtQuick
import "core"

// A plain-text button for the greeter (Tab, Enter, Space and clicks), styled like the shell's.
Rectangle {
    id: button

    property string label: ""
    property bool emphasis: false
    signal activated()

    implicitWidth: caption.implicitWidth + Theme.gapOut * 2
    implicitHeight: 32
    radius: Theme.radius - 2
    color: emphasis ? Theme.accent : (mouse.containsMouse || activeFocus ? Theme.overlay : Theme.surfaceAlt)
    border.color: emphasis ? Theme.text : Theme.accent
    border.width: activeFocus ? 1 : 0
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: label
    Accessible.onPressAction: button.activated()
    Keys.onReturnPressed: button.activated()
    Keys.onEnterPressed: button.activated()
    Keys.onSpacePressed: button.activated()

    UqText {
        id: caption
        anchors.centerIn: parent
        text: button.label
        color: button.emphasis ? Theme.accentText : Theme.text
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        onClicked: button.activated()
    }
}
