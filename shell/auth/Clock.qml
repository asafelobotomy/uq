import QtQuick
import "../core"

// The large time and the date, as the lock screen and the greeter show them. `date` comes from the
// caller (a SystemClock under Quickshell, anything under qmltestrunner).
Column {
    id: clock

    property date date: new Date()
    property real sizeFactor: 1

    spacing: Theme.gapOut

    UqText {
        anchors.horizontalCenter: parent.horizontalCenter
        text: Qt.formatDateTime(clock.date, "HH:mm")
        font.pixelSize: Theme.fontSize * 6 * clock.sizeFactor
        Accessible.role: Accessible.StaticText
        Accessible.name: "Time " + text
    }
    UqText {
        anchors.horizontalCenter: parent.horizontalCenter
        text: Qt.formatDateTime(clock.date, "dddd d MMMM")
        font.pixelSize: Theme.fontSize * 1.5 * clock.sizeFactor
        color: Theme.textMuted
        Accessible.role: Accessible.StaticText
        Accessible.name: "Date " + text
    }
}
