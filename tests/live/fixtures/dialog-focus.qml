// A window with a modal dialog over it, like a file manager's "New folder": each window has a text
// field that prints what it receives, so a script can tell which window the keyboard reached.
// Run with: qml dialog-focus.qml   (QT_QPA_PLATFORM=wayland)
import QtQuick
import QtQuick.Window

Window {
    id: main
    width: 700
    height: 420
    visible: true
    title: "focus-parent"
    color: "#445"

    TextInput {
        id: parentField
        anchors { left: parent.left; top: parent.top; margins: 12 }
        color: "white"
        font.pixelSize: 24
        text: ""
        focus: true
        onTextChanged: console.log("parent:" + text)
    }

    Timer {
        interval: 800
        running: true
        onTriggered: dlg.visible = true
    }

    Window {
        id: dlg
        transientParent: main
        modality: Qt.WindowModal
        flags: Qt.Dialog
        width: 360
        height: 140
        title: "focus-dialog"
        color: "#cde"
        visible: false
        onVisibleChanged: if (visible) field.forceActiveFocus()

        Text { x: 12; y: 10; text: "New folder"; font.pixelSize: 18 }
        Rectangle {
            x: 12; y: 50; width: 336; height: 40; color: "white"
            TextInput {
                id: field
                anchors { fill: parent; margins: 6 }
                font.pixelSize: 24
                focus: true
                onTextChanged: console.log("dialog:" + text)
            }
        }
    }
}
