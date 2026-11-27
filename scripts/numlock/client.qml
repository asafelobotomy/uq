import QtQuick
import Quickshell

// Two windows for scripts/numlock-live.sh: each logs what its field holds ("numA:<text>"), so the
// driver can tell whether a keypad 1 typed a digit (Num Lock on, as the app sees it) or not.
ShellRoot {
    FloatingWindow {
        title: "numA"
        implicitWidth: 400
        implicitHeight: 300
        color: "#202020"
        TextInput {
            anchors.fill: parent
            focus: true
            color: "white"
            onTextChanged: console.log("numA:" + text)
        }
    }
    FloatingWindow {
        title: "numB"
        implicitWidth: 400
        implicitHeight: 300
        color: "#303030"
        TextInput {
            anchors.fill: parent
            focus: true
            color: "white"
            onTextChanged: console.log("numB:" + text)
        }
    }
}
