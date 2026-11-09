import QtQuick
import QtQuick.Layouts
import "../core"

// A quick-settings row with a glyph, a label and an on/off switch; `toggled` on click, Space,
// Return or the accessibility action. Used by the Power, Network, Bluetooth and Theme groups.
Rectangle {
    id: t
    property string label: ""
    property string glyph: ""
    property bool on: false
    signal toggled()

    Layout.fillWidth: true
    implicitHeight: 32
    radius: Theme.radius - 2
    color: tMouse.containsMouse ? Theme.overlay : Theme.surfaceAlt
    activeFocusOnTab: true
    border.color: Theme.accent
    border.width: activeFocus ? 1 : 0
    Accessible.role: Accessible.CheckBox
    Accessible.checkable: true
    Accessible.checked: on
    Accessible.name: label
    Accessible.onPressAction: t.toggled()
    Accessible.onToggleAction: t.toggled()
    Keys.onSpacePressed: t.toggled()
    Keys.onReturnPressed: t.toggled()

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Theme.gapIn * 2
        anchors.rightMargin: Theme.gapIn * 2
        spacing: Theme.gapIn * 2
        UqText {
            Layout.preferredWidth: 18
            horizontalAlignment: Text.AlignHCenter
            text: t.glyph
        }
        UqText {
            Layout.fillWidth: true
            text: t.label
        }
        // switch
        Rectangle {
            Layout.preferredWidth: 30
            Layout.preferredHeight: 16
            radius: 8
            color: t.on ? Theme.accent : Theme.overlay
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                x: t.on ? parent.width - width - 2 : 2
                width: 12
                height: 12
                radius: 6
                color: t.on ? Theme.accentText : Theme.textMuted
            }
        }
    }
    MouseArea {
        id: tMouse
        anchors.fill: parent
        hoverEnabled: true
        onClicked: t.toggled()
    }
}
