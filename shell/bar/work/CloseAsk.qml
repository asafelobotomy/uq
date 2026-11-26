// What a tab's X asks when the close setting is ask.
import QtQuick
import QtQuick.Effects
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import "../../core"
import "../../services"
import "../../notif"
import "../islands"
import "../../logic/minimise.js" as Min
import "../../logic/work.js" as Work
import "../../logic/dock.js" as Dock

ColumnLayout {
    // Passed in from WorkIsland.qml.
    property var root

    spacing: 2
    width: Theme.fontSize * 20
    UqText {
        text: "Minimize to Tab?"
        baseBold: true
        Layout.fillWidth: true
    }
    UqText {
        text: root.askWindow ? (root.askWindow.title || root.askWindow.cls) : ""
        color: Theme.textMuted
        Layout.fillWidth: true
    }
    MenuRow {
        Layout.fillWidth: true
        text: "Always minimize instead of close"
        onActivated: root.answerAsk("minimize")
        Component.onCompleted: forceActiveFocus()
    }
    MenuRow {
        Layout.fillWidth: true
        text: "Never minimize instead of close"
        onActivated: root.answerAsk("close")
    }
    MenuRow {
        Layout.fillWidth: true
        text: "Cancel"
        onActivated: root.answerAsk("")
    }
    UqText {
        text: "Change it later in this island's settings."
        color: Theme.textMuted
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
    }
}
