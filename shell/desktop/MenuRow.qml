import QtQuick
import Quickshell
import Quickshell.Widgets
import "../core"

// One row of the desktop menu: an optional icon, the label, and on the right a muted detail, a
// check mark (the current choice) or a › (it opens a submenu). `lit` is the menu's keyboard and
// hover selection, kept by DesktopMenu; a disabled row is muted and never lit. Plain text only
// (UqText); every colour, size and radius is a Theme token.
Rectangle {
    id: root

    property var entry: ({})
    property bool lit: false
    readonly property bool usable: entry.enabled !== false
    // A row that names an icon keeps its column even when the theme lacks it, so labels line up.
    readonly property bool hasIcon: !!entry.icon
    readonly property string iconPath: entry.icon ? (Quickshell.iconPath(entry.icon, true) || "") : ""

    signal hovered()
    signal activated()

    implicitWidth: Theme.gapIn * 3 + (hasIcon ? glyph.implicitSize + Theme.gapIn * 2 : 0)
        + label.implicitWidth + Theme.gapIn * 4 + side.implicitWidth
    implicitHeight: Math.max(label.implicitHeight, glyph.implicitSize) + Theme.gapIn * 2
    radius: Math.max(0, Theme.radius - 2)
    color: lit && usable ? Theme.surfaceAlt : "transparent"
    border.color: lit && usable ? Theme.accent : "transparent"
    border.width: lit && usable ? Theme.borderWidth : 0

    Accessible.role: Accessible.MenuItem
    Accessible.name: String(entry.label || "") + (entry.detail ? ", " + entry.detail : "")
    Accessible.checkable: entry.checked !== undefined
    Accessible.checked: entry.checked === true
    Accessible.onPressAction: if (root.usable) root.activated()

    IconImage {
        id: glyph
        visible: root.hasIcon
        anchors.left: parent.left
        anchors.leftMargin: Theme.gapIn * 2
        anchors.verticalCenter: parent.verticalCenter
        implicitSize: Theme.fontSize + 5
        source: root.iconPath
        opacity: root.usable ? 1 : 0.5
    }
    UqText {
        id: label
        anchors.left: glyph.visible ? glyph.right : parent.left
        anchors.leftMargin: Theme.gapIn * 2
        anchors.right: side.left
        anchors.rightMargin: Theme.gapIn * 2
        anchors.verticalCenter: parent.verticalCenter
        maximumLineCount: 1
        elide: Text.ElideRight
        text: String(root.entry.label || "")
        color: root.usable ? Theme.text : Theme.textMuted
    }
    UqText {
        id: side
        anchors.right: parent.right
        anchors.rightMargin: Theme.gapIn * 2
        anchors.verticalCenter: parent.verticalCenter
        text: root.entry.sub ? "›" : (root.entry.checked === true ? "✓" : String(root.entry.detail || ""))
        color: root.entry.checked === true ? Theme.accent : Theme.textMuted
        font.bold: root.entry.checked === true
    }
    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onEntered: root.hovered()
        onClicked: if (root.usable) root.activated()
    }
}
