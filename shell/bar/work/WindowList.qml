// Every window on the workspace, from +N or the lead icon.
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
    property var listPopup
    property var root

    spacing: 2
    width: Theme.fontSize * 26
    UqText {
        text: root.ordered.length === 1 ? "1 window on this workspace" : root.ordered.length + " windows on this workspace"
        color: Theme.textMuted
    }
    Repeater {
        model: root.ordered
        delegate: MenuRow {
            required property var modelData
            required property int index
            Layout.fillWidth: true
            text: modelData.title || modelData.cls
            detail: modelData.focused ? "Focused"
                : (root.alerts[modelData.address] === true ? "Wants attention"
                : (modelData.minimised ? "Minimised" : ""))
            onActivated: {
                root.choose(modelData);
                listPopup.hide();
            }
            Component.onCompleted: if (index === 0) forceActiveFocus()
        }
    }
}
