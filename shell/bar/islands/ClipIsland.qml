import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import "../../core"
import "../../clip"
import "../../logic/clip.js" as C
import ".."

// The clipboard history (uqlip). Icon: the clipboard, dimmed while recording is paused. Small:
// and how many entries are kept. A click opens the history popup. Hidden while uqlip isn't
// running.
Island {
    id: root

    readonly property int iconSize: Theme.fontSize + 5
    shown: ClipState.connected
    Accessible.role: Accessible.Button
    Accessible.name: C.summary(ClipState) + ". Open the clipboard history"

    RowLayout {
        anchors.centerIn: parent
        width: Math.min(implicitWidth, root.room)
        spacing: Theme.gapIn

        IconImage {
            implicitSize: root.iconSize
            Layout.preferredWidth: root.iconSize
            Layout.preferredHeight: root.iconSize
            source: Quickshell.iconPath("edit-paste", "edit-copy")
            opacity: ClipState.paused || !ClipState.enabled ? 0.45 : 0.9
        }
        UqText {
            visible: !root.isIcon
            text: String(ClipState.count)
            baseFamily: Theme.fontMono
            color: ClipState.paused ? Theme.textMuted : Theme.text
            Layout.minimumWidth: implicitWidth
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: ClipState.toggle()
    }
}
