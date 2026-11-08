import Quickshell
import Quickshell.Wayland

// Stub until Task 9.
PanelWindow {
    property var modelData
    screen: modelData
    WlrLayershell.namespace: "uq-bar"
    anchors { top: true; left: true; right: true }
    implicitHeight: 1
    color: "transparent"
}
