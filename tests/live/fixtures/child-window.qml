import QtQuick
import QtQuick.Window

// An app with a dialog (scripts/windows-live.sh): "app main", and 2.5 s later "app dialog", a
// modal dialog whose parent is the main window, as an app's preferences or "Save changes?" are.
Window {
    id: main
    title: "app main"
    visible: true
    width: 640
    height: 420
    color: "#2b3a55"

    Window {
        id: dialog
        transientParent: main
        flags: Qt.Dialog
        modality: Qt.WindowModal
        title: "app dialog"
        width: 360
        height: 200
        color: "#55302b"
    }
    Timer {
        interval: 2500
        running: true
        onTriggered: dialog.visible = true
    }
}
