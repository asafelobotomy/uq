import QtQuick
import Quickshell
import Quickshell.Widgets
import "../../core"

// One row of an island popup or the bar menu: a click, Enter or Space activates it; Up and
// Down move to the previous and next row (any focusable item in the popup). `text` is plain
// text (UqText); `detail` is shown muted on the right.
Rectangle {
    id: root

    property string text: ""
    // An icon name: the row shows the icon instead of `text` (which still names it for screen
    // readers through Accessible.name).
    property string icon: ""
    property string detail: ""
    property bool danger: false
    property alias textItem: label
    signal activated()

    readonly property bool lit: mouse.containsMouse || activeFocus
    implicitWidth: (icon !== "" ? glyph.implicitWidth : label.implicitWidth) + detailText.implicitWidth + Theme.gapIn * 4
    implicitHeight: Math.max(label.implicitHeight, detailText.implicitHeight) + Theme.gapIn * 2
    radius: Math.max(0, Theme.radius - 2)
    color: lit ? Theme.surfaceAlt : "transparent"
    border.color: activeFocus ? Theme.accent : "transparent"
    border.width: activeFocus ? Theme.borderWidth : 0
    activeFocusOnTab: true

    Accessible.role: Accessible.MenuItem
    Accessible.name: detail !== "" ? text + ", " + detail : text
    Accessible.onPressAction: activated()

    Keys.onReturnPressed: activated()
    Keys.onEnterPressed: activated()
    Keys.onSpacePressed: activated()
    Keys.onUpPressed: nextItemInFocusChain(false).forceActiveFocus()
    Keys.onDownPressed: nextItemInFocusChain(true).forceActiveFocus()

    IconImage {
        id: glyph
        visible: root.icon !== ""
        anchors.centerIn: parent
        implicitSize: Theme.fontSize + 5
        source: root.icon !== "" ? Quickshell.iconPath(root.icon, true) : ""
    }
    UqText {
        id: label
        visible: root.icon === ""
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        anchors.leftMargin: Theme.gapIn
        anchors.right: detailText.left
        anchors.rightMargin: Theme.gapIn
        text: root.text
        color: root.danger ? Theme.error : Theme.text
    }
    UqText {
        id: detailText
        anchors.verticalCenter: parent.verticalCenter
        anchors.right: parent.right
        anchors.rightMargin: Theme.gapIn
        text: root.detail
        color: Theme.textMuted
        visible: text !== ""
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        onClicked: root.activated()
    }
}
